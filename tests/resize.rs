mod common;

use kito::image::png::PngImage;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn available() -> bool {
    let ok = Command::new("magick")
        .arg("-version")
        .output()
        .is_ok_and(|result| result.status.success());
    assert!(
        ok || std::env::var_os("KITO_REQUIRE_BACKEND").is_none(),
        "CI 必须安装 ImageMagick"
    );
    ok
}

/// 非均匀的 8×4 输入既能暴露等比计算错误，也让放大后文件确实大于原文件。
fn source() -> Vec<u8> {
    let pixels: Vec<u8> = (0..32u8)
        .flat_map(|i| {
            [
                i.wrapping_mul(73),
                i.wrapping_mul(139),
                i.wrapping_mul(197),
                255,
            ]
        })
        .collect();
    common::png_bytes(png::ColorType::Rgba, png::BitDepth::Eight, 8, 4, &pixels)
}

fn run_json(args: &[&str]) -> Value {
    let result = common::cli(args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

fn decoded(path: &Path) -> PngImage {
    PngImage::decode(&fs::read(path).unwrap(), 64 * 1024 * 1024).unwrap()
}

fn dimensions(path: &Path, expected: (u32, u32)) {
    let image = decoded(path);
    assert_eq!((image.width, image.height), expected);
}

fn resize_report(report: &Value, requested: &str, expected: (u32, u32)) {
    assert_eq!(report["source_width"], 8);
    assert_eq!(report["source_height"], 4);
    assert_eq!(report["width"], expected.0);
    assert_eq!(report["height"], expected.1);
    assert_eq!(report["requested"], requested);
    assert_eq!(report["changed"], expected != (8, 4));
    assert!(report["filter"].as_str().is_some_and(|s| !s.is_empty()));
}

#[test]
fn resize_infers_actual_format_and_uses_a_distinct_default_name() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // 扩展名故意不声明格式，要求按已识别的 PNG 内容选择输出格式。
    let input = dir.path().join("中文 图片.data");
    let bytes = source();
    fs::write(&input, &bytes).unwrap();
    let report = run_json(&[
        "image",
        "resize",
        common::path(&input),
        "--resize",
        "4x",
        "--json",
    ]);
    let output = dir.path().join("中文 图片.resized.png");
    dimensions(&output, (4, 2));
    assert_eq!(report["output"], common::path(&output));
    resize_report(&report["conversion"]["resize"], "4x", (4, 2));
    assert_eq!(report["conversion"]["verified_lossless"], false);
    assert_eq!(report["conversion"]["encoding_verified_lossless"], true);
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[test]
fn geometry_preserves_aspect_ratio_and_allows_percentage_enlargement() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    fs::write(&input, source()).unwrap();
    // 固定预期独立于生产代码：8×4 放入 6×6 应为 6×3，不能拉伸成正方形。
    for (index, (requested, expected)) in [
        ("6x6", (6, 3)),
        ("x3", (6, 3)),
        ("50%", (4, 2)),
        ("150%", (12, 6)),
        ("100%", (8, 4)),
    ]
    .into_iter()
    .enumerate()
    {
        let output = dir.path().join(format!("case-{index}.png"));
        let report = run_json(&[
            "image",
            "resize",
            common::path(&input),
            "--resize",
            requested,
            "-o",
            common::path(&output),
            "--json",
        ]);
        dimensions(&output, expected);
        resize_report(&report["conversion"]["resize"], requested, expected);
        assert_eq!(
            report["conversion"]["verified_lossless"],
            expected == (8, 4)
        );
        assert_eq!(report["conversion"]["encoding_verified_lossless"], true);
    }
}

#[test]
fn convert_resize_keeps_constant_sixteen_bit_samples_and_transparency() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // 恒定色块的预期通道可直接给出；无需调用被测缩放算法求期望值。
    let cases = [
        (
            png::BitDepth::Sixteen,
            [0x12, 0x34, 0xab, 0xcd, 0x01, 0x23, 0x80, 0x00].repeat(8),
            [0x1234, 0xabcd, 0x0123, 0x8000].repeat(2),
        ),
        (
            png::BitDepth::Eight,
            [12, 34, 56, 0].repeat(8),
            [3084, 8738, 14392, 0].repeat(2),
        ),
    ];
    for (index, (depth, pixels, expected)) in cases.into_iter().enumerate() {
        let input = dir.path().join(format!("source-{index}.png"));
        let output = dir.path().join(format!("converted-{index}.png"));
        let bytes = common::png_bytes(png::ColorType::Rgba, depth, 4, 2, &pixels);
        fs::write(&input, &bytes).unwrap();
        let report = run_json(&[
            "image",
            "convert",
            common::path(&input),
            "--to",
            "png",
            "--resize",
            "50%",
            "-o",
            common::path(&output),
            "--json",
        ]);
        dimensions(&output, (2, 1));
        assert_eq!(decoded(&output).pixels, expected);
        assert_eq!(report["conversion"]["verified_lossless"], false);
        assert_eq!(report["conversion"]["encoding_verified_lossless"], true);
        assert_eq!(report["conversion"]["resize"]["source_width"], 4);
        assert_eq!(report["conversion"]["resize"]["source_height"], 2);
        assert_eq!(fs::read(&input).unwrap(), bytes);
    }
}

