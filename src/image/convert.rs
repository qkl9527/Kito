//! 通用转换交给 ImageMagick；编码器返回成功不代表转换无损，必须回读校验。
use crate::{
    backend, files,
    image::{
        png,
        resize::{ResizeReport, ResizeSpec},
    },
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const OUTPUTS: &[&str] = &[
    "PNG", "JPEG", "WEBP", "BMP", "TIFF", "GIF", "ICO", "AVIF", "HEIC", "HEIF", "JXL", "TGA",
    "QOI", "PPM", "PGM", "PBM", "PNM", "EXR", "HDR",
];
pub fn canonical(format: &str) -> String {
    match format.to_ascii_uppercase().as_str() {
        "JPG" | "JPE" => "JPEG".into(),
        "TIF" => "TIFF".into(),
        other => other.into(),
    }
}
#[derive(Serialize)]
pub struct FormatCapability {
    pub format: String,
    pub read: bool,
    pub write: bool,
    pub cli_output: bool,
    pub description: String,
}
pub fn formats() -> Result<Vec<FormatCapability>> {
    let result = backend::run_magick(
        &["identify".into(), "-list".into(), "format".into()],
        Some(Duration::from_secs(60)),
    )?;
    let text = String::from_utf8(result.stdout)?;
    let mut formats = Vec::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 4 {
            continue;
        }
        let mode = fields[2].as_bytes();
        if mode.len() != 3 || !matches!(mode[0], b'r' | b'-') || !matches!(mode[1], b'w' | b'-') {
            continue;
        }
        let name = fields[0].trim_end_matches('*').to_string();
        formats.push(FormatCapability {
            cli_output: OUTPUTS.contains(&name.as_str()) && mode[1] == b'w',
            format: name,
            read: mode[0] == b'r',
            write: mode[1] == b'w',
            description: fields[3..].join(" "),
        });
    }
    ensure!(!formats.is_empty(), "无法解析后端格式列表");
    Ok(formats)
}
#[derive(Clone)]
pub struct ConvertConfig {
    pub source_hint: Option<String>,
    pub target: String,
    pub resize: Option<ResizeSpec>,
    pub allow_lossy: bool,
    pub quality: Option<u8>,
    pub background: Option<String>,
    pub frame: Option<usize>,
    pub flatten: bool,
    pub rasterize: bool,
    pub render_raw: bool,
    pub memory_bytes: usize,
    pub timeout: Duration,
}
#[derive(Serialize)]
pub struct Converted {
    #[serde(skip)]
    pub bytes: Vec<u8>,
    pub source_format: String,
    pub target_format: String,
    pub width: u64,
    pub height: u64,
    pub verified_lossless: bool,
    pub encoding_verified_lossless: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resize: Option<ResizeReport>,
    pub transformations: Vec<String>,
    pub backend: String,
    pub elapsed_ms: u128,
}
#[derive(Clone, Debug, Serialize)]
pub struct Info {
    pub format: String,
    pub width: u64,
    pub height: u64,
    pub depth: u64,
    pub colorspace: String,
    pub opaque: bool,
    pub orientation: String,
    pub gamma: f64,
    pub profiles: Vec<String>,
}
fn args_limit(memory: usize) -> Vec<OsString> {
    vec![
        "-limit".into(),
        "memory".into(),
        memory.to_string().into(),
        "-limit".into(),
        "map".into(),
        memory.to_string().into(),
        "-limit".into(),
        "disk".into(),
        "0".into(),
        "-limit".into(),
        "thread".into(),
        "1".into(),
        "-limit".into(),
        "list-length".into(),
        "1024".into(),
    ]
}
fn call(args: Vec<OsString>, timeout: Duration) -> Result<backend::ProcessOutput> {
    backend::run_magick(&args, Some(timeout))
}
fn probe(path: &Path, config: &ConvertConfig) -> Result<Vec<Info>> {
    let mut args = vec!["identify".into()];
    args.extend(args_limit(config.memory_bytes));
    // 使用 ping 预读几何信息，在要求解码完整像素前检查预算。
    args.extend([
        "-ping".into(),
        "-format".into(),
        "%m|%w|%h|%z|%[colorspace]|%[opaque]|%[orientation]|%[gamma]|%[profiles]\n".into(),
        path.as_os_str().to_owned(),
    ]);
    let output = call(args.clone(), config.timeout)?;
    let text = String::from_utf8(output.stdout)?;
    let mut frames = Vec::new();
    for line in text.lines() {
        let f: Vec<_> = line.split('|').collect();
        ensure!(f.len() == 9, "无法解析图片信息");
        frames.push(Info {
            format: f[0].into(),
            width: f[1].parse()?,
            height: f[2].parse()?,
            depth: f[3].parse()?,
            colorspace: f[4].into(),
            opaque: f[5].eq_ignore_ascii_case("true"),
            orientation: f[6].into(),
            gamma: f[7].parse()?,
            profiles: f[8]
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        });
    }
    ensure!(!frames.is_empty(), "未识别到图片");
    let mut total = 0u64;
    for frame in &frames {
        let estimate = frame
            .width
            .checked_mul(frame.height)
            .and_then(|n| n.checked_mul(96))
            .context("图片尺寸溢出")?;
        total = total.checked_add(estimate).context("帧总内存溢出")?;
        ensure!(
            total <= config.memory_bytes as u64,
            "图片解码工作集超过内存预算"
        );
    }
    // -ping 不读取 Alpha 像素，opaque 判断不可靠。通过尺寸预算后再完整识别。
    args.retain(|arg| arg != "-ping");
    let output = call(args, config.timeout)?;
    let text = String::from_utf8(output.stdout)?;
    let lines: Vec<_> = text.lines().collect();
    ensure!(lines.len() == frames.len(), "两次识别的帧数不一致");
    for (frame, line) in frames.iter_mut().zip(lines) {
        let fields: Vec<_> = line.split('|').collect();
        ensure!(fields.len() == 9, "无法解析完整图片信息");
        frame.opaque = fields[5].eq_ignore_ascii_case("true");
        frame.profiles = fields[8]
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    }
    Ok(frames)
}
fn json_info(path: &Path, config: &ConvertConfig) -> Result<Value> {
    let mut args = args_limit(config.memory_bytes);
    args.extend([path.as_os_str().to_owned(), "json:-".into()]);
    let result = call(args, config.timeout)?;
    let value: Value = serde_json::from_slice(&result.stdout)?;
    value
        .get(0)
        .and_then(|v| v.get("image"))
        .cloned()
        .context("后端未返回图片元数据")
}
fn raw_hash(path: &Path, dir: &Path, name: &str, config: &ConvertConfig) -> Result<Vec<u8>> {
    let target = dir.join(name);
    let mut args = args_limit(config.memory_bytes);
    args.extend([
        path.as_os_str().to_owned(),
        "-alpha".into(),
        "on".into(),
        "-depth".into(),
        "16".into(),
        "-endian".into(),
        "MSB".into(),
        format!("RGBA:{}", target.display()).into(),
    ]);
    call(args, config.timeout)?;
    let bytes = files::read_bounded(
        &target,
        config.memory_bytes.checked_mul(8).context("预算溢出")?,
    )?;
    Ok(Sha256::digest(bytes).to_vec())
}
fn profile_bytes(
    path: &Path,
    dir: &Path,
    name: &str,
    slot: &str,
    config: &ConvertConfig,
) -> Result<Vec<u8>> {
    let format = name.to_ascii_uppercase();
    ensure!(
        ["ICC", "ICM", "EXIF", "XMP", "IPTC", "8BIM"].contains(&format.as_str()),
        "无法验证 profile {name}，需要 --allow-lossy"
    );
    let target = dir.join(slot);
    let mut args = args_limit(config.memory_bytes);
    args.extend([
        path.as_os_str().to_owned(),
        format!("{format}:{}", target.display()).into(),
    ]);
    call(args, config.timeout)?;
    Ok(fs::read(target)?)
}
fn verify(
    a: &Path,
    b: &Path,
    dir: &Path,
    config: &ConvertConfig,
    compare_pixels: bool,
) -> Result<()> {
    let ia = probe(a, config)?;
    let ib = probe(b, config)?;
    ensure!(ia.len() == 1 && ib.len() == 1, "转换意外改变帧数");
    let (ia, ib) = (&ia[0], &ib[0]);
    if compare_pixels {
        ensure!(
            (ia.width, ia.height) == (ib.width, ib.height),
            "转换改变尺寸"
        );
    }
    ensure!(
        ia.depth <= 16 && ib.depth <= 16,
        "无法使用 16-bit 校验高精度输入"
    );
    let family = |s: &str| match s {
        "Gray" | "sRGB" => "sRGB",
        "LinearGray" | "RGB" => "RGB",
        _ => "unsupported",
    };
    ensure!(
        family(&ia.colorspace) != "unsupported" && family(&ia.colorspace) == family(&ib.colorspace),
        "颜色空间变化或不支持严格验证，请显式允许有损"
    );
    ensure!((ia.gamma - ib.gamma).abs() < 0.00001, "转换改变 gamma");
    let orientation = |s: &str| {
        if s == "Undefined" || s == "TopLeft" {
            "TopLeft".to_owned()
        } else {
            s.to_owned()
        }
    };
    ensure!(
        orientation(&ia.orientation) == orientation(&ib.orientation),
        "转换改变 EXIF 方向"
    );
    let ma = json_info(a, config)?;
    let mb = json_info(b, config)?;
    ensure!(
        ma["properties"]["quantum:format"] != "floating-point",
        "浮点输入不能用 RGBA16 量化结果证明无损；需 --allow-lossy"
    );
    verify_display_metadata(a, &ma, &mb)?;
    ensure!(
        ma.get("chromaticity") == mb.get("chromaticity"),
        "转换改变颜色原色信息"
    );
    let mut pa = ia.profiles.clone();
    let mut pb = ib.profiles.clone();
    pa.sort();
    pb.sort();
    ensure!(pa == pb, "转换丢失或改变 profile");
    for profile in pa {
        ensure!(
            profile_bytes(a, dir, &profile, "before.profile", config)?
                == profile_bytes(b, dir, &profile, "after.profile", config)?,
            "转换改变 {profile} profile"
        );
    }
    // 对照片格式读取后的像素作比较，不将重新编码 JPEG 冒充无损压缩。
    if compare_pixels {
        ensure!(
            raw_hash(a, dir, "before.rgba", config)? == raw_hash(b, dir, "after.rgba", config)?,
            "转换改变像素（含 Alpha、透明 RGB 或精度）；可显式 --allow-lossy"
        );
    }
    Ok(())
}

