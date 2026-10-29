mod common;
use std::fs;
#[test]
fn default_output_and_json_preserve_original() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("中文 空格.png");
    fs::write(&input, common::rgba()).unwrap();
    let result = common::cli(&[
        "image",
        "compress",
        common::path(&input),
        "--effort",
        "fast",
        "--json",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["failed"], 0);
    assert!(dir.path().join("中文 空格.optimized.png").exists());
    assert_eq!(fs::read(&input).unwrap(), common::rgba());
}
#[test]
fn refuses_existing_output_and_input_alias() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.png");
    let out = dir.path().join("out.png");
    fs::write(&input, common::rgba()).unwrap();
    fs::write(&out, b"KEEP").unwrap();
    assert!(
        !common::cli(&[
            "image",
            "compress",
            common::path(&input),
            "-o",
            common::path(&out),
            "--effort",
            "fast"
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(&out).unwrap(), b"KEEP");
    assert!(
        !common::cli(&[
            "image",
            "compress",
            common::path(&input),
            "-o",
            common::path(&input),
            "--overwrite"
        ])
        .status
        .success()
    );
    let alias = dir.path().join("alias.png");
    fs::hard_link(&input, &alias).unwrap();
    assert!(
        !common::cli(&[
            "image",
            "compress",
            common::path(&input),
            "-o",
            common::path(&alias),
            "--overwrite"
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(&input).unwrap(), common::rgba());
}
#[test]
fn batch_keeps_structure_and_reports_partial_failure() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source");
    let out = dir.path().join("dest");
    fs::create_dir_all(input.join("sub")).unwrap();
    fs::write(input.join("a.png"), common::rgba()).unwrap();
    fs::write(input.join("sub/b.png"), common::rgba()).unwrap();
    fs::write(input.join("broken.png"), b"bad").unwrap();
    let result = common::cli(&[
        "image",
        "compress",
        common::path(&input),
        "--recursive",
        "--output-dir",
        common::path(&out),
        "--effort",
        "fast",
        "--json",
    ]);
    assert!(!result.status.success());
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["failed"], 1);
    assert_eq!(report["succeeded"], 2);
    assert!(out.join("a.png").exists() && out.join("sub/b.png").exists());
}
#[test]
fn in_place_is_explicit_and_info_is_machine_readable() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("-input.png");
    fs::write(&input, common::rgba()).unwrap();
    assert!(
        common::cli(&[
            "image",
            "compress",
            common::path(&input),
            "--in-place",
            "--effort",
            "fast"
        ])
        .status
        .success()
    );
    let result = common::cli(&["image", "info", common::path(&input), "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["width"], 2);
}

#[test]
fn batch_output_may_be_the_input_parent() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("a.png"), common::rgba()).unwrap();
    let r = common::cli(&[
        "image",
        "compress",
        common::path(&input),
        "--output-dir",
        common::path(dir.path()),
        "--effort",
        "fast",
    ]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(dir.path().join("a.png").exists());
}
