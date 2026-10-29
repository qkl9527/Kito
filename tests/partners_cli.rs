//! 新伙伴的共享输入输出约定：验证用户看见的结果和文件保护。
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

fn kito(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn new_partners_appear_in_top_level_help() {
    let output = kito(&["--help"]);
    let help = String::from_utf8(output.stdout).unwrap();
    for name in ["Codi", "Sumi", "Kavi"] {
        assert!(help.contains(name), "帮助缺少伙伴 {name}: {help}");
    }
}

#[test]
fn binary_stdin_round_trip_preserves_every_byte_without_added_newline() {
    let bytes: Vec<u8> = (0..=255).collect();
    let mut child = Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(["codec", "base64", "encode", "--file", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&bytes).unwrap();
    let encoded = child.wait_with_output().unwrap();
    assert!(
        encoded.status.success(),
        "{}",
        String::from_utf8_lossy(&encoded.stderr)
    );
    let decoded = kito(&[
        "codec",
        "base64",
        "decode",
        std::str::from_utf8(&encoded.stdout).unwrap(),
    ]);
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    assert_eq!(decoded.stdout, bytes);
}

#[test]
fn transformation_refuses_same_file_and_preserves_existing_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.bin");
    let output = dir.path().join("output.txt");
    fs::write(&input, b"hello\0world").unwrap();
    fs::write(&output, b"KEEP").unwrap();
    for destination in [&input, &output] {
        let result = kito(&[
            "codec",
            "base64",
            "encode",
            "--file",
            input.to_str().unwrap(),
            "-o",
            destination.to_str().unwrap(),
        ]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("输出"));
    }
    assert_eq!(fs::read(&input).unwrap(), b"hello\0world");
    assert_eq!(fs::read(&output).unwrap(), b"KEEP");
}

#[test]
fn explicit_overwrite_still_refuses_to_replace_input_and_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.bin");
    fs::write(&input, b"ORIGINAL").unwrap();
    let output = kito(&[
        "codec",
        "base64",
        "encode",
        "--file",
        input.to_str().unwrap(),
        "-o",
        input.to_str().unwrap(),
        "--overwrite",
    ]);
    assert!(!output.status.success());
    assert_eq!(fs::read(&input).unwrap(), b"ORIGINAL");
    #[cfg(unix)]
    {
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&input, &link).unwrap();
        let output = kito(&[
            "codec",
            "base64",
            "encode",
            "hello",
            "-o",
            link.to_str().unwrap(),
            "--overwrite",
        ]);
        assert!(!output.status.success());
        assert_eq!(fs::read(&input).unwrap(), b"ORIGINAL");
        assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    }
}

#[test]
fn shared_reader_enforces_byte_limit_and_does_not_replace_invalid_utf8() {
    use kito::io_args::InputArgs;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bytes");
    fs::write(&path, [0xef, 0xbb, 0xbf, b'\n', 0xff]).unwrap();
    let input = InputArgs {
        text: None,
        file: Some(path),
    };
    assert_eq!(
        input.read_bytes(5).unwrap(),
        [0xef, 0xbb, 0xbf, b'\n', 0xff]
    );
    assert!(input.read_bytes(4).is_err());
    assert!(input.read_utf8(5).is_err());
}

#[test]
fn simultaneous_text_and_file_are_rejected() {
    let output = kito(&["id", "hash", "hello", "--file", "-"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
