use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(args)
        .output()
        .unwrap()
}
fn real(args: &[&str], stdin: Option<&str>) -> Option<Output> {
    let backend = std::env::var_os("KITO_OPENSSL").unwrap_or_else(|| "openssl".into());
    let version = Command::new(&backend).arg("version").output();
    if !version.is_ok_and(|v| {
        v.status.success() && String::from_utf8_lossy(&v.stdout).starts_with("OpenSSL 3.")
    }) {
        assert!(
            std::env::var_os("KITO_REQUIRE_BACKEND").as_deref() != Some(std::ffi::OsStr::new("1")),
            "KITO_REQUIRE_BACKEND=1 要求可用的 OpenSSL 3"
        );
        eprintln!("SKIP: key 真实后端测试需要 OpenSSL 3，设置 KITO_OPENSSL");
        return None;
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_kito"))
        .env("KITO_OPENSSL", backend)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    Some(child.wait_with_output().unwrap())
}
fn p(path: &Path) -> &str {
    path.to_str().unwrap()
}
#[test]
fn missing_backend_is_runtime_error() {
    let out = Command::new(env!("CARGO_BIN_EXE_kito"))
        .env("KITO_OPENSSL", "/missing/kito/openssl")
        .args(["key", "doctor", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("OpenSSL"));
}
#[test]
fn generate_requires_explicit_passphrase_policy() {
    let out = run(&["key", "generate", "--out-dir", "unused"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("passphrase"));
}
#[test]
fn ed25519_rejects_bits() {
    let temp = tempfile::tempdir().unwrap();
    let out = run(&[
        "key",
        "generate",
        "--algorithm",
        "ed25519",
        "--bits",
        "2048",
        "--out-dir",
        p(&temp.path().join("keys")),
        "--no-passphrase",
    ]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("bits"));
    assert!(!temp.path().join("keys").exists());
}
#[test]
fn real_rsa_and_ed25519_roundtrip_and_protection() {
    let temp = tempfile::tempdir().unwrap();
    for algorithm in ["rsa", "ed25519"] {
        let dir = temp.path().join(algorithm);
        let mut args = vec![
            "key",
            "generate",
            "--algorithm",
            algorithm,
            "--out-dir",
            p(&dir),
            "--passphrase-stdin",
            "--json",
        ];
        if algorithm == "rsa" {
            args.extend(["--bits", "2048"]);
        }
        let Some(out) = real(&args, Some("temporary-test-secret\n")) else {
            return;
        };
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["algorithm"], algorithm);
        assert_eq!(report["encryption"]["iterations"], 600000);
        let private = dir.join("private.pem");
        let public = dir.join("public.pem");
        let private_bytes = std::fs::read(&private).unwrap();
        assert!(private_bytes.starts_with(b"-----BEGIN ENCRYPTED PRIVATE KEY-----"));
        let check = real(
            &[
                "key",
                "check",
                p(&private),
                "--public",
                p(&public),
                "--passphrase-stdin",
                "--json",
            ],
            Some("temporary-test-secret\n"),
        )
        .unwrap();
        assert!(
            check.status.success(),
            "{}",
            String::from_utf8_lossy(&check.stderr)
        );
        let checked: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
        assert_eq!(checked["validity"], true);
        assert_eq!(checked["match"], true);
        let wrong = real(
            &["key", "check", p(&private), "--passphrase-stdin"],
            Some("wrong-secret\n"),
        )
        .unwrap();
        assert_eq!(wrong.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&wrong.stderr).contains("wrong-secret"));
        let exported = temp.path().join(format!("{algorithm}-public.pem"));
        let export = real(
            &[
                "key",
                "public",
                p(&private),
                "--passphrase-stdin",
                "-o",
                p(&exported),
            ],
            Some("temporary-test-secret\n"),
        )
        .unwrap();
        assert!(export.status.success());
        assert_eq!(
            std::fs::read(exported).unwrap(),
            std::fs::read(&public).unwrap()
        );
        let conflict = real(&args, Some("temporary-test-secret\n")).unwrap();
        assert_eq!(conflict.status.code(), Some(1));
        assert_eq!(std::fs::read(private).unwrap(), private_bytes);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(dir.join("private.pem"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
#[test]
fn real_wrong_pair_and_existing_export_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("one");
    let second = temp.path().join("two");
    for dir in [&first, &second] {
        let Some(out) = real(
            &[
                "key",
                "generate",
                "--algorithm",
                "ed25519",
                "--out-dir",
                p(dir),
                "--no-passphrase",
            ],
            None,
        ) else {
            return;
        };
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let out = real(
        &[
            "key",
            "check",
            p(&first.join("private.pem")),
            "--public",
            p(&second.join("public.pem")),
            "--json",
        ],
        None,
    )
    .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let checked: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(checked["match"], false);
    let out = real(
        &[
            "key",
            "public",
            p(&first.join("private.pem")),
            "-o",
            p(&second.join("public.pem")),
        ],
        None,
    )
    .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn real_nested_output_and_passphrase_file() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("keys/application");
    let pass = temp.path().join("passphrase.txt");
    std::fs::write(&pass, "temporary-test-secret\r\n").unwrap();
    let Some(output) = real(
        &[
            "key",
            "generate",
            "--algorithm",
            "ed25519",
            "--out-dir",
            p(&dir),
            "--passphrase-file",
            p(&pass),
            "--json",
        ],
        None,
    ) else {
        return;
    };
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let check = real(&["key", "check", p(&dir.join("private.pem"))], None).unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&check.stderr).contains("已加密"));
    let backend = std::env::var_os("KITO_OPENSSL").unwrap_or_else(|| "openssl".into());
    let parameters = Command::new(backend)
        .args(["asn1parse", "-in", p(&dir.join("private.pem"))])
        .output()
        .unwrap();
    assert!(parameters.status.success());
    let parameters = String::from_utf8(parameters.stdout).unwrap();
    for parameter in ["PBES2", "PBKDF2", "hmacWithSHA256", "aes-256-cbc", "0927C0"] {
        assert!(parameters.contains(parameter), "missing {parameter}");
    }
}
#[test]
fn invalid_passphrase_never_creates_output() {
    let temp = tempfile::tempdir().unwrap();
    for (index, passphrase) in ["", "\n", "a\nb", "a\rb", "a\0b"].iter().enumerate() {
        let dir = temp.path().join(index.to_string());
        let pass = temp.path().join("passphrase.txt");
        std::fs::write(&pass, passphrase).unwrap();
        let output = run(&[
            "key",
            "generate",
            "--out-dir",
            p(&dir),
            "--passphrase-file",
            p(&pass),
        ]);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("口令必须"));
        assert!(!dir.exists());
    }
}
#[test]
fn protection_choices_are_mutually_exclusive() {
    let output = run(&[
        "key",
        "generate",
        "--out-dir",
        "unused",
        "--no-passphrase",
        "--passphrase-stdin",
    ]);
    assert_eq!(output.status.code(), Some(2));
}
#[cfg(unix)]
#[test]
fn symlink_destination_does_not_change_original() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let existing = temp.path().join("existing");
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("keep"), "sentinel").unwrap();
    let link = temp.path().join("link");
    symlink(&existing, &link).unwrap();
    let output = run(&["key", "generate", "--out-dir", p(&link), "--no-passphrase"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(std::fs::read(existing.join("keep")).unwrap(), b"sentinel");
}

#[test]
fn regression_unsupported_ec_never_publishes_public_output() {
    let Some(doctor) = real(&["key", "doctor"], None) else {
        return;
    };
    assert!(doctor.status.success());
    let temp = tempfile::tempdir().unwrap();
    let private = temp.path().join("ec-private.pem");
    let public = temp.path().join("export.pem");
    let backend = std::env::var_os("KITO_OPENSSL").unwrap_or_else(|| "openssl".into());
    let generated = Command::new(backend)
        .args([
            "genpkey",
            "-algorithm",
            "EC",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-out",
            p(&private),
        ])
        .output()
        .unwrap();
    assert!(generated.status.success());
    let original = std::fs::read(&private).unwrap();
    let checked = real(&["key", "check", p(&private), "--json"], None).unwrap();
    assert_eq!(checked.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&checked.stderr).contains("仅支持 RSA 和 Ed25519"));
    let exported = real(
        &["key", "public", p(&private), "-o", p(&public), "--json"],
        None,
    )
    .unwrap();
    assert_eq!(exported.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&exported.stderr).contains("仅支持 RSA 和 Ed25519"));
    assert!(!public.exists(), "不支持的算法不能留下已发布公钥");
    assert_eq!(std::fs::read(private).unwrap(), original);
}

