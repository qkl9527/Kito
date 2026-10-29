mod common;
use std::{fs, process::Command};
fn available() -> bool {
    let ok = Command::new("magick").arg("-version").output().is_ok();
    assert!(
        ok || std::env::var_os("KITO_REQUIRE_BACKEND").is_none(),
        "CI 必须安装 ImageMagick"
    );
    ok
}
#[test]
fn lossless_webp_roundtrip_keeps_hidden_rgb() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    let webp = dir.path().join("a.webp");
    let output = dir.path().join("back.png");
    fs::write(&input, common::rgba()).unwrap();
    let result = common::cli(&[
        "image",
        "convert",
        common::path(&input),
        "--to",
        "webp",
        "--lossless",
        "-o",
        common::path(&webp),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = common::cli(&[
        "image",
        "convert",
        common::path(&webp),
        "--to",
        "png",
        "-o",
        common::path(&output),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let image =
        kito::image::png::PngImage::decode(&fs::read(&output).unwrap(), 64 * 1024 * 1024).unwrap();
    assert_eq!(
        image.pixels,
        vec![3084, 8738, 14392, 0, 51400, 25700, 12850, 65535]
    );
}

#[test]
fn full_depth_sbit_allows_lossless_conversion() {
    if !available() {
        return;
    }
    let mut palette = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut palette, 2, 1);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::One);
        encoder.set_palette(vec![12, 34, 56, 200, 100, 50]);
        encoder.set_trns(vec![0, 255]);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[0b01000000])
            .unwrap();
    }
    // sBIT 描述样本精度，调色板项仍为 8-bit，不能误用 1-bit 索引深度。
    let cases = [
        (
            common::png_bytes(
                png::ColorType::Rgb,
                png::BitDepth::Eight,
                2,
                1,
                &[12, 34, 56, 200, 100, 50],
            ),
            vec![8, 8, 8],
            "webp",
        ),
        (common::rgba(), vec![8, 8, 8, 8], "webp"),
        (palette, vec![8, 8, 8], "webp"),
        (
            common::png_bytes(
                png::ColorType::Grayscale,
                png::BitDepth::One,
                2,
                1,
                &[0b01000000],
            ),
            vec![1],
            "tiff",
        ),
        (
            common::png_bytes(
                png::ColorType::GrayscaleAlpha,
                png::BitDepth::Eight,
                2,
                1,
                &[12, 0, 200, 255],
            ),
            vec![8, 8],
            "tiff",
        ),
        (
            common::png_bytes(
                png::ColorType::Rgb,
                png::BitDepth::Sixteen,
                1,
                1,
                &[0x12, 0x34, 0xab, 0xcd, 0, 1],
            ),
            vec![16, 16, 16],
            "tiff",
        ),
    ];
    for (index, (bytes, significant_bits, target)) in cases.into_iter().enumerate() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.png");
        let output = dir.path().join(format!("converted.{target}"));
        let decoded = dir.path().join("decoded.rgba");
        let source = common::add_chunk(&bytes, b"sBIT", &significant_bits);
        // WebP 正例与截图一样显式声明 sRGB，确保放行 sBIT 后仍走颜色校验。
        // TIFF 正例只添加 sBIT，以免混入 TIFF 对 sRGB rendering intent 的限制。
        let source = if target == "webp" {
            common::add_chunk(&source, b"sRGB", &[0])
        } else {
            source
        };
        fs::write(&input, &source).unwrap();
        let result = common::cli(&[
            "image",
            "convert",
            common::path(&input),
            "--to",
            target,
            "--lossless",
            "-o",
            common::path(&output),
            "--json",
        ]);
        assert!(
            result.status.success(),
            "case {index}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["conversion"]["verified_lossless"], true);
        // 从原 PNG 独立解码预期值，再检查目标的 RGBA16；包括透明 RGB 和 16-bit 低位。
        assert!(
            Command::new("magick")
                .arg(&output)
                .args(["-alpha", "on", "-depth", "16", "-endian", "MSB"])
                .arg(format!("RGBA:{}", decoded.display()))
                .status()
                .unwrap()
                .success()
        );
        let actual: Vec<u16> = fs::read(&decoded)
            .unwrap()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect();
        let expected = kito::image::png::PngImage::decode(&source, 64 * 1024 * 1024).unwrap();
        assert_eq!(actual, expected.pixels, "case {index}");
        assert_eq!(fs::read(input).unwrap(), source);
    }
}