fn verify_display_metadata(source: &Path, before: &Value, after: &Value) -> Result<()> {
    // 未声明的默认信息允许由目标编码器补充；显式声明必须在目标中仍有相同语义。
    if before.get("resolution").is_some() {
        ensure!(
            before.get("resolution") == after.get("resolution")
                && before.get("units") == after.get("units"),
            "转换改变像素纵横比／分辨率或单位"
        );
    }
    if before
        .get("renderingIntent")
        .is_some_and(|v| v != "Undefined")
    {
        ensure!(
            before.get("renderingIntent") == after.get("renderingIntent"),
            "转换改变 rendering intent"
        );
    }
    let bytes = std::fs::read(source)?;
    if bytes.starts_with(png::SIGNATURE) {
        let chunks = png::chunks(&bytes)?;
        if chunks.iter().any(|c| c.kind == *b"bKGD") {
            ensure!(
                before.get("backgroundColor") == after.get("backgroundColor"),
                "转换改变 bKGD 背景提示"
            );
        }
        if chunks.iter().any(|c| c.kind == *b"pHYs") {
            ensure!(
                before.get("resolution") == after.get("resolution")
                    && before.get("units") == after.get("units"),
                "转换丢失 pHYs 显示信息"
            );
        }
    }
    if let Some(properties) = before.get("properties").and_then(Value::as_object) {
        for (key, value) in properties {
            // 编码器诊断（png:*、tiff:*）、时间戳和像素签名不是用户文本属性。
            if !key.contains(':') && key != "signature" {
                ensure!(
                    after["properties"].get(key) == Some(value),
                    "转换丢失文本属性 {key}；需要显式允许有损"
                );
            }
        }
    }
    Ok(())
}

