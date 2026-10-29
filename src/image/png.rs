//! PNG 的信任边界：先校验容器，再按原精度解码；优化引擎不能自行判定无损。
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use std::io::Cursor;

pub const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
#[derive(Clone, Debug)]
pub struct Chunk<'a> {
    pub kind: [u8; 4],
    pub data: &'a [u8],
    pub encoded: &'a [u8],
}

pub fn chunks(bytes: &[u8]) -> Result<Vec<Chunk<'_>>> {
    ensure!(bytes.starts_with(SIGNATURE), "输入不是 PNG");
    let mut pos = 8usize;
    let mut result = Vec::new();
    while pos < bytes.len() {
        ensure!(bytes.len() - pos >= 12, "PNG chunk 被截断");
        let size = u32::from_be_bytes(bytes[pos..pos + 4].try_into()?) as usize;
        ensure!(size <= bytes.len() - pos - 12, "PNG chunk 长度超出文件");
        let end = pos + size + 12;
        let kind: [u8; 4] = bytes[pos + 4..pos + 8].try_into()?;
        ensure!(
            kind.iter().all(u8::is_ascii_alphabetic) && kind[2].is_ascii_uppercase(),
            "非法 PNG chunk 名称"
        );
        let expected = u32::from_be_bytes(bytes[end - 4..end].try_into()?);
        ensure!(
            crc32fast::hash(&bytes[pos + 4..end - 4]) == expected,
            "PNG {} CRC 错误",
            String::from_utf8_lossy(&kind)
        );
        if result.is_empty() {
            ensure!(kind == *b"IHDR" && size == 13, "PNG 必须以 IHDR 开始");
        } else {
            ensure!(kind != *b"IHDR", "重复 IHDR");
        }
        result.push(Chunk {
            kind,
            data: &bytes[pos + 8..end - 4],
            encoded: &bytes[pos..end],
        });
        pos = end;
        if kind == *b"IEND" {
            ensure!(size == 0 && pos == bytes.len(), "IEND 非法或含尾随数据");
            validate_structure(&result)?;
            return Ok(result);
        }
    }
    bail!("PNG 缺少 IEND")
}