#[test]
fn regression_passphrase_1024_bytes_is_rejected_before_generation() {
    let temp = tempfile::tempdir().unwrap();
    for source in ["stdin", "file"] {
        let dir = temp.path().join(source);
        let pass = format!("{}X\r\n", "a".repeat(1023));
        let pass_path = temp.path().join("passphrase.txt");
        std::fs::write(&pass_path, &pass).unwrap();
        let mut arguments = vec![
            "key",
            "generate",
            "--algorithm",
            "ed25519",
            "--out-dir",
            p(&dir),
        ];
        if source == "stdin" {
            arguments.push("--passphrase-stdin");
        } else {
            arguments.extend(["--passphrase-file", p(&pass_path)]);
        }
        let Some(output) = real(&arguments, (source == "stdin").then_some(pass.as_str())) else {
            return;
        };
        assert_eq!(
            output.status.code(),
            Some(1),
            "1024字节口令必须拒绝，不能让后端截断"
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("1–1023"));
        assert!(!dir.exists());
    }
}

#[test]
fn regression_passphrase_1023_bytes_preserves_final_byte() {
    let temp = tempfile::tempdir().unwrap();
    for (index, (correct, incorrect)) in [
        (
            format!("{}X", "a".repeat(1022)),
            format!("{}Y", "a".repeat(1022)),
        ),
        ("界".repeat(341), format!("{}田", "界".repeat(340))),
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(correct.len(), 1023);
        assert_eq!(incorrect.len(), 1023);
        let dir = temp.path().join(index.to_string());
        let Some(generated) = real(
            &[
                "key",
                "generate",
                "--algorithm",
                "ed25519",
                "--out-dir",
                p(&dir),
                "--passphrase-stdin",
            ],
            Some(&format!("{correct}\r\n")),
        ) else {
            return;
        };
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let private = dir.join("private.pem");
        let checked = real(
            &["key", "check", p(&private), "--passphrase-stdin"],
            Some(correct),
        )
        .unwrap();
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
        let wrong = real(
            &["key", "check", p(&private), "--passphrase-stdin"],
            Some(incorrect),
        )
        .unwrap();
        assert_eq!(
            wrong.status.code(),
            Some(1),
            "口令最后字节改变后必须无法解密"
        );
    }
}

