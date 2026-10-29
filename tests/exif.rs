mod common;

use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

/// 测试直接按 TIFF 目录格式构造已知元数据，不依赖 ImageMagick 或设备照片。
/// 目录在前、变长值在后，IFD0/IFD1 和 EXIF/GPS 子目录的标签可以独立断言。
fn reserve_ifd(bytes: &mut Vec<u8>, count: u16) -> usize {
    let offset = bytes.len();
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.resize(offset + 2 + usize::from(count) * 12 + 4, 0);
    offset
}

fn entry(
    bytes: &mut Vec<u8>,
    ifd: usize,
    index: usize,
    tag: u16,
    ty: u16,
    count: u32,
    data: &[u8],
) {
    let offset = ifd + 2 + index * 12;
    bytes[offset..offset + 2].copy_from_slice(&tag.to_le_bytes());
    bytes[offset + 2..offset + 4].copy_from_slice(&ty.to_le_bytes());
    bytes[offset + 4..offset + 8].copy_from_slice(&count.to_le_bytes());
    if data.len() <= 4 {
        bytes[offset + 8..offset + 8 + data.len()].copy_from_slice(data);
    } else {
        let data_offset = bytes.len() as u32;
        bytes[offset + 8..offset + 12].copy_from_slice(&data_offset.to_le_bytes());
        bytes.extend_from_slice(data);
        // TIFF 值按偶数字节对齐，便于也用标准解析器检查测试素材。
        if !bytes.len().is_multiple_of(2) {
            bytes.push(0);
        }
    }
}

fn rationals(values: &[(u32, u32)]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|(n, d)| n.to_le_bytes().into_iter().chain(d.to_le_bytes()))
        .collect()
}

fn tiff(make: &[u8], exposure_denominator: u32) -> Vec<u8> {
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    let primary = reserve_ifd(&mut bytes, 5);
    let camera = reserve_ifd(&mut bytes, 2);
    let gps = reserve_ifd(&mut bytes, 4);
    let thumbnail = reserve_ifd(&mut bytes, 1);

    entry(&mut bytes, primary, 0, 0x010f, 2, make.len() as u32, make);
    entry(&mut bytes, primary, 1, 0x0110, 2, 8, b"Model X\0");
    entry(&mut bytes, primary, 2, 0x0112, 3, 1, &6_u16.to_le_bytes());
    entry(
        &mut bytes,
        primary,
        3,
        0x8769,
        4,
        1,
        &(camera as u32).to_le_bytes(),
    );
    entry(
        &mut bytes,
        primary,
        4,
        0x8825,
        4,
        1,
        &(gps as u32).to_le_bytes(),
    );
    bytes[primary + 2 + 5 * 12..primary + 2 + 5 * 12 + 4]
        .copy_from_slice(&(thumbnail as u32).to_le_bytes());

    entry(
        &mut bytes,
        camera,
        0,
        0x829a,
        5,
        1,
        &rationals(&[(1, exposure_denominator)]),
    );
    entry(
        &mut bytes,
        camera,
        1,
        0x9003,
        2,
        20,
        b"2026:10:28 13:14:15\0",
    );
    entry(&mut bytes, gps, 0, 1, 2, 2, b"S\0");
    entry(
        &mut bytes,
        gps,
        1,
        2,
        5,
        3,
        &rationals(&[(37, 1), (46, 1), (30, 1)]),
    );
    entry(&mut bytes, gps, 2, 3, 2, 2, b"W\0");
    entry(
        &mut bytes,
        gps,
        3,
        4,
        5,
        3,
        &rationals(&[(122, 1), (25, 1), (9, 1)]),
    );
    entry(&mut bytes, thumbnail, 0, 0x010f, 2, 10, b"Thumbnail\0");
    bytes
}

fn jpeg(metadata: &[u8]) -> Vec<u8> {
    // 只检验 APP1 元数据读取，不把没有像素扫描段的最小容器当成可解码图片。
    let mut bytes = vec![0xff, 0xd8, 0xff, 0xe1];
    bytes.extend_from_slice(&((metadata.len() + 8) as u16).to_be_bytes());
    bytes.extend_from_slice(b"Exif\0\0");
    bytes.extend_from_slice(metadata);
    bytes.extend_from_slice(&[0xff, 0xd9]);
    bytes
}

