//! 只读 EXIF：容器提取 → 有界 TIFF 预检 → 标签解析 → 一次性输出完整报告。
use super::{exif_guard, png};
use crate::files;
use ::exif::{Reader, Value};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use std::{borrow::Cow, fmt::Write as _, io::Cursor, path::Path};

#[derive(Debug, Serialize)]
pub struct ExifReport {
    pub input: std::path::PathBuf,
    pub format: &'static str,
    pub has_exif: bool,
    pub fields: Vec<ExifField>,
}

#[derive(Debug, Serialize)]
pub struct ExifField {
    pub ifd: u16,
    pub context: String,
    pub tag_id: u16,
    pub tag: String,
    #[serde(rename = "type")]
    pub value_type: &'static str,
    pub value: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_offset: Option<u32>,
    pub display_value: String,
}

pub fn read(input: &Path, memory_bytes: usize) -> Result<ExifReport> {
    let bytes = files::read_bounded(input, memory_bytes)?;
    let (format, raw) = extract(&bytes)?;
    let mut fields = Vec::new();
    let mut display_bytes_left = memory_bytes / 16;
    if let Some(raw) = raw {
        exif_guard::validate(&raw, memory_bytes)?;
        // 默认不启用 continue_on_error；损坏元数据不能伪装成“没有 EXIF”，
        // 也不能输出一份看似完整的部分结果。
        let exif = Reader::new()
            .read_raw(raw.into_owned())
            .context("解析 EXIF 失败")?;
        for field in exif.fields() {
            let (value_type, value) = raw_value(&field.value);
            fields.push(ExifField {
                ifd: field.ifd_num.index(),
                context: format!("{:?}", field.tag.context()),
                tag_id: field.tag.number(),
                tag: field.tag.to_string(),
                value_type,
                value,
                value_offset: match &field.value {
                    Value::Undefined(_, offset) => Some(*offset),
                    _ => None,
                },
                // GPS 方位、分辨率单位等来自其他标签；不能仅显示无单位数值。
                display_value: limited_display(
                    field.display_value().with_unit(&exif),
                    &mut display_bytes_left,
                )?,
            });
        }
    }
    Ok(ExifReport {
        input: input.to_path_buf(),
        format,
        has_exif: !fields.is_empty(),
        fields,
    })
}

/// 单位可能引用另一个字段。上千个标签引用同一长单位值时，字段数据很小，
/// 展示文字却会重复放大；必须在 fmt 写入时扣除全报告预算，不能先 to_string
/// 再检查长度，否则超限字符串已经分配。失败发生在打印报告之前。
fn limited_display(value: impl std::fmt::Display, remaining: &mut usize) -> Result<String> {
    struct LimitedText<'a> {
        text: String,
        remaining: &'a mut usize,
    }
    impl std::fmt::Write for LimitedText<'_> {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let left = self
                .remaining
                .checked_sub(text.len())
                .ok_or(std::fmt::Error)?;
            self.text.push_str(text);
            *self.remaining = left;
            Ok(())
        }
    }
    let mut output = LimitedText {
        text: String::new(),
        remaining,
    };
    write!(output, "{value}").context("EXIF 展示文字累计长度超过内存预算（预算的 1/16）")?;
    Ok(output.text)
}

/// 按文件签名选择容器。只读元数据，不启动外部程序，不进行像素解码。
fn extract(bytes: &[u8]) -> Result<(&'static str, Option<Cow<'_, [u8]>>)> {
    if bytes.starts_with(png::SIGNATURE) {
        // 复用 PNG 的完整 chunk/CRC 校验，避免把截断文件误报成“无 EXIF”。
        let parts = png::chunks(bytes)?;
        let data = parts.iter().find(|part| part.kind == *b"eXIf");
        Ok(("PNG", data.map(|part| Cow::Borrowed(part.data))))
    } else if bytes.starts_with(b"II") || bytes.starts_with(b"MM") {
        Ok(("TIFF", Some(Cow::Borrowed(bytes))))
    } else if bytes.starts_with(b"\xff\xd8") {
        match ::exif::get_exif_attr_from_jpeg(&mut Cursor::new(bytes)) {
            Ok(data) => Ok(("JPEG", Some(Cow::Owned(data)))),
            Err(::exif::Error::NotFound(_)) => Ok(("JPEG", None)),
            Err(error) => Err(error).context("读取 JPEG EXIF 失败"),
        }
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Ok(("WebP", webp_exif(bytes)?.map(Cow::Borrowed)))
    } else {
        bail!("不支持此格式的 EXIF 读取；当前支持 JPEG、TIFF、PNG、WebP（按文件内容识别）")
    }
}