#[cfg(unix)]
fn non_utf8_name() -> std::ffi::OsString {
    use std::os::unix::ffi::OsStringExt;
    std::ffi::OsString::from_vec(b"key-\xff".to_vec())
}

#[cfg(unix)]
#[test]
fn regression_non_utf8_generate_path_is_rejected_before_writes() {
    let Some(doctor) = real(&["key", "doctor"], None) else {
        return;
    };
    assert!(doctor.status.success());
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("must-not-be-created");
    let dir = parent.join(non_utf8_name());
    let output = Command::new(env!("CARGO_BIN_EXE_kito"))
        .args([
            "key",
            "generate",
            "--algorithm",
            "ed25519",
            "--no-passphrase",
            "--json",
            "--out-dir",
        ])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("UTF-8"));
    assert!(!dir.exists());
    assert!(!parent.exists(), "报告路径无效时连父目录也不能创建");
}

#[cfg(unix)]
#[test]
fn regression_non_utf8_public_output_is_rejected_before_publish() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("keys");
    let Some(generated) = real(
        &[
            "key",
            "generate",
            "--algorithm",
            "ed25519",
            "--out-dir",
            p(&dir),
            "--no-passphrase",
        ],
        None,
    ) else {
        return;
    };
    assert!(generated.status.success());
    let private = dir.join("private.pem");
    let public = temp.path().join(non_utf8_name());
    let output = Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(["key", "public"])
        .arg(&private)
        .arg("-o")
        .arg(&public)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("UTF-8"));
    assert!(!public.exists());
}

#[cfg(unix)]
#[test]
fn regression_non_utf8_check_path_reports_error_without_panic() {
    let temp = tempfile::tempdir().unwrap();
    // 即使文件不存在，也必须在文件读取或后端操作之前拒绝不可序列化的报告路径。
    let private = temp.path().join(non_utf8_name());
    let output = Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(["key", "check"])
        .arg(&private)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("UTF-8"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    assert!(!private.exists());
}
