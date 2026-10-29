mod common;

use kito::image::png;
use serde_json::Value;
use std::{
    ffi::OsStr,
    fs,
    path::PathBuf,
    process::{Command, Output},
};

struct Fixture {
    dir: tempfile::TempDir,
    input: PathBuf,
    output: PathBuf,
    original: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.png");
        let output = dir.path().join("output.png");
        let original = common::rgba();
        fs::write(&input, &original).unwrap();
        Self {
            dir,
            input,
            output,
            original,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kito"));
        // 仅隔离子进程的 PATH，避免依赖开发机安装情况或并行修改全局环境。
        command
            .current_dir(self.dir.path())
            .env("PATH", self.dir.path())
            .env_remove("KITO_ZOPFLIPNG")
            .env_remove("CTK_ZOPFLIPNG");
        command
    }

    fn compress(&self, backend: Option<&OsStr>, json: bool) -> Output {
        let mut command = self.command();
        command
            .args(["image", "compress"])
            .arg(&self.input)
            .arg("--output")
            .arg(&self.output)
            .arg("--overwrite");
        if let Some(backend) = backend {
            command.env("KITO_ZOPFLIPNG", backend);
        }
        if json {
            command.arg("--json");
        }
        command.output().unwrap()
    }

    fn report(&self, backend: Option<&OsStr>) -> Value {
        let output = self.compress(backend, true);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["failed"], 0);
        assert_eq!(report["files"][0]["success"], true);
        // 可选后端的诊断不能改变已验证结果，或覆盖输入文件。
        let optimized = fs::read(&self.output).unwrap();
        assert!(optimized.len() <= self.original.len());
        png::equivalent(&self.original, &optimized, false, 1024 * 1024).unwrap();
        assert_eq!(fs::read(&self.input).unwrap(), self.original);
        report["files"][0].clone()
    }
}

fn zopfli_trial(trials: &Value) -> &Value {
    trials
        .as_array()
        .unwrap()
        .iter()
        .find(|trial| trial["engine"] == "zopflipng-m")
        .expect("默认 max 必须记录可选后端的执行或跳过")
}

#[test]
fn absent_default_backend_is_a_note_and_does_not_invalidate_compression() {
    let fixture = Fixture::new();
    let report = fixture.report(None);
    assert!(report["warnings"].as_array().unwrap().is_empty());
    assert!(report["notes"][0].as_str().unwrap().contains("跳过"));
    assert_eq!(report["search_complete"], true);
    let trial = zopfli_trial(&report["trials"]);
    assert_eq!(trial["skipped"], true);
    assert_eq!(trial["valid"], false);
    assert_eq!(trial["bytes"], Value::Null);

    let text = fixture.compress(None, false);
    assert!(text.status.success());
    assert!(text.stderr.is_empty());
    assert!(String::from_utf8_lossy(&text.stdout).contains("跳过"));
}

#[test]
fn explicit_missing_or_empty_backend_remains_a_configuration_warning() {
    let fixture = Fixture::new();
    let nonexistent = fixture.dir.path().join("configured-but-missing");
    for program in [nonexistent.as_os_str(), OsStr::new("")] {
        let report = fixture.report(Some(program));
        let warning = report["warnings"][0].as_str().unwrap();
        assert!(warning.contains("KITO_ZOPFLIPNG"));
        assert!(!warning.contains("KITO_MAGICK"));
        assert!(report["notes"].as_array().unwrap().is_empty());
        let trial = zopfli_trial(&report["trials"]);
        assert_eq!(trial["skipped"], false);
        assert_eq!(trial["valid"], false);
    }
}

#[test]
fn skipped_backend_is_not_counted_as_a_valid_benchmark_baseline() {
    let fixture = Fixture::new();
    let output = fixture
        .command()
        .args(["image", "benchmark"])
        .arg(&fixture.input)
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["comparison_complete"], false);
    let trial = zopfli_trial(&report["files"][0]["baselines"]);
    assert_eq!(trial["skipped"], true);
    assert_eq!(trial["valid"], false);
    assert_eq!(trial["bytes"], Value::Null);
    let aggregate = zopfli_trial(&report["baselines"]);
    assert_eq!(aggregate["valid_files"], 0);
}

#[test]
fn fast_effort_does_not_attempt_optional_backend() {
    let fixture = Fixture::new();
    let output = fixture
        .command()
        .args(["image", "compress"])
        .arg(&fixture.input)
        .args(["--effort", "fast", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let file = &report["files"][0];
    assert!(file["warnings"].as_array().unwrap().is_empty());
    assert!(file["notes"].as_array().unwrap().is_empty());
    assert!(
        file["trials"]
            .as_array()
            .unwrap()
            .iter()
            .all(|trial| trial["engine"] != "zopflipng-m")
    );
}

#[cfg(unix)]
fn executable(fixture: &Fixture, body: &str, mode: u32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let program = fixture.dir.path().join("zopflipng");
    fs::write(&program, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(mode)).unwrap();
    program
}

#[cfg(unix)]
#[test]
fn backend_execution_failure_preserves_its_diagnostic() {
    let fixture = Fixture::new();
    let program = executable(&fixture, "echo backend-test-failure >&2\nexit 7", 0o755);
    let report = fixture.report(Some(program.as_os_str()));
    assert!(
        report["warnings"][0]
            .as_str()
            .unwrap()
            .contains("backend-test-failure")
    );
    assert_eq!(zopfli_trial(&report["trials"])["skipped"], false);
    assert_eq!(zopfli_trial(&report["trials"])["valid"], false);
}

#[cfg(unix)]
#[test]
fn successful_process_without_output_is_a_failure_not_a_missing_backend() {
    let fixture = Fixture::new();
    executable(&fixture, "exit 0", 0o755);
    let report = fixture.report(None);
    assert_eq!(report["warnings"].as_array().unwrap().len(), 1);
    assert!(report["notes"].as_array().unwrap().is_empty());
    let trial = zopfli_trial(&report["trials"]);
    assert_eq!(trial["skipped"], false);
    assert_eq!(trial["valid"], false);
    assert_eq!(trial["bytes"], Value::Null);
}

#[cfg(unix)]
#[test]
fn non_executable_default_backend_is_a_warning() {
    let fixture = Fixture::new();
    executable(&fixture, "exit 0", 0o644);
    let report = fixture.report(None);
    assert_eq!(report["warnings"].as_array().unwrap().len(), 1);
    assert!(report["notes"].as_array().unwrap().is_empty());
    assert_eq!(zopfli_trial(&report["trials"])["skipped"], false);
}

#[cfg(unix)]
#[test]
fn available_backend_candidate_is_validated_and_counted() {
    let fixture = Fixture::new();
    // 使用真实子进程写入候选；前两个参数固定为 -m 和 --always_zopflify。
    let program = executable(&fixture, "/bin/cp \"$3\" \"$4\"", 0o755);
    let report = fixture.report(Some(program.as_os_str()));
    assert!(report["warnings"].as_array().unwrap().is_empty());
    assert!(report["notes"].as_array().unwrap().is_empty());
    let trial = zopfli_trial(&report["trials"]);
    assert_eq!(trial["skipped"], false);
    assert_eq!(trial["valid"], true);
    assert_eq!(trial["bytes"], fixture.original.len());
}