/// WebP 的 EXIF chunk 存放 TIFF 数据；兼容少数写入器添加的 Exif\0\0 前缀。
/// 先检查整个 RIFF 长度和 chunk 边界，返回借用切片，避免不必要的分配。
fn webp_exif(bytes: &[u8]) -> Result<Option<&[u8]>> {
    let length = u32::from_le_bytes(bytes[4..8].try_into()?) as usize;
    ensure!(
        length.checked_add(8) == Some(bytes.len()),
        "WebP RIFF 长度与文件不符"
    );
    let mut offset = 12;
    let mut exif = None;
    while offset < bytes.len() {
        ensure!(bytes.len() - offset >= 8, "WebP chunk 头被截断");
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into()?) as usize;
        let start = offset + 8;
        let end = start.checked_add(size).context("WebP chunk 长度溢出")?;
        let next = end.checked_add(size % 2).context("WebP padding 偏移溢出")?;
        ensure!(next <= bytes.len(), "WebP chunk 数据或 padding 被截断");
        if &bytes[offset..offset + 4] == b"EXIF" {
            ensure!(exif.is_none(), "WebP 包含重复 EXIF chunk");
            let data = &bytes[start..end];
            exif = Some(data.strip_prefix(b"Exif\0\0").unwrap_or(data));
        }
        offset = next;
    }
    Ok(exif)
}

/// JSON 同时提供解析值与展示值：不能用四舍五入后的显示字符串替代原分数。
/// ASCII 按库的 NUL 分隔规则保留字节序列，非 UTF-8 内容不会被替换为 �。
fn raw_value(value: &Value) -> (&'static str, JsonValue) {
    match value {
        Value::Byte(v) => ("Byte", json!(v)),
        Value::Ascii(v) => ("Ascii", json!(v)),
        Value::Short(v) => ("Short", json!(v)),
        Value::Long(v) => ("Long", json!(v)),
        Value::Rational(v) => (
            "Rational",
            v.iter()
                .map(|r| json!({"numerator": r.num, "denominator": r.denom}))
                .collect(),
        ),
        Value::SByte(v) => ("SByte", json!(v)),
        Value::Undefined(v, _) => ("Undefined", json!(v)),
        Value::SShort(v) => ("SShort", json!(v)),
        Value::SLong(v) => ("SLong", json!(v)),
        Value::SRational(v) => (
            "SRational",
            v.iter()
                .map(|r| json!({"numerator": r.num, "denominator": r.denom}))
                .collect(),
        ),
        // JSON 数字不能表示 NaN/Infinity，统一用十进制字符串，避免静默变为 null。
        Value::Float(v) => ("Float", v.iter().map(|n| json!(n.to_string())).collect()),
        Value::Double(v) => ("Double", v.iter().map(|n| json!(n.to_string())).collect()),
        Value::Unknown(typ, count, offset) => (
            "Unknown",
            json!({"type_id": typ, "count": count, "offset": offset}),
        ),
    }
}

pub fn render_text(report: &ExifReport) -> String {
    let mut output = format!(
        "图片：{}\n格式：{}\n",
        single_line(&report.input.to_string_lossy()),
        report.format
    );
    if report.fields.is_empty() {
        output.push_str("未发现 EXIF 字段\n");
    } else {
        writeln!(output, "EXIF：{} 个字段", report.fields.len()).unwrap();
        for field in &report.fields {
            writeln!(
                output,
                "[IFD{}/{}] {} (0x{:04X})：{}",
                field.ifd,
                single_line(&field.context),
                single_line(&field.tag),
                field.tag_id,
                single_line(&field.display_value)
            )
            .unwrap();
        }
    }
    output
}

/// 文件名和元数据都不可信；单行输出转义终端控制字符，同时保留正常中文。
fn single_line(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        // 引号、反斜杠本身不会触发终端操作，保留它们避免重复转义库的
        // ASCII 展示值，也避免把正常的 Windows 路径变成双反斜杠。
        if matches!(c, '\'' | '"' | '\\') {
            escaped.push(c);
        } else {
            escaped.extend(c.escape_debug());
        }
    }
    escaped
}
