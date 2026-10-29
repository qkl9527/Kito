use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

struct Fixture(tempfile::TempDir);

impl Fixture {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    fn path(&self) -> PathBuf {
        self.0.path().join("time.json")
    }
    fn run(&self, args: &[&str], timezone: Option<&str>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kito"));
        // 每个子进程共享该测试的独立目录，验证落盘持久化，同时避免读写开发机配置。
        command
            .arg("time")
            .args(args)
            .env("KITO_CONFIG_DIR", self.0.path())
            .env_remove("KITO_TIMEZONE");
        if let Some(value) = timezone {
            command.env("KITO_TIMEZONE", value);
        }
        command.output().unwrap()
    }
    fn text(&self, args: &[&str]) -> String {
        let result = self.run(args, None);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        String::from_utf8(result.stdout).unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        serde_json::from_str(&self.text(args)).unwrap()
    }
    fn set(&self, zone: &str) {
        assert_eq!(
            self.text(&["timezone", "set", zone, "--value-only"]),
            format!("{zone}\n")
        );
    }
}

#[test]
fn saved_timezone_persists_across_processes_and_applies_to_every_conversion() {
    let fixture = Fixture::new();
    fixture.set("Asia/Shanghai");
    let stored: Value = serde_json::from_slice(&fs::read(fixture.path()).unwrap()).unwrap();
    assert_eq!(stored["timezone"], "Asia/Shanghai");
    let current = fixture.json(&["timezone", "get", "--json"]);
    assert_eq!(current["timezone"], "Asia/Shanghai");
    assert_eq!(current["source"], "config");
    assert_eq!(
        fixture.text(&["from-timestamp", "0", "--value-only"]),
        "1970-01-01T08:00:00+08:00\n"
    );
    assert_eq!(
        fixture.text(&["to-timestamp", "1970-01-01 08:00:00", "--value-only"]),
        "0\n"
    );
    assert_eq!(
        fixture.json(&["now", "--json"])["timezone"],
        "Asia/Shanghai"
    );
    assert_eq!(
        fixture.text(&["convert", "1970-01-01T00:00:00Z", "--value-only"]),
        "1970-01-01T08:00:00+08:00\n"
    );
    assert_eq!(
        fixture.text(&[
            "convert",
            "1970-01-01 08:00:00",
            "--to-timezone",
            "UTC",
            "--value-only"
        ]),
        "1970-01-01T00:00:00Z\n"
    );
}

#[test]
fn explicit_parameters_then_environment_override_saved_default() {
    let fixture = Fixture::new();
    fixture.set("Asia/Shanghai");
    for (args, env, expected) in [
        (
            vec!["from-timestamp", "0", "--value-only"],
            "America/New_York",
            "1969-12-31T19:00:00-05:00\n",
        ),
        (
            vec!["from-timestamp", "0", "--timezone", "UTC", "--value-only"],
            "invalid",
            "1970-01-01T00:00:00Z\n",
        ),
        (
            vec!["to-timestamp", "1970-01-01T00:00:00Z", "--value-only"],
            "invalid",
            "0\n",
        ),
        (
            vec![
                "convert",
                "1970-01-01T00:00:00Z",
                "--to-timezone",
                "+05:45",
                "--value-only",
            ],
            "invalid",
            "1970-01-01T05:45:00+05:45\n",
        ),
        (vec!["timezone", "get", "--value-only"], "UTC", "UTC\n"),
    ] {
        let output = fixture.run(&args, Some(env));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
    let output = fixture.run(&["timezone", "get", "--json"], Some("UTC"));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["source"], "environment");
    assert_eq!(
        fixture.text(&["timezone", "get", "--value-only"]),
        "Asia/Shanghai\n"
    );
}

#[test]
fn fresh_configuration_defaults_to_utc_without_creating_files() {
    let fixture = Fixture::new();
    let report = fixture.json(&["timezone", "get", "--json"]);
    assert_eq!(report["timezone"], "UTC");
    assert_eq!(report["source"], "fallback");
    assert_eq!(
        fixture.text(&["convert", "1970-01-01T08:00:00+08:00", "--value-only"]),
        "1970-01-01T00:00:00Z\n"
    );
    assert_eq!(
        fixture.text(&["timezone", "reset", "--value-only"]),
        "reset\n"
    );
    assert!(!fixture.path().exists());
    assert_eq!(fs::read_dir(fixture.0.path()).unwrap().count(), 0);
}

#[test]
fn set_and_reset_preserve_unrelated_configuration_fields() {
    let fixture = Fixture::new();
    fs::write(
        fixture.path(),
        br#"{"timezone":"UTC","other":{"keep":true}}"#,
    )
    .unwrap();
    fixture.set("-03:30");
    let report = fixture.json(&["timezone", "set", "+05:45", "--json"]);
    assert_eq!(report["timezone"], "+05:45");
    assert_eq!(report["source"], "config");
    let stored: Value = serde_json::from_slice(&fs::read(fixture.path()).unwrap()).unwrap();
    assert_eq!(stored["other"], json!({"keep": true}));
    let reset = fixture.json(&["timezone", "reset", "--json"]);
    assert_eq!(reset["action"], "reset");
    assert!(reset["timezone"].is_null());
    let stored: Value = serde_json::from_slice(&fs::read(fixture.path()).unwrap()).unwrap();
    assert_eq!(stored, json!({"other": {"keep": true}}));
    assert_eq!(fixture.text(&["timezone", "get", "--value-only"]), "UTC\n");
}