#[test]
fn nontrivial_or_invalid_sbit_cannot_be_silently_discarded() {
    if !available() {
        return;
    }
    // 最后一个通道为 Alpha；也必须保留其原始精度声明。
    for bits in [&[8, 8, 8, 7][..], &[8, 0, 8, 8], &[8, 8, 8], &[9, 8, 8, 8]] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.png");
        let output = dir.path().join("output.webp");
        fs::write(&input, common::add_chunk(&common::rgba(), b"sBIT", bits)).unwrap();
        let result = common::cli(&[
            "image",
            "convert",
            common::path(&input),
            "--to",
            "webp",
            "--lossless",
            "-o",
            common::path(&output),
        ]);
        assert!(!result.status.success(), "sBIT {bits:?} 不得被静默丢弃");
        assert!(String::from_utf8_lossy(&result.stderr).contains("sBIT"));
        assert!(!output.exists());
    }
}
#[test]
fn jpeg_requires_lossy_consent_and_explicit_background() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    let output = dir.path().join("a.jpg");
    fs::write(&input, common::rgba()).unwrap();
    assert!(
        !common::cli(&[
            "image",
            "convert",
            common::path(&input),
            "--to",
            "jpeg",
            "-o",
            common::path(&output)
        ])
        .status
        .success()
    );
    assert!(!output.exists());
    assert!(
        !common::cli(&[
            "image",
            "convert",
            common::path(&input),
            "--to",
            "jpeg",
            "--allow-lossy",
            "--quality",
            "90",
            "-o",
            common::path(&output)
        ])
        .status
        .success()
    );
    let result = common::cli(&[
        "image",
        "convert",
        common::path(&input),
        "--to",
        "jpeg",
        "--allow-lossy",
        "--quality",
        "90",
        "--background",
        "white",
        "-o",
        common::path(&output),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fs::read(&output).unwrap().starts_with(&[0xff, 0xd8]));
}
#[test]
fn sixteen_bit_to_webp_cannot_silently_truncate() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    let output = dir.path().join("a.webp");
    fs::write(
        &input,
        common::png_bytes(
            png::ColorType::Rgb,
            png::BitDepth::Sixteen,
            1,
            1,
            &[0x12, 0x34, 0xab, 0xcd, 0x01, 0x23],
        ),
    )
    .unwrap();
    assert!(
        !common::cli(&[
            "image",
            "convert",
            common::path(&input),
            "--to",
            "webp",
            "-o",
            common::path(&output)
        ])
        .status
        .success()
    );
    assert!(!output.exists());
}
#[test]
fn animation_requires_explicit_frame_selection() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.gif");
    let output = dir.path().join("a.png");
    assert!(
        Command::new("magick")
            .args(["-size", "2x2", "xc:red", "xc:blue"])
            .arg(&input)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        !common::cli(&[
            "image",
            "convert",
            common::path(&input),
            "--to",
            "png",
            "-o",
            common::path(&output)
        ])
        .status
        .success()
    );
    assert!(!output.exists());
    let result = common::cli(&[
        "image",
        "convert",
        common::path(&input),
        "--to",
        "png",
        "--frame",
        "1",
        "-o",
        common::path(&output),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
fn missing_backend_has_actionable_error_without_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    let output = dir.path().join("a.webp");
    fs::write(&input, common::rgba()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_kito"))
        .env("KITO_MAGICK", dir.path().join("missing-executable"))
        .args([
            "image",
            "convert",
            common::path(&input),
            "--to",
            "webp",
            "-o",
            common::path(&output),
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("KITO_MAGICK"));
    assert!(!output.exists());
}

#[test]
fn opaque_alpha_and_extension_dependent_formats_roundtrip() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("opaque.png");
    let bytes = common::png_bytes(
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        2,
        1,
        &[12, 34, 56, 255, 200, 100, 50, 255],
    );
    fs::write(&input, &bytes).unwrap();
    for format in ["bmp", "ico", "tga"] {
        let out = dir.path().join(format!("image.{format}"));
        let back = dir.path().join(format!("{format}.png"));
        let r = common::cli(&[
            "image",
            "convert",
            common::path(&input),
            "--to",
            format,
            "-o",
            common::path(&out),
        ]);
        assert!(
            r.status.success(),
            "{format}: {}",
            String::from_utf8_lossy(&r.stderr)
        );
        let r = common::cli(&[
            "image",
            "convert",
            common::path(&out),
            "--to",
            "png",
            "-o",
            common::path(&back),
        ]);
        assert!(
            r.status.success(),
            "{format}: {}",
            String::from_utf8_lossy(&r.stderr)
        );
        let actual =
            kito::image::png::PngImage::decode(&fs::read(back).unwrap(), 64 * 1024 * 1024).unwrap();
        assert_eq!(
            actual.pixels,
            vec![3084, 8738, 14392, 65535, 51400, 25700, 12850, 65535]
        );
    }
}

#[test]
fn same_format_png_conversion_preserves_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    let output = dir.path().join("b.png");
    let bytes = common::add_chunk(&common::rgba(), b"tEXt", b"Comment\0keep");
    fs::write(&input, &bytes).unwrap();
    let r = common::cli(&[
        "image",
        "convert",
        common::path(&input),
        "--to",
        "png",
        "-o",
        common::path(&output),
    ]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    kito::image::png::equivalent(&bytes, &fs::read(output).unwrap(), false, 64 * 1024 * 1024)
        .unwrap();
}

#[test]
fn lossless_conversion_rejects_lost_pixel_aspect_ratio() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("aspect.png");
    let out = dir.path().join("aspect.webp");
    let mut physical = 300u32.to_be_bytes().to_vec();
    physical.extend_from_slice(&100u32.to_be_bytes());
    physical.push(0);
    fs::write(
        &input,
        common::add_chunk(&common::rgba(), b"pHYs", &physical),
    )
    .unwrap();
    let r = common::cli(&[
        "image",
        "convert",
        common::path(&input),
        "--to",
        "webp",
        "-o",
        common::path(&out),
    ]);
    assert!(!r.status.success(), "不能丢失非方形像素信息后宣称无损");
    assert!(!out.exists());
}

#[test]
fn half_float_tiff_cannot_be_certified_after_clipping() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("float.tiff");
    let out = dir.path().join("out.png");
    assert!(
        Command::new("magick")
            .args([
                "-size",
                "1x1",
                "xc:red",
                "-evaluate",
                "multiply",
                "2",
                "-define",
                "quantum:format=floating-point",
                "-depth",
                "16"
            ])
            .arg(&input)
            .status()
            .unwrap()
            .success()
    );
    let r = common::cli(&[
        "image",
        "convert",
        common::path(&input),
        "--to",
        "png",
        "-o",
        common::path(&out),
    ]);
    assert!(!r.status.success(), "浮点高光不能被截断后宣称无损");
    assert!(!out.exists());
}