pub fn info(bytes: &[u8], hint: Option<&str>) -> Result<Vec<Info>> {
    info_with_limits(bytes, hint, 512 * 1024 * 1024, Duration::from_secs(120))
}

/// 默认格式推断同样属于本次处理，必须继承调用方的预算，不能先用较大的
/// 固定预算完整解码，再在真正转换时才发现用户给出的预算不足。
pub fn info_with_limits(
    bytes: &[u8],
    hint: Option<&str>,
    memory_bytes: usize,
    timeout: Duration,
) -> Result<Vec<Info>> {
    let (_dir, path) = backend::safe_input_as(bytes, hint)?;
    probe(
        &path,
        &ConvertConfig {
            source_hint: hint.map(str::to_owned),
            target: "PNG".into(),
            resize: None,
            allow_lossy: false,
            quality: None,
            background: None,
            frame: None,
            flatten: false,
            rasterize: false,
            render_raw: false,
            memory_bytes,
            timeout,
        },
    )
}

pub fn convert(input: &[u8], config: &ConvertConfig) -> Result<Converted> {
    convert_inner(input, config, true)
}

/// compress 在缩放后还会按自己的 effort/预算搜索 PNG，准备阶段不能偷偷再跑
/// 一次 OxiPNG；候选与耗时应由外层统一记录。仍完整验证中间 PNG 的编码质量。
pub(crate) fn prepare_png_for_compression(
    input: &[u8],
    config: &ConvertConfig,
) -> Result<Converted> {
    ensure!(
        canonical(&config.target) == "PNG" && !config.allow_lossy,
        "压缩准备阶段必须输出无损 PNG"
    );
    convert_inner(input, config, false)
}