/// 校验解码器可能宽容忽略的结构错误，尤其不能因已经读到像素就跳过尾部。
fn validate_structure(parts: &[Chunk<'_>]) -> Result<()> {
    use std::collections::BTreeSet;
    let header = parts[0].data;
    let depth = header[8];
    let color = header[9];
    let valid_depth = match color {
        0 => [1, 2, 4, 8, 16].contains(&depth),
        2 | 4 | 6 => [8, 16].contains(&depth),
        3 => [1, 2, 4, 8].contains(&depth),
        _ => false,
    };
    ensure!(
        valid_depth && header[10] == 0 && header[11] == 0 && header[12] <= 1,
        "非法 IHDR 编码参数"
    );
    let mut seen = BTreeSet::new();
    let mut data_state = 0;
    let mut palette_entries = 0usize;
    for c in parts {
        let kind = &c.kind;
        if kind[0].is_ascii_uppercase() {
            ensure!(
                matches!(kind, b"IHDR" | b"PLTE" | b"IDAT" | b"IEND"),
                "不支持的 critical chunk {}",
                String::from_utf8_lossy(kind)
            );
        }
        let singleton = matches!(
            kind,
            b"IHDR"
                | b"PLTE"
                | b"IEND"
                | b"tRNS"
                | b"gAMA"
                | b"cHRM"
                | b"sRGB"
                | b"iCCP"
                | b"sBIT"
                | b"bKGD"
                | b"hIST"
                | b"pHYs"
                | b"tIME"
                | b"eXIf"
                | b"cICP"
                | b"mDCV"
                | b"cLLI"
                | b"acTL"
        );
        if singleton {
            ensure!(
                seen.insert(*kind),
                "重复 PNG chunk {}",
                String::from_utf8_lossy(kind)
            );
        }
        if kind == b"IDAT" {
            ensure!(data_state != 2, "IDAT chunks 必须连续");
            data_state = 1;
        } else if data_state == 1 {
            data_state = 2;
        }
        if matches!(
            kind,
            b"PLTE"
                | b"tRNS"
                | b"gAMA"
                | b"cHRM"
                | b"sRGB"
                | b"iCCP"
                | b"sBIT"
                | b"bKGD"
                | b"hIST"
                | b"pHYs"
                | b"sPLT"
                | b"cICP"
                | b"mDCV"
                | b"cLLI"
                | b"acTL"
        ) {
            ensure!(
                data_state == 0,
                "chunk {} 必须位于 IDAT 前",
                String::from_utf8_lossy(kind)
            );
        }
        if matches!(kind, b"gAMA" | b"cHRM" | b"sRGB" | b"iCCP" | b"sBIT") {
            ensure!(palette_entries == 0, "颜色说明 chunk 必须位于 PLTE 前");
        }
        let fixed = match kind {
            b"gAMA" => Some(4),
            b"cHRM" => Some(32),
            b"sRGB" => Some(1),
            b"pHYs" => Some(9),
            b"tIME" => Some(7),
            b"cICP" => Some(4),
            b"mDCV" => Some(24),
            b"cLLI" => Some(8),
            b"acTL" => Some(8),
            b"fcTL" => Some(26),
            _ => None,
        };
        if let Some(len) = fixed {
            ensure!(
                c.data.len() == len,
                "{} 长度错误",
                String::from_utf8_lossy(kind)
            );
        }
        match kind {
            b"PLTE" => {
                ensure!(
                    !matches!(color, 0 | 4)
                        && !c.data.is_empty()
                        && c.data.len() <= 768
                        && c.data.len() % 3 == 0,
                    "非法 PLTE"
                );
                palette_entries = c.data.len() / 3;
                if color == 3 {
                    ensure!(palette_entries <= 1usize << depth, "PLTE 超出位深范围");
                }
            }
            b"gAMA" => ensure!(c.data != [0, 0, 0, 0], "gAMA 不能为零"),
            b"sRGB" => ensure!(c.data[0] <= 3, "非法 sRGB rendering intent"),
            b"pHYs" => ensure!(c.data[8] <= 1, "非法 pHYs 单位"),
            b"tRNS" => ensure!(
                match color {
                    0 => c.data.len() == 2,
                    2 => c.data.len() == 6,
                    3 =>
                        palette_entries > 0 && !c.data.is_empty() && c.data.len() <= palette_entries,
                    _ => false,
                },
                "tRNS 与颜色类型不匹配"
            ),
            b"bKGD" => ensure!(
                match color {
                    0 | 4 => c.data.len() == 2,
                    2 | 6 => c.data.len() == 6,
                    3 => c.data.len() == 1 && usize::from(c.data[0]) < palette_entries,
                    _ => false,
                },
                "bKGD 与颜色类型不匹配"
            ),
            b"hIST" => ensure!(
                palette_entries > 0 && c.data.len() == palette_entries * 2,
                "hIST 与 PLTE 不匹配"
            ),
            b"sBIT" => {
                let count = match color {
                    0 => 1,
                    2 | 3 => 3,
                    4 => 2,
                    6 => 4,
                    _ => 0,
                };
                let limit = if color == 3 { 8 } else { depth };
                ensure!(
                    c.data.len() == count && c.data.iter().all(|&b| b > 0 && b <= limit),
                    "非法 sBIT"
                );
            }
            b"iCCP" | b"zTXt" => {
                let end = c
                    .data
                    .iter()
                    .position(|&b| b == 0)
                    .context("缺少 chunk 名称终止符")?;
                ensure!(
                    (1..=79).contains(&end)
                        && c.data.get(end + 1) == Some(&0)
                        && c.data.len() > end + 2,
                    "非法压缩元数据"
                );
            }
            b"fdAT" => ensure!(c.data.len() >= 4, "fdAT 缺少序号"),
            _ => {}
        }
    }
    ensure!(data_state != 0, "PNG 缺少 IDAT");
    ensure!(color != 3 || palette_entries > 0, "索引 PNG 缺少 PLTE");
    ensure!(
        !(seen.contains(b"sRGB") && seen.contains(b"iCCP")),
        "sRGB 与 iCCP 不能同时存在"
    );
    Ok(())
}

pub fn removable(kind: &[u8; 4]) -> bool {
    matches!(kind, b"tEXt" | b"zTXt" | b"iTXt" | b"tIME")
}
fn is_encoding(kind: &[u8; 4]) -> bool {
    matches!(kind, b"IHDR" | b"PLTE" | b"tRNS" | b"IDAT" | b"IEND")
}

#[derive(Debug, Serialize)]
pub struct PngInfo {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: u8,
    pub animated: bool,
    pub chunks: Vec<String>,
}
pub fn inspect(bytes: &[u8]) -> Result<PngInfo> {
    let parts = chunks(bytes)?;
    let h = parts[0].data;
    Ok(PngInfo {
        width: u32::from_be_bytes(h[..4].try_into()?),
        height: u32::from_be_bytes(h[4..8].try_into()?),
        bit_depth: h[8],
        color_type: h[9],
        animated: parts.iter().any(|c| c.kind == *b"acTL"),
        chunks: parts
            .iter()
            .map(|c| String::from_utf8_lossy(&c.kind).into_owned())
            .collect(),
    })
}

pub struct PngImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u16>,
}
impl PngImage {
    pub fn decode(bytes: &[u8], budget: usize) -> Result<Self> {
        let info = inspect(bytes)?;
        ensure!(!info.animated, "首版不支持完整 APNG；请显式选取帧后转换");
        ensure!(info.width > 0 && info.height > 0, "图片尺寸不能为零");
        // 估算包括两份解码像素、滤波/编码临时缓冲和输入副本。此预算是预检，
        // 并非操作系统 RSS 的硬限制；所有乘加都检查溢出，避免畸形尺寸绕过限制。
        let pixels = (info.width as usize)
            .checked_mul(info.height as usize)
            .context("尺寸溢出")?;
        let estimate = pixels
            .checked_mul(96)
            .and_then(|n| bytes.len().checked_mul(8).and_then(|m| n.checked_add(m)))
            .context("内存估算溢出")?;
        ensure!(
            estimate <= budget,
            "预计工作集 {} bytes 超过内存预算 {} bytes",
            estimate,
            budget
        );
        let mut decoder = png::Decoder::new(Cursor::new(bytes));
        decoder.set_limits(png::Limits { bytes: budget });
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().context("PNG 解码头失败")?;
        let size = reader.output_buffer_size().context("PNG 解码大小溢出")?;
        ensure!(size <= budget, "解码缓冲超过预算");
        let mut buffer = vec![0; size];
        let frame = reader.next_frame(&mut buffer).context("PNG 解码失败")?;
        reader.finish().context("PNG 尾部解码校验失败")?;
        let data = &buffer[..frame.buffer_size()];
        let channels = frame.color_type.samples();
        let mut rgba = Vec::with_capacity(pixels * 4);
        // 使用完整 u16 样本。8-bit 的 x 映射到 x*257，不丢失 16-bit 低字节。
        let samples: Vec<u16> = if frame.bit_depth == png::BitDepth::Sixteen {
            data.as_chunks::<2>()
                .0
                .iter()
                .map(|p| u16::from_be_bytes([p[0], p[1]]))
                .collect()
        } else {
            data.iter().map(|&v| u16::from(v) * 257).collect()
        };
        for p in samples.chunks_exact(channels) {
            let pixel = match frame.color_type {
                png::ColorType::Grayscale => [p[0], p[0], p[0], 65535],
                png::ColorType::GrayscaleAlpha => [p[0], p[0], p[0], p[1]],
                png::ColorType::Rgb => [p[0], p[1], p[2], 65535],
                png::ColorType::Rgba => [p[0], p[1], p[2], p[3]],
                png::ColorType::Indexed => bail!("调色板未展开"),
            };
            rgba.extend_from_slice(&pixel);
        }
        ensure!(rgba.len() == pixels * 4, "像素数与尺寸不符");
        Ok(Self {
            width: info.width,
            height: info.height,
            pixels: rgba,
        })
    }
}