#[test]
fn apng_fallback_cannot_masquerade_as_animation_frame_zero() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("animated.png");
    let out = dir.path().join("out.png");
    let red = common::png_bytes(
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        1,
        1,
        &[255, 0, 0, 255],
    );
    let blue = common::png_bytes(
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        1,
        1,
        &[0, 0, 255, 255],
    );
    let mut actl = 1u32.to_be_bytes().to_vec();
    actl.extend_from_slice(&0u32.to_be_bytes());
    let fallback = common::add_chunk(&red, b"acTL", &actl);
    let mut fctl = Vec::new();
    for number in [0u32, 1, 1, 0, 0] {
        fctl.extend_from_slice(&number.to_be_bytes());
    }
    fctl.extend_from_slice(&[0, 1, 0, 10, 0, 0]);
    let fallback = common::before_iend(&fallback, b"fcTL", &fctl);
    let idat = kito::image::png::chunks(&blue)
        .unwrap()
        .into_iter()
        .find(|c| c.kind == *b"IDAT")
        .unwrap()
        .data
        .to_vec();
    let mut fdat = 1u32.to_be_bytes().to_vec();
    fdat.extend_from_slice(&idat);
    fs::write(&input, common::before_iend(&fallback, b"fdAT", &fdat)).unwrap();
    let r = common::cli(&[
        "image",
        "convert",
        common::path(&input),
        "--to",
        "png",
        "--frame",
        "0",
        "-o",
        common::path(&out),
    ]);
    assert!(
        !r.status.success(),
        "后端只识别 fallback 时必须拒绝而不是输出错误帧"
    );
    assert!(!out.exists());
}
