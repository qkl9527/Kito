use kito::backend;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

const BACKEND_VARIABLES: [&str; 4] = [
    "KITO_MAGICK",
    "KITO_ZOPFLIPNG",
    "CTK_MAGICK",
    "CTK_ZOPFLIPNG",
];

fn isolated_command(executable: &Path, directory: &Path, values: &[(&str, &str)]) -> Command {
    let mut command = Command::new(executable);
    // 环境只注入子进程；PATH 指向临时目录，测试结果与开发机安装和并发测试无关。
    command.current_dir(directory).env("PATH", directory);
    for name in BACKEND_VARIABLES {
        command.env_remove(name);
    }
    command.envs(values.iter().copied());
    command
}

fn doctor(directory: &Path, values: &[(&str, &str)]) -> Value {
    let output = isolated_command(Path::new(env!("CARGO_BIN_EXE_kito")), directory, values)
        .args(["image", "doctor", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn selected_programs(directory: &Path, values: &[(&str, &str)]) -> Value {
    let report = directory.join("selected-programs.json");
    let output = isolated_command(&std::env::current_exe().unwrap(), directory, values)
        .args(["--exact", "backend_selection_child"])
        .env("KITO_TEST_SELECTION_REPORT", &report)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&fs::read(report).unwrap()).unwrap()
}

/// 在独立测试子进程中调用公开 path helpers，避免用 unsafe 修改并行测试的环境。
#[test]
fn backend_selection_child() {
    let Some(report) = std::env::var_os("KITO_TEST_SELECTION_REPORT") else {
        return;
    };
    let programs = serde_json::json!({
        "magick": backend::magick().to_string_lossy(),
        "zopflipng": backend::zopflipng().to_string_lossy(),
    });
    fs::write(report, serde_json::to_vec(&programs).unwrap()).unwrap();
}

fn assert_configuration(
    values: &[(&str, &str)],
    expected_magick: &str,
    expected_zopflipng: &str,
    expected_prefix: &str,
) {
    let directory = tempfile::tempdir().unwrap();
    let selected = selected_programs(directory.path(), values);
    assert_eq!(selected["magick"], expected_magick);
    assert_eq!(selected["zopflipng"], expected_zopflipng);

    // doctor 走 run_* 路径，诊断必须与公开 path helpers 选中的配置来源一致。
    let report = doctor(directory.path(), values);
    for (backend, suffix) in [("magick", "MAGICK"), ("zopflipng", "ZOPFLIPNG")] {
        let error = report[backend]["Err"].as_str().unwrap();
        assert!(
            error.contains(&format!("{expected_prefix}_{suffix}")),
            "{error}"
        );
        let unselected_prefix = if expected_prefix == "KITO" {
            "CTK"
        } else {
            "KITO"
        };
        assert!(
            !error.contains(&format!("{unselected_prefix}_{suffix}")),
            "{error}"
        );
        assert!(
            !error.contains("跳过"),
            "显式配置错误必须保留为错误：{error}"
        );
    }
}

#[test]
fn primary_variables_are_used_before_legacy_variables() {
    assert_configuration(
        &[
            ("KITO_MAGICK", "missing-kito-magick"),
            ("KITO_ZOPFLIPNG", "missing-kito-zopflipng"),
            ("CTK_MAGICK", "missing-ctk-magick"),
            ("CTK_ZOPFLIPNG", "missing-ctk-zopflipng"),
        ],
        "missing-kito-magick",
        "missing-kito-zopflipng",
        "KITO",
    );
}

#[test]
fn primary_variables_work_without_legacy_configuration() {
    assert_configuration(
        &[
            ("KITO_MAGICK", "missing-kito-magick"),
            ("KITO_ZOPFLIPNG", "missing-kito-zopflipng"),
        ],
        "missing-kito-magick",
        "missing-kito-zopflipng",
        "KITO",
    );
}

#[test]
fn empty_primary_variables_do_not_fall_back_to_legacy_configuration() {
    assert_configuration(
        &[
            ("KITO_MAGICK", ""),
            ("KITO_ZOPFLIPNG", ""),
            ("CTK_MAGICK", "missing-ctk-magick"),
            ("CTK_ZOPFLIPNG", "missing-ctk-zopflipng"),
        ],
        "",
        "",
        "KITO",
    );
}

#[test]
fn legacy_variables_remain_supported_including_empty_values() {
    for value in ["missing-legacy-backend", ""] {
        assert_configuration(
            &[("CTK_MAGICK", value), ("CTK_ZOPFLIPNG", value)],
            value,
            value,
            "CTK",
        );
    }
}

#[test]
fn default_missing_optional_backend_is_still_skipped() {
    let directory = tempfile::tempdir().unwrap();
    let selected = selected_programs(directory.path(), &[]);
    assert_eq!(selected["magick"], "magick");
    assert_eq!(selected["zopflipng"], "zopflipng");
    let report = doctor(directory.path(), &[]);
    let magick = report["magick"]["Err"].as_str().unwrap();
    assert!(magick.contains("KITO_MAGICK"));
    assert!(!magick.contains("CTK_MAGICK"));
    assert!(
        report["zopflipng"]["Err"]
            .as_str()
            .unwrap()
            .contains("跳过")
    );
}

#[cfg(unix)]
#[test]
fn version_helpers_execute_the_selected_primary_or_legacy_program() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let primary = directory.path().join("primary-backend");
    let legacy = directory.path().join("legacy-backend");
    for (path, marker) in [(&primary, "primary-version"), (&legacy, "legacy-version")] {
        fs::write(path, format!("#!/bin/sh\necho {marker}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let primary = primary.to_str().unwrap();
    let legacy = legacy.to_str().unwrap();
    for (values, expected) in [
        (
            vec![
                ("KITO_MAGICK", primary),
                ("KITO_ZOPFLIPNG", primary),
                ("CTK_MAGICK", legacy),
                ("CTK_ZOPFLIPNG", legacy),
            ],
            "primary-version",
        ),
        (
            vec![("CTK_MAGICK", legacy), ("CTK_ZOPFLIPNG", legacy)],
            "legacy-version",
        ),
    ] {
        let report = doctor(directory.path(), &values);
        assert_eq!(report["magick"]["Ok"], expected);
        assert_eq!(report["zopflipng"]["Ok"], expected);
    }
}
