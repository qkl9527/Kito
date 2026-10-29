use std::io::Write;
use std::process::{Command, Output, Stdio};

fn cli(args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kito"));
    command.arg("id").args(args);
    if let Some(input) = stdin {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        child.wait_with_output().unwrap()
    } else {
        command.output().unwrap()
    }
}

fn success(args: &[&str]) -> String {
    let output = cli(args, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn hash_matches_standard_abc_vectors_for_every_algorithm() {
    for (algorithm, digest) in [
        ("md5", "900150983cd24fb0d6963f7d28e17f72"),
        ("sha1", "a9993e364706816aba3e25717850c26c9cd0d89d"),
        (
            "sha224",
            "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7",
        ),
        (
            "sha256",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "sha384",
            "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7",
        ),
        (
            "sha512",
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        ),
    ] {
        assert_eq!(
            success(&["hash", "abc", "--algorithm", algorithm]),
            format!("{digest}\n")
        );
    }
}

#[test]
fn hash_defaults_to_sha256_and_json_describes_only_the_digest() {
    let report: serde_json::Value =
        serde_json::from_str(&success(&["hash", "abc", "--json"])).unwrap();
    assert_eq!(
        report,
        serde_json::json!({
            "algorithm": "sha256", "format": "hex",
            "digest": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "input_bytes": "3", "source_kind": "text"
        })
    );
    assert_eq!(
        success(&["hash", "abc", "--format", "base64"]),
        "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=\n"
    );
}

#[test]
fn hash_reads_stdin_as_exact_binary_bytes() {
    let bytes = [0, 255, 13, 10, 65];
    for args in [
        vec!["hash", "--json"],
        vec!["hash", "--file", "-", "--json"],
    ] {
        let output = cli(&args, Some(&bytes));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            report["digest"],
            "4259d3f5151edfc7205e3e7c3fde341e237968666aad8946dcbcf6a374502c60"
        );
        assert_eq!(report["input_bytes"], "5");
        assert_eq!(report["source_kind"], "stdin");
    }
}

#[test]
fn hash_preserves_empty_inputs_and_unicode_utf8() {
    assert_eq!(
        success(&["hash", ""]),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n"
    );
    let output = cli(&["hash", "--json"], Some("你好".as_bytes()));
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["input_bytes"], "6");
    assert_eq!(report["source_kind"], "stdin");
}

#[test]
fn file_hash_reports_its_source_and_output_conflicts_preserve_existing_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.bin");
    let destination = directory.path().join("digest.txt");
    std::fs::write(&input, b"abc").unwrap();
    std::fs::write(&destination, b"existing").unwrap();
    let report: serde_json::Value = serde_json::from_str(&success(&[
        "hash",
        "--file",
        input.to_str().unwrap(),
        "--json",
    ]))
    .unwrap();
    assert_eq!(report["source_kind"], "file");
    assert_eq!(report["input_bytes"], "3");
    for target in [&input, &destination] {
        let output = cli(
            &[
                "hash",
                "--file",
                input.to_str().unwrap(),
                "-o",
                target.to_str().unwrap(),
            ],
            None,
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let same_file = cli(
        &[
            "hash",
            "--file",
            input.to_str().unwrap(),
            "-o",
            input.to_str().unwrap(),
            "--overwrite",
        ],
        None,
    );
    assert!(!same_file.status.success());
    assert_eq!(std::fs::read(&input).unwrap(), b"abc");
    assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
    let output = cli(
        &[
            "hash",
            "--file",
            input.to_str().unwrap(),
            "-o",
            destination.to_str().unwrap(),
            "--overwrite",
        ],
        None,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert_eq!(
        std::fs::read_to_string(&destination).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\n"
    );
}

#[test]
fn uuid_can_write_a_full_batch_to_a_new_file() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("uuid.txt");
    let output = cli(
        &[
            "uuid",
            "--count",
            "10000",
            "-o",
            destination.to_str().unwrap(),
        ],
        None,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert_eq!(
        std::fs::read_to_string(destination)
            .unwrap()
            .lines()
            .count(),
        10000
    );
}

#[test]
fn uuid_has_requested_version_variant_representation_and_count() {
    for version in ["v4", "v7"] {
        for format in ["hyphenated", "simple", "urn"] {
            let report: serde_json::Value = serde_json::from_str(&success(&[
                "uuid",
                "--version",
                version,
                "--format",
                format,
                "--count",
                "10",
                "--json",
            ]))
            .unwrap();
            assert_eq!(report["version"], version);
            assert_eq!(report["format"], format);
            let values = report["values"].as_array().unwrap();
            assert_eq!(values.len(), 10);
            for value in values {
                let value = value.as_str().unwrap();
                let simple = value
                    .strip_prefix("urn:uuid:")
                    .unwrap_or(value)
                    .replace('-', "");
                assert_eq!(simple.len(), 32);
                assert_eq!(&simple[12..13], &version[1..]);
                assert!(matches!(&simple[16..17], "8" | "9" | "a" | "b"));
                assert!(
                    simple
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                );
                assert_eq!(
                    value.len(),
                    match format {
                        "simple" => 32,
                        "urn" => 45,
                        _ => 36,
                    }
                );
            }
        }
    }
    let default = success(&["uuid"]);
    assert_eq!(default.lines().count(), 1);
    assert_eq!(default.as_bytes()[14], b'4');
}

#[test]
fn uuid_v7_timestamp_is_unix_milliseconds() {
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let value = success(&["uuid", "--version", "v7", "--format", "simple"]);
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let timestamp = u128::from_str_radix(&value[..12], 16).unwrap();
    assert!((before..=after).contains(&timestamp));
}

#[test]
fn uuid_and_hash_validate_options_before_stdout() {
    for args in [
        vec!["uuid", "--count", "0"],
        vec!["uuid", "--count", "10001"],
        vec!["uuid", "--version", "v1"],
        vec!["hash", "abc", "--algorithm", "unknown"],
        vec!["hash", "abc", "--json", "--output", "result.txt"],
        vec!["uuid", "--json", "--output", "result.txt"],
    ] {
        let output = cli(&args, None);
        assert!(!output.status.success(), "unexpected success: {args:?}");
        assert!(output.stdout.is_empty());
    }
}
