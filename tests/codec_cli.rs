use std::io::Write;
use std::process::{Command, Output, Stdio};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kito"))
        .arg("codec")
        .args(args)
        .output()
        .unwrap()
}

fn success(args: &[&str]) -> Vec<u8> {
    let result = cli(args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    result.stdout
}

fn reject(args: &[&str]) {
    let result = cli(args);
    assert!(!result.status.success(), "应拒绝 {args:?}");
    assert!(result.stdout.is_empty(), "错误前不可输出部分结果");
}

#[test]
fn base64_standard_vectors_and_padding_are_exact() {
    for (plain, encoded) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("你好", "5L2g5aW9"),
    ] {
        assert_eq!(success(&["base64", "encode", plain]), encoded.as_bytes());
        assert_eq!(success(&["base64", "decode", encoded]), plain.as_bytes());
    }
    assert_eq!(
        success(&["base64", "encode", "f", "--padding", "unpadded"]),
        b"Zg"
    );
    assert_eq!(success(&["base64", "decode", "Zg"]), b"f");
    assert_eq!(
        success(&["base64", "decode", " Zg==\r\n\t", "--ignore-whitespace"]),
        b"f"
    );
    for invalid in ["Zh==", "Zg=", "Zg===", "Z=g=", " Zg==", "Zg==\u{a0}"] {
        reject(&["base64", "decode", invalid]);
    }
    reject(&["base64", "encode", "f", "--padding", "auto"]);
    reject(&["base64", "decode", "Zg", "--padding", "padded"]);
    reject(&["base64", "decode", "Zg==", "--padding", "unpadded"]);
}

#[test]
fn base64_binary_stdin_and_file_round_trip_do_not_add_newlines() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.bin");
    let encoded = dir.path().join("encoded.txt");
    let restored = dir.path().join("restored.bin");
    let bytes = [0, 255, 254, 251, b'\n', b'\r'];
    std::fs::write(&input, bytes).unwrap();
    success(&[
        "base64",
        "encode",
        "--file",
        input.to_str().unwrap(),
        "--variant",
        "url-safe",
        "-o",
        encoded.to_str().unwrap(),
    ]);
    assert_eq!(std::fs::read(&encoded).unwrap(), b"AP_--woN");
    success(&[
        "base64",
        "decode",
        "--file",
        encoded.to_str().unwrap(),
        "--variant",
        "url-safe",
        "-o",
        restored.to_str().unwrap(),
    ]);
    assert_eq!(std::fs::read(restored).unwrap(), bytes);
    let mut child = Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(["codec", "base64", "encode", "--file", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&bytes).unwrap();
    assert_eq!(child.wait_with_output().unwrap().stdout, b"AP/++woN");
}

#[test]
fn output_conflicts_preserve_files_and_reject_same_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source");
    let output = dir.path().join("target");
    std::fs::write(&input, b"hello").unwrap();
    std::fs::write(&output, b"keep").unwrap();
    reject(&[
        "base64",
        "encode",
        "--file",
        input.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
    ]);
    reject(&[
        "base64",
        "encode",
        "--file",
        input.to_str().unwrap(),
        "-o",
        input.to_str().unwrap(),
        "--overwrite",
    ]);
    assert_eq!(std::fs::read(&input).unwrap(), b"hello");
    assert_eq!(std::fs::read(&output).unwrap(), b"keep");
    reject(&[
        "base64",
        "encode",
        "text",
        "--file",
        input.to_str().unwrap(),
    ]);
    reject(&["base64", "encode", "hello", "--max-input-mib", "0"]);
}

#[test]
fn url_preserves_repetition_empty_values_and_equals_presence() {
    let output = success(&[
        "url",
        "parse",
        "https://example.com/a?tag=a&tag=b&x=&flag&=v&&x=2#top",
        "--json",
    ]);
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        report["parameters"],
        serde_json::json!([
            {"name":"tag", "value":"a", "has_equals":true}, {"name":"tag", "value":"b", "has_equals":true},
            {"name":"x", "value":"", "has_equals":true}, {"name":"flag", "value":"", "has_equals":false},
            {"name":"", "value":"v", "has_equals":true}, {"name":"x", "value":"2", "has_equals":true}
        ])
    );
    assert_eq!(report["components"]["fragment"], "top");
    // 命令入口简化后，仍经 parse 验证两种加号语义和只解码一层的规则。
    for (query, mode, expected) in [
        ("?q=a+b", "percent", "a+b"),
        ("q=a+b", "form", "a b"),
        ("q=%2526%3Dx#z", "form", "%26=x#z"),
    ] {
        let bytes = success(&["url", "parse", query, "--query", "--mode", mode, "--json"]);
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            parsed["parameters"],
            serde_json::json!([{"name":"q", "value":expected, "has_equals":true}])
        );
    }
}

#[test]
fn url_only_offers_parse_and_rejects_removed_get() {
    let removed = cli(&["url", "get", "https://example.com/?tag=a", "--key", "tag"]);
    assert_eq!(removed.status.code(), Some(2));
    assert!(removed.stdout.is_empty());
    let help = String::from_utf8(success(&["url", "--help"])).unwrap();
    assert!(help.contains("  parse "));
    assert!(!help.contains("  get "));
}