#[test]
fn hidden_rgb_recovery_does_not_pollute_visible_alpha_weighted_edges() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("edge.png");
    let output = dir.path().join("output.png");
    let reference = dir.path().join("normal.miff");
    let raw = dir.path().join("normal.rgba");
    let pixels: Vec<u8> = (0..8)
        .flat_map(|_| {
            (0..32).flat_map(|x| {
                if x < 24 {
                    [12, 34, 56, 0]
                } else {
                    [180, 90, 30, 255]
                }
            })
        })
        .collect();
    fs::write(
        &input,
        common::png_bytes(png::ColorType::Rgba, png::BitDepth::Eight, 32, 8, &pixels),
    )
    .unwrap();
    run_json(&[
        "image",
        "resize",
        common::path(&input),
        "--resize",
        "50%",
        "-o",
        common::path(&output),
        "--json",
    ]);
    // 独立运行正常 Alpha 加权缩放作为可见边缘参考；它不包含被测的隐藏 RGB
    // 修复逻辑。若错误地对整图做未加权缩放，可见边缘就会混入透明背景颜色。
    assert!(
        Command::new("magick")
            .arg(&input)
            .args(["-filter", "Lanczos", "-resize", "16x4!", "-depth", "8"])
            .arg(&reference)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("magick")
            .arg(&reference)
            .args(["-depth", "16", "-endian", "MSB"])
            .arg(format!("RGBA:{}", raw.display()))
            .status()
            .unwrap()
            .success()
    );
    let normal: Vec<u16> = fs::read(raw)
        .unwrap()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u16::from_be_bytes(*v))
        .collect();
    let actual = decoded(&output);
    assert_eq!((actual.width, actual.height), (16, 4));
    assert!(
        normal
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 0 && p[3] < 65535)
    );
    for (before, after) in normal
        .as_chunks::<4>()
        .0
        .iter()
        .zip(actual.pixels.as_chunks::<4>().0)
    {
        assert_eq!(before[3], after[3]);
        if before[3] > 0 {
            assert_eq!(before, after);
        }
    }
    // 远离可见边缘的恒定透明区仍应为原本的隐藏颜色，不得被归零。
    assert_eq!(&actual.pixels[..4], &[3084, 8738, 14392, 0]);
}

