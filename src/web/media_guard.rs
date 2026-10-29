//! 浏览器图片输入先识别像素容器，避免把脚本、矢量或文档交给外部解释器。
use anyhow::{Result, bail};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub fn raster_extension(bytes: &[u8]) -> Result<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok("png");
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Ok("jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("gif");
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Ok("webp");
    }
    if bytes.starts_with(b"BM") {
        return Ok("bmp");
    }
    if [
        b"II\x2a\x00".as_slice(),
        b"MM\x00\x2a",
        b"II\x2b\x00",
        b"MM\x00\x2b",
    ]
    .iter()
    .any(|magic| bytes.starts_with(magic))
    {
        return Ok("tiff");
    }
    if bytes.starts_with(b"\x00\x00\x01\x00") {
        return Ok("ico");
    }
    // ISO BMFF 也用于视频；严格检查完整 ftyp box 及图像 brand，不能只看“ftyp”。
    if bytes.get(4..8) == Some(b"ftyp") && bytes.len() >= 16 {
        let size = u32::from_be_bytes(bytes[..4].try_into()?) as usize;
        if size >= 16 && size <= bytes.len() && size.is_multiple_of(4) {
            let brands = std::iter::once(&bytes[8..12]).chain(
                bytes[16..size]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|brand| brand.as_slice()),
            );
            let mut heif = false;
            for brand in brands {
                if brand == b"avif" || brand == b"avis" {
                    return Ok("avif");
                }
                heif |= [
                    b"heic".as_slice(),
                    b"heix",
                    b"hevc",
                    b"hevx",
                    b"heim",
                    b"heis",
                    b"hevm",
                    b"hevs",
                ]
                .contains(&brand);
            }
            if heif {
                return Ok("heic");
            }
        }
    }
    bail!(
        "网页图片输入仅支持 PNG、JPEG、WebP、GIF、BMP、TIFF、ICO、AVIF、HEIC 像素容器；不接收 SVG、PDF 或脚本格式。更改扩展名不能改变文件内容。"
    )
}

/// 使用识别出的可信扩展名创建只读输入别名，避免 .mvg/.msl 等扩展名引导外部解释器。
pub fn image_alias(source: &Path, directory: &Path) -> Result<PathBuf> {
    let mut header = Vec::new();
    File::open(source)?.take(4096).read_to_end(&mut header)?;
    let extension = raster_extension(&header)?;
    let destination = directory.join(format!("input.{extension}"));
    // 同一会话文件系统中使用 hard link 不重复占据上传空间；领域命令绝不修改输入。
    std::fs::hard_link(source, &destination)?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renamed_scripts_and_documents_are_rejected_before_backend() {
        for bytes in [
            b"<svg xmlns='http://www.w3.org/2000/svg'/>".as_slice(),
            b"%PDF-1.7",
            b"push graphic-context",
            b"<?xml version='1.0'?><image/>",
        ] {
            assert!(raster_extension(bytes).is_err());
        }
    }
    #[test]
    fn normal_raster_signatures_and_exact_image_brands_are_recognized() {
        assert_eq!(raster_extension(b"\x89PNG\r\n\x1a\n").unwrap(), "png");
        assert_eq!(raster_extension(b"\xff\xd8\xff\xe0").unwrap(), "jpg");
        assert_eq!(raster_extension(b"RIFF0000WEBP").unwrap(), "webp");
        assert_eq!(
            raster_extension(b"\x00\x00\x00\x18ftypavif\0\0\0\0avifmif1").unwrap(),
            "avif"
        );
        assert!(raster_extension(b"\x00\x00\x00\x18ftypmp42\0\0\0\0mp42isom").is_err());
        assert!(raster_extension(b"\x00\x00\x00\x18ftypavif").is_err());
    }
}