#[test]
fn url_default_output_appends_copyable_query_json() {
    let text = String::from_utf8(success(&[
        "url",
        "parse",
        "https://example.com/search?q=hello+world&tag=a&tag=b&empty=&flag&n=123&truth=false&x%22y=line%0Ab",
    ])).unwrap();
    // 既有组件和参数展示继续可读，新段落则必须能直接作为合法 JSON 复制使用。
    assert!(text.contains("scheme: https\n"));
    assert!(text.contains("原始 query：q=hello+world"));
    assert!(text.contains("tag=a\ntag=b\n"));
    let (_, json) = text
        .split_once("\nQuery JSON:\n")
        .expect("缺少可复制的 Query JSON");
    assert!(json.starts_with("{\n"), "JSON 应分行缩进");
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "q":"hello world", "tag":["a","b"], "empty":"", "flag":"",
            "n":"123", "truth":"false", "x\"y":"line\nb"
        })
    );
}

#[test]
fn query_json_handles_no_query_and_percent_mode_without_type_inference() {
    for (input, extra, expected) in [
        ("https://example.com/path", vec![], serde_json::json!({})),
        (
            "?q=a+b&q=&=v&arr[]=1",
            vec!["--query", "--mode", "percent"],
            serde_json::json!({"q":["a+b",""], "":"v", "arr[]":"1"}),
        ),
    ] {
        let mut args = vec!["url", "parse", input];
        args.extend(extra);
        let text = String::from_utf8(success(&args)).unwrap();
        let (_, json) = text.split_once("\nQuery JSON:\n").expect("缺少 Query JSON");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json).unwrap(),
            expected
        );
    }
}

#[test]
fn url_rejects_ambiguous_sources_and_invalid_escapes_before_output() {
    for input in ["x=%", "x=%0", "x=%gg", "x=%ff", "x=%c0%af", "x=a\nb"] {
        reject(&["url", "parse", input, "--query"]);
    }
    for input in [
        "example.com?x=1",
        "https://example.com/a b",
        "https://example.com/\nx",
    ] {
        reject(&["url", "parse", input]);
    }
    reject(&["url", "parse", "x=1", "--query", "--json", "-o", "unused"]);
}

#[test]
fn html_has_full_named_entities_and_decodes_exactly_once() {
    assert_eq!(
        success(&[
            "html",
            "decode",
            "&NotEqualTilde; &fjlig; &amp;lt; &#x1F600;"
        ]),
        "≂\u{338} fj &lt; 😀".as_bytes()
    );
    assert_eq!(
        success(&["html", "encode", "<&\"'你好>"]),
        b"&lt;&amp;&quot;&apos;\xe4\xbd\xa0\xe5\xa5\xbd&gt;"
    );
    assert_eq!(
        success(&["html", "encode", "<&\"'", "--context", "text"]),
        b"&lt;&amp;\"'"
    );
    assert_eq!(
        success(&["html", "encode", "你好&", "--style", "hex", "--all"]),
        b"&#x4F60;&#x597D;&#x26;"
    );
    assert_eq!(success(&["html", "decode", "&#128;"]), "\u{80}".as_bytes());
    assert_eq!(
        success(&["html", "decode", "bare & text &1"]),
        b"bare & text &1"
    );
}

#[test]
fn html_and_xml_reject_invalid_entities_and_scalar_ranges() {
    for input in [
        "ok &unknown;",
        "&amp",
        "&#;",
        "&#x;",
        "&#xD800;",
        "&#1114112;",
        "&#0;",
        "&#xFDD0;",
        "&#xFFFF;",
    ] {
        reject(&["html", "decode", input]);
    }
    for input in ["&nbsp;", "&#1;", "&#xB;", "&#xFFFE;", "&#X41;"] {
        reject(&["html", "decode", input, "--standard", "xml"]);
    }
    assert_eq!(
        success(&["html", "encode", "\t\n\r", "--standard", "xml"]),
        b"&#9;&#10;&#13;"
    );
    assert_eq!(
        success(&[
            "html",
            "decode",
            "&lt;&amp;&quot;&apos;&gt;",
            "--standard",
            "xml"
        ]),
        b"<&\"'>"
    );
    reject(&["html", "encode", "\u{1}", "--standard", "xml"]);
}

#[test]
fn invalid_url_parse_does_not_publish_or_replace_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("values");
    let args = [
        "url",
        "parse",
        "x=%GG",
        "--query",
        "-o",
        path.to_str().unwrap(),
    ];
    assert_eq!(cli(&args).status.code(), Some(1));
    assert!(!path.exists());
    std::fs::write(&path, b"keep").unwrap();
    let mut overwrite = args.to_vec();
    overwrite.push("--overwrite");
    assert_eq!(cli(&overwrite).status.code(), Some(1));
    assert_eq!(std::fs::read(path).unwrap(), b"keep");
}

#[cfg(unix)]
#[test]
fn symbolic_link_output_cannot_replace_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    let link = dir.path().join("link");
    std::fs::write(&input, b"source").unwrap();
    std::os::unix::fs::symlink(&input, &link).unwrap();
    reject(&[
        "base64",
        "encode",
        "--file",
        input.to_str().unwrap(),
        "-o",
        link.to_str().unwrap(),
        "--overwrite",
    ]);
    assert_eq!(std::fs::read(&input).unwrap(), b"source");
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn utf8_decoders_reject_invalid_file_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("invalid-utf8");
    std::fs::write(&input, [0xff, b'&', b'a', b';']).unwrap();
    reject(&["html", "decode", "--file", input.to_str().unwrap()]);
    reject(&["url", "parse", "--query", "--file", input.to_str().unwrap()]);
    reject(&["base64", "decode", "AP_--woN"]);
    reject(&["base64", "decode", "AP/++woN", "--variant", "url-safe"]);
}