#[test]
fn compress_enlarges_even_when_the_result_is_larger_and_reports_signed_savings() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    let output = dir.path().join("larger.png");
    let bytes = source();
    fs::write(&input, &bytes).unwrap();
    let report = run_json(&[
        "image",
        "compress",
        common::path(&input),
        "--resize",
        "64x32",
        "--effort",
        "fast",
        "-o",
        common::path(&output),
        "--json",
    ]);
    dimensions(&output, (64, 32));
    let file = &report["files"][0];
    resize_report(&file["resize"], "64x32", (64, 32));
    assert_eq!(file["verified_lossless"], false);
    assert_eq!(file["encoding_verified_lossless"], true);
    let output_bytes = fs::read(&output).unwrap();
    assert!(output_bytes.len() > bytes.len());
    assert_eq!(
        file["saved_bytes"].as_i64().unwrap(),
        bytes.len() as i64 - output_bytes.len() as i64
    );
    assert!(file["saved_percent"].as_f64().unwrap() < 0.0);
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[test]
fn omitting_resize_keeps_existing_compress_and_convert_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    let bytes = source();
    fs::write(&input, &bytes).unwrap();
    for command in ["compress", "convert"] {
        let output = dir.path().join(format!("{command}.png"));
        let mut args = vec![
            "image",
            command,
            common::path(&input),
            "-o",
            common::path(&output),
            "--json",
        ];
        if command == "compress" {
            args.extend(["--effort", "fast"]);
        } else {
            args.extend(["--to", "png"]);
        }
        let report = run_json(&args);
        dimensions(&output, (8, 4));
        kito::image::png::equivalent(&bytes, &fs::read(&output).unwrap(), false, 64 * 1024 * 1024)
            .unwrap();
        if command == "convert" {
            assert_eq!(report["conversion"]["verified_lossless"], true);
        }
    }
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[test]
fn invalid_sizes_fail_before_publishing_an_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    let output = dir.path().join("output.png");
    let bytes = source();
    fs::write(&input, &bytes).unwrap();
    for size in ["0x4", "4x0", "x", "0%", "-10%", "4x4!", "4", "NaN%"] {
        let result = common::cli(&[
            "image",
            "resize",
            common::path(&input),
            &format!("--resize={size}"),
            "-o",
            common::path(&output),
        ]);
        assert!(!result.status.success(), "非法尺寸 {size} 必须失败");
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(!error.contains("unrecognized subcommand"), "{error}");
        assert!(!error.contains("unexpected argument"), "{error}");
        assert!(!output.exists());
    }
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[test]
fn oversized_resize_respects_the_output_memory_budget_for_all_entry_points() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    let output = dir.path().join("output.png");
    let bytes = source();
    fs::write(&input, &bytes).unwrap();
    for command in ["resize", "convert", "compress"] {
        let mut args = vec![
            "image",
            command,
            common::path(&input),
            "--resize",
            "100000x100000",
            "--memory-mib",
            "1",
            "-o",
            common::path(&output),
            "--json",
        ];
        if command == "convert" {
            args.extend(["--to", "png"]);
        }
        let result = common::cli(&args);
        assert!(!result.status.success(), "{command} 不能分配超预算输出");
        // compress 将逐文件错误放在 JSON 中，另两个入口经 stderr 返回错误。
        let diagnostic = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            diagnostic.contains("预算") || diagnostic.to_ascii_lowercase().contains("memory"),
            "{diagnostic}"
        );
        assert!(!output.exists());
    }
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[test]
fn resize_never_replaces_existing_outputs_or_input_aliases_implicitly() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    let output = dir.path().join("output.png");
    let alias = dir.path().join("alias.png");
    let bytes = source();
    fs::write(&input, &bytes).unwrap();
    fs::write(&output, b"KEEP EXISTING OUTPUT").unwrap();
    fs::hard_link(&input, &alias).unwrap();
    for target in [&output, &input, &alias] {
        let mut args = vec![
            "image",
            "resize",
            common::path(&input),
            "--resize",
            "50%",
            "-o",
            common::path(target),
        ];
        if target != &output {
            args.push("--overwrite");
        }
        let result = common::cli(&args);
        assert!(!result.status.success());
    }
    assert_eq!(fs::read(&input).unwrap(), bytes);
    assert_eq!(fs::read(&alias).unwrap(), bytes);
    assert_eq!(fs::read(&output).unwrap(), b"KEEP EXISTING OUTPUT");
    // 显式授权覆盖正常输出必须可用，避免负例仅因入口完全不可用而误通过。
    run_json(&[
        "image",
        "resize",
        common::path(&input),
        "--resize",
        "50%",
        "-o",
        common::path(&output),
        "--overwrite",
        "--json",
    ]);
    dimensions(&output, (4, 2));
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[test]
fn batch_compress_resizes_each_image_and_keeps_the_directory_structure() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source");
    let output = dir.path().join("dest");
    fs::create_dir_all(input.join("sub")).unwrap();
    let bytes = source();
    fs::write(input.join("a.png"), &bytes).unwrap();
    fs::write(input.join("sub/b.png"), &bytes).unwrap();
    let report = run_json(&[
        "image",
        "compress",
        common::path(&input),
        "--recursive",
        "--output-dir",
        common::path(&output),
        "--resize",
        "50%",
        "--effort",
        "fast",
        "--jobs",
        "2",
        "--json",
    ]);
    assert_eq!(report["succeeded"], 2);
    assert_eq!(report["failed"], 0);
    for path in ["a.png", "sub/b.png"] {
        dimensions(&output.join(path), (4, 2));
        assert_eq!(fs::read(input.join(path)).unwrap(), bytes);
    }
    for file in report["files"].as_array().unwrap() {
        resize_report(&file["resize"], "50%", (4, 2));
        assert_eq!(file["verified_lossless"], false);
        assert_eq!(file["encoding_verified_lossless"], true);
    }
}