fn convert_inner(input: &[u8], config: &ConvertConfig, optimize_png: bool) -> Result<Converted> {
    let started = Instant::now();
    let target = canonical(&config.target);
    ensure!(
        OUTPUTS.contains(&target.as_str()),
        "首版不支持输出 {target}；使用 image formats 查看 cli_output"
    );
    ensure!(
        config.allow_lossy || config.quality.is_none(),
        "--quality 需要 --allow-lossy；严格无损不使用质量百分比"
    );
    ensure!(
        config.allow_lossy || config.background.is_none(),
        "合成背景会移除透明度，需要 --allow-lossy"
    );
    if input.starts_with(png::SIGNATURE)
        && target == "PNG"
        && !config.allow_lossy
        && config.frame.is_none()
        && !config.flatten
        && !config.rasterize
        && !config.render_raw
        && !png::inspect(input)?.animated
        && config
            .resize
            .as_ref()
            .map(|spec| {
                let info = png::inspect(input)?;
                let source = (u64::from(info.width), u64::from(info.height));
                Ok::<_, anyhow::Error>(spec.dimensions(source.0, source.1)? == source)
            })
            .transpose()?
            .unwrap_or(true)
    {
        let info = png::inspect(input)?;
        let bytes = if optimize_png {
            super::optimize::optimize(
                input,
                &super::optimize::OptimizeConfig {
                    memory_bytes: config.memory_bytes,
                    time_limit: Some(config.timeout),
                    ..Default::default()
                },
            )?
            .bytes
        } else {
            png::PngImage::decode(input, config.memory_bytes)?;
            input.to_vec()
        };
        return Ok(Converted {
            bytes,
            source_format: "PNG".into(),
            target_format: "PNG".into(),
            width: u64::from(info.width),
            height: u64::from(info.height),
            verified_lossless: true,
            encoding_verified_lossless: true,
            resize: config.resize.as_ref().map(|spec| ResizeReport {
                requested: spec.to_string(),
                source_width: u64::from(info.width),
                source_height: u64::from(info.height),
                width: u64::from(info.width),
                height: u64::from(info.height),
                changed: false,
                filter: "Lanczos".into(),
            }),
            transformations: Vec::new(),
            backend: if optimize_png {
                "native OxiPNG 10.2.1"
            } else {
                "native PNG validation"
            }
            .into(),
            elapsed_ms: started.elapsed().as_millis(),
        });
    }
    let version = backend::magick_version()?;
    ensure!(version.contains("ImageMagick 7"), "需要 ImageMagick 7");
    if !config.allow_lossy {
        ensure!(
            version.contains("Q16") || version.contains("Q32") || version.contains("Q64"),
            "严格无损需要至少 Q16 精度的 ImageMagick"
        );
    }
    let (dir, source) = backend::safe_input_as(input, config.source_hint.as_deref())?;
    let infos = probe(&source, config)?;
    let format = infos[0].format.clone();
    let animated_png = input.starts_with(png::SIGNATURE) && png::inspect(input)?.animated;
    ensure!(
        !(infos.len() > 1 || animated_png) || config.frame.is_some() || config.flatten,
        "动图／多页／多尺寸图片必须显式 --frame N，PSD 可 --flatten"
    );
    let vector = ["SVG", "MSVG", "RSVG", "PDF", "PS", "EPS", "AI"].contains(&format.as_str());
    ensure!(
        !vector || config.rasterize,
        "矢量／文档输入需要 --rasterize，输出为位图"
    );
    ensure!(
        format != "PSD" || config.flatten,
        "PSD 输入需要 --flatten 使用合成图"
    );
    ensure!(
        !config.flatten || format == "PSD",
        "--flatten 只用于 PSD 合成图"
    );
    let raw = [
        "DNG", "CR2", "CR3", "CRW", "NEF", "NRW", "ARW", "RAF", "RW2", "ORF", "PEF", "SRW", "3FR",
        "DCR",
    ]
    .contains(&format.as_str());
    ensure!(
        !raw || config.render_raw,
        "RAW 输入需要 --render-raw 显式显影"
    );
    let mut frame = config.frame.unwrap_or(0);
    if animated_png {
        let parts = png::chunks(input)?;
        let control = parts
            .iter()
            .find(|c| c.kind == *b"acTL")
            .context("APNG 缺少 acTL")?;
        let count = u32::from_be_bytes(control.data[..4].try_into()?) as usize;
        let frame_control = parts
            .iter()
            .position(|c| c.kind == *b"fcTL")
            .context("APNG 缺少 fcTL")?;
        let idat = parts
            .iter()
            .position(|c| c.kind == *b"IDAT")
            .context("APNG 缺少 IDAT")?;
        let fallback = usize::from(frame_control > idat);
        ensure!(count > 0 && frame < count, "APNG 动画帧索引越界");
        ensure!(
            infos.len() == count + fallback,
            "当前后端没有完整识别 APNG 动画帧，不能将默认图当作动画帧；请换用支持 APNG 的后端"
        );
        frame += fallback;
    }
    ensure!(
        frame < infos.len(),
        "帧／页索引越界或当前后端无法读取动画帧"
    );
    let mut transformations = Vec::new();
    let mut selected: PathBuf;
    if config.frame.is_some() || config.flatten || vector || raw {
        selected = dir.path().join("selected.miff");
        let mut args = args_limit(config.memory_bytes);
        args.push(source.as_os_str().to_owned());
        if ["GIF", "WEBP", "PNG", "APNG"].contains(&format.as_str()) && infos.len() > 1 {
            args.push("-coalesce".into());
        }
        if infos.len() > 1 {
            let delete = (0..infos.len())
                .filter(|&n| n != frame)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",");
            args.extend(["-delete".into(), delete.into()]);
        }
        args.extend(["+repage".into(), selected.as_os_str().to_owned()]);
        call(args, config.timeout)?;
        if config.frame.is_some() {
            transformations.push(format!("选择帧／页 {}", config.frame.unwrap_or(0)));
        }
        if config.flatten {
            transformations.push("使用 PSD 合成图".into());
        }
        if vector {
            transformations.push("矢量栅格化（后端默认分辨率）".into());
        }
        if raw {
            transformations.push("RAW 显影（后端默认设置）".into());
        }
    } else {
        selected = source.clone();
    }
    let mut selected_info = probe(&selected, config)?.remove(0);
    if !config.allow_lossy {
        let metadata = json_info(&selected, config)?;
        ensure!(
            metadata["properties"]["quantum:format"] != "floating-point",
            "浮点输入目前不支持严格无损转换，请显式 --allow-lossy"
        );
        ensure!(
            selected_info.depth <= 16 && !["EXR", "HDR"].contains(&format.as_str()),
            "高动态／浮点输入需显式允许有损转换"
        );
        if input.starts_with(png::SIGNATURE)
            && !animated_png
            && (target != "PNG" || config.resize.is_some())
        {
            let parts = png::chunks(input)?;
            let header = parts[0].data;
            // IHDR 对 indexed PNG 记录的是索引位深，而 sBIT 描述 PLTE 中
            // RGB 样本的精度；调色板项固定为 8-bit，二者不能混用。
            let sample_depth = if header[9] == 3 { 8 } else { header[8] };
            // 对无法跨容器证明保真的 PNG 显示扩展采取保守拒绝。
            for c in parts {
                if c.kind == *b"sBIT" {
                    // chunks() 已校验通道数、取值范围和位置。所有通道均为
                    // 完整位深时，sBIT 与未写该 chunk 的默认语义相同。
                    // 这里只允许省略冗余声明，仍须通过后续像素、精度和颜色
                    // 校验；较低有效位数含额外原始精度信息，不能静默丢弃。
                    ensure!(
                        c.data.iter().all(|&bits| bits == sample_depth),
                        "PNG sBIT 记录了不同于存储位深的原始精度，无法可靠映射到目标格式；请保留 PNG，或显式 --allow-lossy"
                    );
                    continue;
                }
                ensure!(
                    matches!(
                        &c.kind,
                        b"IHDR"
                            | b"PLTE"
                            | b"tRNS"
                            | b"IDAT"
                            | b"IEND"
                            | b"gAMA"
                            | b"cHRM"
                            | b"sRGB"
                            | b"iCCP"
                            | b"eXIf"
                            | b"tEXt"
                            | b"zTXt"
                            | b"iTXt"
                            | b"tIME"
                            | b"pHYs"
                            | b"bKGD"
                    ),
                    "PNG 扩展 {} 不能可靠映射到目标格式",
                    String::from_utf8_lossy(&c.kind)
                );
            }
        }
    }
    if ["JPEG", "AVIF", "HEIC", "HEIF"].contains(&target.as_str()) {
        ensure!(
            config.allow_lossy && config.quality.is_some(),
            "{target} 首版要求 --allow-lossy 和 --quality 1..100"
        );
    }
    let before_resize = selected.clone();
    let mut resize_report = None;
    let mut resized_depth = None;
    if let Some(spec) = &config.resize {
        let (width, height) = spec.dimensions(selected_info.width, selected_info.height)?;
        let changed = (width, height) != (selected_info.width, selected_info.height);
        resize_report = Some(ResizeReport {
            requested: spec.to_string(),
            source_width: selected_info.width,
            source_height: selected_info.height,
            width,
            height,
            changed,
            filter: "Lanczos".into(),
        });
        if changed {
            // 在启动缩放器之前把源图与目标图的工作集一起计入预算；尤其不能
            // 先尝试分配巨大的放大结果，再依靠后端失败来保护内存。
            let pixels = width
                .checked_mul(height)
                .and_then(|n| {
                    selected_info
                        .width
                        .checked_mul(selected_info.height)
                        .and_then(|m| n.checked_add(m))
                })
                .and_then(|n| n.checked_mul(96))
                .context("缩放工作集溢出")?;
            ensure!(
                pixels <= config.memory_bytes as u64,
                "缩放目标与源图的工作集超过内存预算"
            );
            let mut resized = dir.path().join("resized.miff");
            // 重采样产生的新小数值在参考图这一步按原始精度量化，后续验证
            // 比较编码前后参考像素；不能错误要求新像素等于原图，也不能用
            // 8-bit 中间图把原本的 16-bit 数据提前截断后再冒充编码无损。
            let source_depth = if input.starts_with(png::SIGNATURE) {
                u64::from(png::inspect(input)?.bit_depth)
            } else {
                selected_info.depth
            };
            let depth = source_depth.clamp(8, 16);
            resized_depth = Some(depth);
            let mut args = args_limit(config.memory_bytes);
            args.extend([
                selected.as_os_str().to_owned(),
                "-filter".into(),
                "Lanczos".into(),
                "-resize".into(),
                format!("{width}x{height}!").into(),
                "+repage".into(),
                "-depth".into(),
                depth.to_string().into(),
                resized.as_os_str().to_owned(),
            ]);
            // 传给后端的 geometry 完全由受检整数构造，用户字符串从不直接透传。
            call(args, config.timeout)?;
            if !selected_info.opaque {
                // 常规重采样使用 Alpha 加权，避免隐藏颜色污染可见边缘。只有
                // 最终 Alpha=0 的 RGB 从未加权缩放结果恢复，非零 Alpha 完全
                // 保留常规结果。先写 MIFF 完成原精度量化，再判零，避免 HDRI
                // 的微小/负 Alpha 与最终整数 Alpha 不一致。表达式为固定常量，
                // 不接受用户脚本；-channel RGB 确保不改写 Alpha。
                let restored = dir.path().join("resized-exact.miff");
                let mut args = args_limit(config.memory_bytes);
                args.extend([
                    resized.as_os_str().to_owned(),
                    "(".into(),
                    selected.as_os_str().to_owned(),
                    "-alpha".into(),
                    "off".into(),
                    "-filter".into(),
                    "Lanczos".into(),
                    "-resize".into(),
                    format!("{width}x{height}!").into(),
                    ")".into(),
                    "-channel".into(),
                    "RGB".into(),
                    "-fx".into(),
                    "u.a==0?v:u".into(),
                    "+channel".into(),
                    "+repage".into(),
                    "-depth".into(),
                    depth.to_string().into(),
                    restored.as_os_str().to_owned(),
                ]);
                call(args, config.timeout)?;
                resized = restored;
            }
            let frames = probe(&resized, config)?;
            ensure!(
                frames.len() == 1 && (frames[0].width, frames[0].height) == (width, height),
                "缩放后尺寸异常"
            );
            transformations.push(format!(
                "Lanczos 缩放 {}×{} → {width}×{height}",
                selected_info.width, selected_info.height
            ));
            selected_info = frames.into_iter().next().unwrap();
            selected = resized;
        }
    }
    if !selected_info.opaque
        && ["JPEG", "BMP", "PPM", "PGM", "PBM", "PNM", "HDR"].contains(&target.as_str())
    {
        ensure!(
            config.allow_lossy && config.background.is_some(),
            "透明图片输出 {target} 需要 --allow-lossy 和 --background"
        );
    }
    let destination = dir
        .path()
        .join(format!("encoded.{}", target.to_ascii_lowercase()));
    let mut args = args_limit(config.memory_bytes);
    args.push(selected.as_os_str().to_owned());
    if let Some(bg) = &config.background {
        args.extend([
            "-background".into(),
            bg.into(),
            "-alpha".into(),
            "remove".into(),
            "-alpha".into(),
            "off".into(),
        ]);
        transformations.push(format!("合成背景 {bg}"));
    }
    if let Some(quality) = config.quality {
        args.extend(["-quality".into(), quality.to_string().into()]);
    }
    match target.as_str() {
        "WEBP" => {
            args.extend([
                "-define".into(),
                format!("webp:lossless={}", !config.allow_lossy).into(),
                "-define".into(),
                "webp:exact=true".into(),
                "-define".into(),
                "webp:method=6".into(),
            ]);
        }
        "TIFF" => args.extend(["-compress".into(), "Zip".into()]),
        "PNG" => {
            args.extend(["-define".into(), "png:exclude-chunk=date,time".into()]);
            if let Some(depth) = resized_depth {
                args.extend(["-define".into(), format!("png:bit-depth={depth}").into()]);
                if ["sRGB", "RGB"].contains(&selected_info.colorspace.as_str()) {
                    // RGB 图缩放后可能恰好只剩灰色。PNG 自动降为 Gray 会让
                    // 后端丢失 sRGB rendering intent，保留 RGB 表示再无损优化。
                    args.extend([
                        "-define".into(),
                        format!(
                            "png:color-type={}",
                            if selected_info.opaque { 2 } else { 6 }
                        )
                        .into(),
                    ]);
                }
            }
        }
        "JXL" if !config.allow_lossy => args.extend([
            "-quality".into(),
            "100".into(),
            "-define".into(),
            "jxl:distance=0".into(),
        ]),
        _ => {}
    }
    args.push(format!("{target}:{}", destination.display()).into());
    call(args, config.timeout)?;
    let result_info = probe(&destination, config)?;
    ensure!(result_info.len() == 1, "输出帧数异常");
    ensure!(
        result_info[0].width == selected_info.width
            && result_info[0].height == selected_info.height,
        "输出尺寸异常"
    );
    let actual = canonical(&result_info[0].format);
    ensure!(
        actual == target
            || (["HEIC", "HEIF", "AVIF"].contains(&target.as_str()) && actual == "HEIC")
            || (target == "PNM" && ["PPM", "PGM", "PBM"].contains(&actual.as_str())),
        "实际输出格式 {actual} 与请求 {target} 不符"
    );
    if !config.allow_lossy {
        verify(&selected, &destination, dir.path(), config, true)?;
        if resize_report.as_ref().is_some_and(|r| r.changed) {
            // 中间容器不能携带所有 PNG chunk 标记，因此仍直接用变换前的
            // 图片核对最终色彩、profile、pHYs/bKGD 等；这里只跳过已授权的像素变化。
            verify(&before_resize, &destination, dir.path(), config, false)?;
        }
    }
    let mut bytes = files::read_bounded(&destination, config.memory_bytes)?;
    if !config.allow_lossy
        && target == "PNG"
        && input.starts_with(png::SIGNATURE)
        && !animated_png
        && config.frame.is_none()
        && !resize_report.as_ref().is_some_and(|r| r.changed)
    {
        png::equivalent(input, &bytes, false, config.memory_bytes)?;
    }
    if target == "PNG" && optimize_png {
        let result = super::optimize::optimize(
            &bytes,
            &super::optimize::OptimizeConfig {
                memory_bytes: config.memory_bytes,
                ..Default::default()
            },
        )?;
        bytes = result.bytes;
    }
    Ok(Converted {
        bytes,
        source_format: format,
        target_format: actual,
        width: selected_info.width,
        height: selected_info.height,
        verified_lossless: !config.allow_lossy
            && !resize_report.as_ref().is_some_and(|r| r.changed),
        encoding_verified_lossless: !config.allow_lossy,
        resize: resize_report,
        transformations,
        backend: version,
        elapsed_ms: started.elapsed().as_millis(),
    })
}