pub fn equivalent(original: &[u8], candidate: &[u8], strip: bool, budget: usize) -> Result<()> {
    let a = PngImage::decode(original, budget)?;
    let b = PngImage::decode(candidate, budget)?;
    ensure!(
        (a.width, a.height) == (b.width, b.height) && a.pixels == b.pixels,
        "候选改变了尺寸或像素，包括透明 RGB／16-bit 低位"
    );
    let ac = chunks(original)?;
    let bc = chunks(candidate)?;
    let metadata = |parts: &[Chunk<'_>]| -> Vec<([u8; 4], Vec<u8>)> {
        parts
            .iter()
            .filter(|c| !is_encoding(&c.kind) && !(strip && removable(&c.kind)))
            .map(|c| (c.kind, c.data.to_vec()))
            .collect()
    };
    ensure!(metadata(&ac) == metadata(&bc), "候选改变了要求保留的元数据");
    if ac.iter().any(|c| c.kind == *b"iCCP") {
        ensure!(
            matches!(ac[0].data[9], 0 | 4) == matches!(bc[0].data[9], 0 | 4),
            "候选颜色类型与原 ICC profile 不兼容"
        );
    }
    for chunk in &ac {
        if matches!(&chunk.kind, b"bKGD" | b"sBIT" | b"hIST") {
            ensure!(
                ac[0].data[8..10] == bc[0].data[8..10],
                "候选改变了元数据依赖的颜色表示"
            );
            let palette = |parts: &[Chunk<'_>]| {
                parts
                    .iter()
                    .find(|c| c.kind == *b"PLTE")
                    .map(|c| c.data.to_vec())
            };
            ensure!(palette(&ac) == palette(&bc), "候选改变了元数据依赖的调色板");
        }
        // PNG 的 unsafe-to-copy 位意味着未知 chunk 不能随重编码任意复制；
        // C2PA 等文件签名同样不能在修改后的文件上冒充仍然有效。
        let known = matches!(
            &chunk.kind,
            b"IHDR"
                | b"PLTE"
                | b"tRNS"
                | b"IDAT"
                | b"IEND"
                | b"cHRM"
                | b"gAMA"
                | b"iCCP"
                | b"sRGB"
                | b"cICP"
                | b"mDCV"
                | b"cLLI"
                | b"sBIT"
                | b"bKGD"
                | b"hIST"
                | b"pHYs"
                | b"sPLT"
                | b"tIME"
                | b"iTXt"
                | b"tEXt"
                | b"zTXt"
                | b"eXIf"
        );
        if (!known && chunk.kind[3].is_ascii_uppercase()) || chunk.kind == *b"caBX" {
            ensure!(
                original == candidate,
                "未知不可安全复制的 chunk 或文件签名：保留原文件"
            );
        }
    }
    Ok(())
}

pub fn strip_metadata(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut out = SIGNATURE.to_vec();
    for c in chunks(bytes)? {
        if !removable(&c.kind) {
            out.extend_from_slice(c.encoded);
        }
    }
    Ok(out)
}