#[test]
fn resize_infers_jpeg_from_output_but_still_requires_lossy_consent_and_quality() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    let output = dir.path().join("output.jpg");
    fs::write(&input, source()).unwrap();
    let base = [
        "image",
        "resize",
        common::path(&input),
        "--resize",
        "50%",
        "-o",
        common::path(&output),
        "--json",
    ];
    for extra in [&[][..], &["--allow-lossy"][..], &["--quality", "85"][..]] {
        let mut args = base.to_vec();
        args.extend(extra);
        assert!(!common::cli(&args).status.success());
        assert!(!output.exists());
    }
    let mut args = base.to_vec();
    args.extend(["--allow-lossy", "--quality", "85"]);
    let report = run_json(&args);
    assert!(fs::read(&output).unwrap().starts_with(&[0xff, 0xd8]));
    let info = run_json(&["image", "info", common::path(&output), "--json"]);
    assert_eq!(info[0]["width"], 4);
    assert_eq!(info[0]["height"], 2);
    assert_eq!(report["conversion"]["verified_lossless"], false);
    assert_eq!(report["conversion"]["encoding_verified_lossless"], false);
}

#[test]
fn resize_srgb_rgb_checkerboard_keeps_rendering_intent_when_pixels_become_gray() {
    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("srgb-checkerboard.png");
    let output = dir.path().join("resized.png");
    // 容器明确是 RGB，像素则只有黑白。后端可能据此选择 Gray 编码，
    // 但不能同时丢掉原来的 sRGB rendering intent 并导致严格验证失败。
    let pixels: Vec<u8> = [
        0, 255, 0, 255, 0, 255, 0, 255, 255, 0, 255, 0, 255, 0, 255, 0,
    ]
    .into_iter()
    .flat_map(|sample| [sample; 3])
    .collect();
    let bytes = common::add_chunk(
        &common::png_bytes(png::ColorType::Rgb, png::BitDepth::Eight, 8, 2, &pixels),
        b"sRGB",
        &[0],
    );
    fs::write(&input, &bytes).unwrap();
    let report = run_json(&[
        "image",
        "resize",
        common::path(&input),
        "--resize",
        "50%",
        "-o",
        common::path(&output),
        "--json",
    ]);
    dimensions(&output, (4, 1));
    assert_eq!(report["conversion"]["encoding_verified_lossless"], true);
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn inferred_non_png_format_uses_the_requested_memory_and_time_limits() {
    use std::os::unix::fs::PermissionsExt;

    if !available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.tiff");
    let default_output = dir.path().join("source.resized.tiff");
    let log = dir.path().join("magick-arguments");
    let wrapper = dir.path().join("record-magick.sh");
    // 高压缩率 TIFF 在磁盘上远小于 1 MiB，但解码工作集超过这一预算。
    assert!(
        Command::new("magick")
            .args([
                "-size",
                "800x800",
                "xc:white",
                "-depth",
                "8",
                "-compress",
                "Zip"
            ])
            .arg(&input)
            .status()
            .unwrap()
            .success()
    );
    let bytes = fs::read(&input).unwrap();
    assert!(bytes.len() < 1024 * 1024);
    // 包装器完整转发给真实 ImageMagick；只记录参数，以捕获格式推断阶段
    // 偷用固定 512 MiB 的问题。可选延迟用于证明首轮探测也受本次超时限制。
    fs::write(
        &wrapper,
        b"#!/bin/sh\nprintf '%s\\0' \"$@\" >> \"$KITO_TEST_MAGICK_LOG\"\nprintf '\\0' >> \"$KITO_TEST_MAGICK_LOG\"\nif [ \"${KITO_TEST_MAGICK_DELAY:-0}\" = 1 ]; then sleep 2; fi\nexec magick \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    for delay in [false, true] {
        fs::write(&log, []).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_kito"))
            .env("KITO_MAGICK", &wrapper)
            .env("KITO_TEST_MAGICK_LOG", &log)
            .env("KITO_TEST_MAGICK_DELAY", if delay { "1" } else { "0" })
            .args([
                "image",
                "resize",
                common::path(&input),
                "--resize",
                "50%",
                "--memory-mib",
                "1",
                "--time-limit",
                "1",
            ])
            .output()
            .unwrap();
        assert!(!result.status.success());
        let recorded = fs::read(&log).unwrap();
        let first_call: Vec<_> = recorded
            .split(|byte| *byte == 0)
            .take_while(|argument| !argument.is_empty())
            .map(|argument| std::str::from_utf8(argument).unwrap())
            .collect();
        assert_eq!(first_call.first(), Some(&"identify"));
        assert!(
            first_call
                .windows(3)
                .any(|args| args == ["-limit", "memory", "1048576"]),
            "格式推断必须继承 1 MiB 预算，实际参数：{first_call:?}"
        );
        if delay {
            let error = String::from_utf8_lossy(&result.stderr);
            assert!(
                error.contains("超时") || error.to_ascii_lowercase().contains("timed out"),
                "{error}"
            );
        }
        assert!(!default_output.exists());
        assert_eq!(fs::read(&input).unwrap(), bytes);
    }
}