#[test]
fn invalid_zone_or_unwritable_target_never_damages_existing_configuration() {
    let fixture = Fixture::new();
    fixture.set("Asia/Shanghai");
    let before = fs::read(fixture.path()).unwrap();
    let output = fixture.run(&["timezone", "set", "not-a-zone"], None);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(fixture.path()).unwrap(), before);
    fs::remove_file(fixture.path()).unwrap();
    fs::create_dir(fixture.path()).unwrap();
    fs::write(fixture.path().join("keep"), "KEEP").unwrap();
    let output = fixture.run(&["timezone", "set", "UTC"], None);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(fixture.path().join("keep")).unwrap(), b"KEEP");
}

#[test]
fn corrupt_configuration_is_rejected_but_explicit_inputs_can_bypass_it() {
    let fixture = Fixture::new();
    for bad in [
        "not json",
        "[]",
        "{\"timezone\":42}",
        "{\"timezone\":\"invalid\"}",
    ] {
        fs::write(fixture.path(), bad).unwrap();
        let output = fixture.run(&["from-timestamp", "0"], None);
        assert!(!output.status.success(), "accepted {bad}");
        assert!(output.stdout.is_empty());
        assert_eq!(
            fixture.text(&["from-timestamp", "0", "--timezone", "UTC", "--value-only"]),
            "1970-01-01T00:00:00Z\n"
        );
        assert_eq!(
            fixture.text(&["to-timestamp", "1970-01-01T00:00:00Z", "--value-only"]),
            "0\n"
        );
        assert_eq!(
            fixture.text(&["zones", "Shanghai", "--value-only"]),
            "Asia/Shanghai\n"
        );
        let output = fixture.run(&["from-timestamp", "0", "--value-only"], Some("UTC"));
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "1970-01-01T00:00:00Z\n"
        );
        assert_eq!(fs::read_to_string(fixture.path()).unwrap(), bad);
    }
    fs::write(fixture.path(), "not json").unwrap();
    for action in [vec!["timezone", "set", "UTC"], vec!["timezone", "reset"]] {
        assert!(!fixture.run(&action, None).status.success());
        assert_eq!(fs::read_to_string(fixture.path()).unwrap(), "not json");
    }
}

#[test]
fn changes_succeed_even_when_the_environment_override_is_invalid() {
    let fixture = Fixture::new();
    let output = fixture.run(
        &["timezone", "set", "Asia/Shanghai", "--value-only"],
        Some("invalid"),
    );
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "Asia/Shanghai\n");
    assert_eq!(
        fixture.text(&["timezone", "get", "--value-only"]),
        "Asia/Shanghai\n"
    );
    let output = fixture.run(&["timezone", "reset", "--value-only"], Some("invalid"));
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "reset\n");
    assert_eq!(fixture.text(&["timezone", "get", "--value-only"]), "UTC\n");
}

#[test]
fn relative_configuration_directory_is_rejected_before_writing() {
    let output = Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(["time", "timezone", "set", "UTC"])
        .env("KITO_CONFIG_DIR", "relative-kito-config")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("绝对路径"));
}

#[test]
fn timezone_updates_preserve_unknown_large_numbers_and_decimal_precision() {
    let fixture = Fixture::new();
    fs::write(fixture.path(), r#"{"timezone":"UTC","other":{"id":18446744073709551617,"decimal":0.123456789012345678901}}"#).unwrap();
    fixture.set("Asia/Shanghai");
    let saved = fs::read_to_string(fixture.path()).unwrap();
    assert!(
        saved.contains("18446744073709551617"),
        "integer changed: {saved}"
    );
    assert!(
        saved.contains("0.123456789012345678901"),
        "decimal changed: {saved}"
    );
    fixture.text(&["timezone", "reset", "--value-only"]);
    let saved = fs::read_to_string(fixture.path()).unwrap();
    assert!(saved.contains("18446744073709551617"));
    assert!(saved.contains("0.123456789012345678901"));
}

#[test]
fn absent_user_directory_still_allows_utc_fallback_but_cannot_save() {
    // 只移除子进程的目录变量，模拟精简的非交互环境；不修改测试进程或真实目录。
    for (args, succeeds, expected) in [
        (
            vec!["time", "from-timestamp", "0", "--value-only"],
            true,
            "1970-01-01T00:00:00Z\n",
        ),
        (
            vec!["time", "timezone", "get", "--value-only"],
            true,
            "UTC\n",
        ),
        (vec!["time", "timezone", "set", "UTC"], false, ""),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kito"));
        command.args(args);
        for name in [
            "HOME",
            "APPDATA",
            "XDG_CONFIG_HOME",
            "KITO_CONFIG_DIR",
            "KITO_TIMEZONE",
        ] {
            command.env_remove(name);
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.success(),
            succeeds,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        if !succeeds {
            assert!(String::from_utf8_lossy(&output.stderr).contains("KITO_CONFIG_DIR"));
        }
    }
}