fn webp(metadata: &[u8]) -> Vec<u8> {
    // 同样仅构造 RIFF/EXIF 元数据容器，不依赖有损编码器产生像素数据。
    let mut body = b"WEBPEXIF".to_vec();
    body.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    body.extend_from_slice(metadata);
    if !metadata.len().is_multiple_of(2) {
        body.push(0);
    }
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    bytes
}

fn json_report(input: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(["image", "exif", common::path(input), "--json"])
        // 即使系统已安装 ImageMagick，测试也能证明 EXIF 读取不依赖它。
        .env("KITO_MAGICK", input.with_file_name("missing-magick"))
        .env("CTK_MAGICK", input.with_file_name("missing-legacy-magick"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn field<'a>(report: &'a Value, ifd: u16, context: &str, tag: &str) -> &'a Value {
    report["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ifd"] == ifd && f["context"] == context && f["tag"] == tag)
        .unwrap_or_else(|| panic!("缺少 {ifd}/{context}/{tag}：{report}"))
}

#[test]
fn reads_tiff_tags_with_context_precision_and_thumbnail_identity() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("photo.tiff");
    fs::write(&input, tiff(b"Kito Camera\0", 125)).unwrap();
    let report = json_report(&input);
    assert_eq!(report["input"], common::path(&input));
    assert_eq!(report["has_exif"], true);
    let make = field(&report, 0, "Tiff", "Make");
    assert_eq!(make["tag_id"], 0x010f);
    assert_eq!(make["type"], "Ascii");
    assert_eq!(make["value"], json!([b"Kito Camera".to_vec()]));
    assert_eq!(
        field(&report, 1, "Tiff", "Make")["value"],
        json!([b"Thumbnail".to_vec()])
    );
    assert_eq!(
        field(&report, 0, "Tiff", "Orientation")["value"],
        json!([6])
    );
    assert_eq!(
        field(&report, 0, "Exif", "ExposureTime")["value"],
        json!([{"numerator":1,"denominator":125}])
    );
    assert!(
        field(&report, 0, "Exif", "DateTimeOriginal")["display_value"]
            .as_str()
            .unwrap()
            .contains("2026")
    );
    assert_eq!(
        field(&report, 0, "Gps", "GPSLatitude")["value"],
        json!([
            {"numerator":37,"denominator":1}, {"numerator":46,"denominator":1}, {"numerator":30,"denominator":1}
        ])
    );
    assert!(
        field(&report, 0, "Gps", "GPSLatitude")["display_value"]
            .as_str()
            .unwrap()
            .contains('S')
    );
    assert!(
        field(&report, 0, "Gps", "GPSLongitude")["display_value"]
            .as_str()
            .unwrap()
            .contains('W')
    );
}

#[test]
fn reads_common_containers_by_content_without_modifying_input() {
    let dir = tempfile::tempdir().unwrap();
    let metadata = tiff(b"Kito Camera\0", 125);
    let containers = [
        metadata.clone(),
        jpeg(&metadata),
        common::add_chunk(&common::rgba(), b"eXIf", &metadata),
        webp(&metadata),
    ];
    for (index, bytes) in containers.iter().enumerate() {
        // 扩展名故意不透露实际格式；文件名还覆盖中文与空格。
        let input = dir.path().join(format!("照片 {index}.data"));
        fs::write(&input, bytes).unwrap();
        let report = json_report(&input);
        assert_eq!(report["has_exif"], true);
        assert_eq!(
            field(&report, 0, "Exif", "ExposureTime")["value"],
            json!([{"numerator":1,"denominator":125}])
        );
        assert_eq!(fs::read(&input).unwrap(), *bytes);
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), containers.len());
}

#[test]
fn missing_or_empty_exif_is_a_successful_empty_report() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        common::rgba(),
        vec![0xff, 0xd8, 0xff, 0xd9],
        b"II\x2a\0\x08\0\0\0\0\0\0\0\0\0".to_vec(),
    ];
    for (index, bytes) in cases.iter().enumerate() {
        let input = dir.path().join(format!("empty-{index}.data"));
        fs::write(&input, bytes).unwrap();
        let report = json_report(&input);
        assert_eq!(report["has_exif"], false);
        assert_eq!(report["fields"], json!([]));
        let text = common::cli(&["image", "exif", common::path(&input)]);
        assert!(text.status.success());
        assert!(String::from_utf8_lossy(&text.stdout).contains("未发现 EXIF"));
    }
}

#[test]
fn damaged_metadata_is_an_error_not_an_empty_report() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = common::add_chunk(&common::rgba(), b"eXIf", b"II\x2a\0\xff\xff\xff\xff");
    let input = dir.path().join("damaged-exif.png");
    fs::write(&input, &bytes).unwrap();
    let output = common::cli(&["image", "exif", common::path(&input), "--json"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[test]
fn invalid_png_crc_and_truncated_png_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut bad_crc = common::add_chunk(&common::rgba(), b"eXIf", &tiff(b"Kito\0", 125));
    bad_crc[41] ^= 1; // 只破坏 eXIf 数据，不更新其 CRC。
    for (index, bytes) in [bad_crc, b"\x89PNG\r\n\x1a\n".to_vec()].iter().enumerate() {
        let input = dir.path().join(format!("broken-{index}.png"));
        fs::write(&input, bytes).unwrap();
        let output = common::cli(&["image", "exif", common::path(&input), "--json"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn unsupported_contents_and_missing_files_fail_without_partial_json() {
    let dir = tempfile::tempdir().unwrap();
    let unsupported = dir.path().join("pretend.jpg");
    fs::write(&unsupported, b"This is not an image").unwrap();
    for input in [unsupported, dir.path().join("missing.jpg")] {
        let output = common::cli(&["image", "exif", common::path(&input), "--json"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn memory_budget_is_enforced_before_metadata_parsing() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("large.tiff");
    let mut bytes = tiff(b"Kito\0", 125);
    bytes.resize(128 * 1024 + 1, 0);
    fs::write(&input, &bytes).unwrap();
    let output = common::cli(&[
        "image",
        "exif",
        common::path(&input),
        "--memory-mib",
        "1",
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("预算"));
    assert_eq!(fs::read(&input).unwrap(), bytes);
    assert!(
        !common::cli(&["image", "exif", common::path(&input), "--memory-mib", "0"])
            .status
            .success()
    );
}

#[test]
fn raw_ascii_bytes_and_zero_denominator_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("unusual.tiff");
    fs::write(&input, tiff(b"Camera\xff\0", 0)).unwrap();
    let report = json_report(&input);
    assert_eq!(
        field(&report, 0, "Tiff", "Make")["value"],
        json!([b"Camera\xff".to_vec()])
    );
    assert_eq!(
        field(&report, 0, "Exif", "ExposureTime")["value"],
        json!([{"numerator":1,"denominator":0}])
    );
}

#[test]
fn text_output_does_not_execute_control_characters_from_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("control.tiff");
    fs::write(&input, tiff(b"Camera\x1b[31m\nspoofed\0", 125)).unwrap();
    let output = common::cli(&["image", "exif", common::path(&input)]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Make"));
    assert!(text.contains("ExposureTime"));
    assert!(text.contains("spoofed"));
    assert!(!text.contains('\x1b'));
    assert!(!text.lines().any(|line| line.starts_with("spoofed")));
}

#[cfg(unix)]
#[test]
fn text_output_escapes_control_characters_in_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("photo\x1b[31m\nforged.tiff");
    fs::write(&input, tiff(b"Kito\0", 125)).unwrap();
    let output = common::cli(&["image", "exif", common::path(&input)]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains('\x1b'));
    assert!(!text.lines().any(|line| line.starts_with("forged")));
}

fn assert_metadata_error(bytes: &[u8], expected: &[&str]) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("invalid.data");
    fs::write(&input, bytes).unwrap();
    let output = common::cli(&["image", "exif", common::path(&input), "--json"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        expected.iter().any(|needle| message.contains(needle)),
        "{message}"
    );
}

#[test]
fn repeated_subdirectory_pointer_is_rejected() {
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    let primary = reserve_ifd(&mut bytes, 2);
    let camera = reserve_ifd(&mut bytes, 1);
    for index in 0..2 {
        entry(
            &mut bytes,
            primary,
            index,
            0x8769,
            4,
            1,
            &(camera as u32).to_le_bytes(),
        );
    }
    entry(&mut bytes, camera, 0, 0x829a, 5, 1, &rationals(&[(1, 125)]));
    assert_metadata_error(&bytes, &["重复"]);
}

#[test]
fn excessive_field_count_is_rejected_before_values_are_expanded() {
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    let primary = reserve_ifd(&mut bytes, 4001);
    for index in 0..4001 {
        entry(
            &mut bytes,
            primary,
            index,
            0x0100,
            3,
            1,
            &1_u16.to_le_bytes(),
        );
    }
    assert_metadata_error(&bytes, &["预算", "上限"]);
}

#[test]
fn shared_value_offsets_cannot_bypass_expanded_data_budget() {
    // TIFF 允许多个标签引用同一段数据；读取时逐字段物化会放大工作集。
    // 文件只有约 100 KiB，但 32 个字段合计 3.2 MB，超过默认 2 MiB 字段预算。
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    let primary = reserve_ifd(&mut bytes, 32);
    let shared_offset = bytes.len() as u32;
    for index in 0..32 {
        let offset = primary + 2 + index * 12;
        bytes[offset..offset + 2].copy_from_slice(&(0xc000_u16 + index as u16).to_le_bytes());
        bytes[offset + 2..offset + 4].copy_from_slice(&7_u16.to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&100_000_u32.to_le_bytes());
        bytes[offset + 8..offset + 12].copy_from_slice(&shared_offset.to_le_bytes());
    }
    bytes.resize(bytes.len() + 100_000, 7);
    assert_metadata_error(&bytes, &["预算", "上限"]);
}

#[test]
fn referenced_unit_values_cannot_bypass_display_budget() {
    // 1000 个分辨率字段共享一个异常类型的单位字段。源字段合计仅 4000
    // 字节，满足 1 MiB 预算下的 4096 字节预检；with_unit 却会为每个
    // 分辨率重复格式化整段单位值。必须在格式化过程中累计限制输出，
    // 不能先 to_string 再检查，也不能在超限时留下部分 JSON。
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    let primary = reserve_ifd(&mut bytes, 1001);
    for index in 0..1000 {
        entry(
            &mut bytes,
            primary,
            index,
            0x011a,
            3,
            1,
            &72_u16.to_le_bytes(),
        );
    }
    entry(&mut bytes, primary, 1000, 0x0128, 2, 2000, &[b'A'; 2000]);

    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("repeated-unit.tiff");
    fs::write(&input, &bytes).unwrap();
    let output = common::cli(&[
        "image",
        "exif",
        common::path(&input),
        "--memory-mib",
        "1",
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("预算"));
}

#[test]
fn malformed_webp_lengths_padding_and_duplicate_exif_are_rejected() {
    let metadata = tiff(b"Kito\0", 125);
    let mut bad_length = webp(&metadata);
    let bad_declared_length = bad_length.len() as u32 - 6;
    bad_length[4..8].copy_from_slice(&bad_declared_length.to_le_bytes());

    let mut odd_metadata = metadata.clone();
    odd_metadata.push(0);
    let mut no_padding = webp(&odd_metadata);
    no_padding.pop();
    let no_padding_len = no_padding.len() as u32 - 8;
    no_padding[4..8].copy_from_slice(&no_padding_len.to_le_bytes());

    let mut duplicate = webp(&metadata);
    let repeated_chunk = duplicate[12..].to_vec();
    duplicate.extend_from_slice(&repeated_chunk);
    let duplicate_len = duplicate.len() as u32 - 8;
    duplicate[4..8].copy_from_slice(&duplicate_len.to_le_bytes());

    for bytes in [bad_length, no_padding, duplicate] {
        assert_metadata_error(&bytes, &["WebP", "RIFF", "EXIF", "长度", "填充", "重复"]);
    }
}
