use kito::video::probe;
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command, time::Duration};

#[test]
fn inputs_are_canonical_local_regular_files() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("media with spaces.mp4");
    fs::write(&file, b"not media yet").unwrap();
    assert_eq!(
        probe::input_path(&file).unwrap(),
        file.canonicalize().unwrap()
    );
    assert!(probe::input_path(directory.path()).is_err());
    assert!(probe::input_path(Path::new("-")).is_err());
    assert!(probe::input_path(Path::new("pipe:0")).is_err());
    assert!(probe::input_path(Path::new("https://example.invalid/video.mp4")).is_err());
    assert!(probe::input_path(&directory.path().join("absent.mp4")).is_err());
}

#[test]
fn playlist_cannot_open_an_unselected_local_segment() {
    let directory = tempfile::tempdir().unwrap();
    let segment = directory.path().join("private.ts");
    let generated = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=32x32:rate=25",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440",
            "-t",
            "2",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-f",
            "mpegts",
        ])
        .arg(&segment)
        .output();
    let Ok(generated) = generated else {
        assert_ne!(std::env::var("KITO_REQUIRE_BACKEND").as_deref(), Ok("1"));
        return;
    };
    if Command::new("ffprobe").arg("-version").output().is_err() {
        assert_ne!(std::env::var("KITO_REQUIRE_BACKEND").as_deref(), Ok("1"));
        return;
    }
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let contents = format!(
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n#EXTINF:2.0,\n{}\n#EXT-X-ENDLIST\n",
        segment.display()
    );
    // 原始扩展名在旧实现中会读取 private.ts；改名后仍必须按内容拒绝。
    for name in ["playlist.m3u8", "disguised.mp4"] {
        let playlist = directory.path().join(name);
        fs::write(&playlist, &contents).unwrap();
        let error = probe::probe(&playlist, Duration::from_secs(5)).unwrap_err();
        if name == "playlist.m3u8" {
            assert!(
                format!("{error:#}").contains("Format not on whitelist"),
                "必须由 demuxer 白名单拒绝播放列表：{error:#}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn local_symlinks_to_regular_files_are_supported() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("source.mp4");
    let link = directory.path().join("link.mp4");
    fs::write(&file, b"video").unwrap();
    std::os::unix::fs::symlink(&file, &link).unwrap();
    assert_eq!(
        probe::input_path(&link).unwrap(),
        file.canonicalize().unwrap()
    );
}

/// 后端变量仅设置在独立子进程，避免并行测试修改开发机环境。
fn child_report(directory: &Path, ffmpeg: &Path, ffprobe: &Path, action: &str) -> Value {
    let report = directory.join("report.json");
    let input = directory.join("input media.mp4");
    fs::write(&input, b"fixture").unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "isolated_backend_child"])
        .env("PATH", directory)
        .env("KITO_FFMPEG", ffmpeg)
        .env("KITO_FFPROBE", ffprobe)
        .env("KITO_VIDEO_TEST_REPORT", &report)
        .env("KITO_VIDEO_TEST_ACTION", action)
        .env("KITO_VIDEO_TEST_INPUT", &input)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&fs::read(report).unwrap()).unwrap()
}

#[test]
fn isolated_backend_child() {
    let Some(report) = std::env::var_os("KITO_VIDEO_TEST_REPORT") else {
        return;
    };
    let value = match std::env::var("KITO_VIDEO_TEST_ACTION").unwrap().as_str() {
        "probe" => {
            let input = std::env::var_os("KITO_VIDEO_TEST_INPUT").unwrap();
            match probe::probe(Path::new(&input), Duration::from_secs(5)) {
                Ok(media) => json!({"ok": media}),
                Err(error) => json!({"error": format!("{error:#}")}),
            }
        }
        "ffmpeg" => match probe::ffmpeg(&["-version".into()], Duration::from_secs(5)) {
            Ok(output) => json!({"ok": String::from_utf8(output.stdout).unwrap()}),
            Err(error) => json!({"error": format!("{error:#}")}),
        },
        _ => probe::doctor(),
    };
    fs::write(report, serde_json::to_vec(&value).unwrap()).unwrap();
}

#[test]
fn explicitly_invalid_backends_report_their_configuration_variables() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("not-installed");
    let report = child_report(directory.path(), &missing, &missing, "doctor");
    assert_eq!(report["healthy"], false);
    assert_eq!(report["kito"], env!("CARGO_PKG_VERSION"));
    for (backend, variable) in [("ffmpeg", "KITO_FFMPEG"), ("ffprobe", "KITO_FFPROBE")] {
        assert_eq!(report[backend]["path"], missing.to_string_lossy().as_ref());
        assert!(report[backend]["version"].is_null());
        assert!(
            report[backend]["error"]
                .as_str()
                .unwrap()
                .contains(variable)
        );
    }
    let probe = child_report(directory.path(), &missing, &missing, "probe");
    assert!(probe["error"].as_str().unwrap().contains("KITO_FFPROBE"));
    let ffmpeg = child_report(directory.path(), &missing, &missing, "ffmpeg");
    assert!(ffmpeg["error"].as_str().unwrap().contains("KITO_FFMPEG"));
}

#[cfg(unix)]
fn executable(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn probe_passes_protocol_whitelist_and_one_absolute_input_argument() {
    let directory = tempfile::tempdir().unwrap();
    let backend = directory.path().join("probe fixture");
    executable(
        &backend,
        r#"#!/bin/sh
[ "$#" -eq 12 ] || exit 41
[ "$1" = "-v" ] && [ "$2" = "error" ] || exit 42
[ "$3" = "-protocol_whitelist" ] && [ "$4" = "file" ] || exit 43
[ "$5" = "-format_whitelist" ] || exit 44
[ "$7" = "-show_format" ] && [ "$8" = "-show_streams" ] || exit 44
[ "$9" = "-of" ] && [ "${10}" = "json" ] && [ "${11}" = "-i" ] || exit 45
case "${12}" in /*"input media.mp4") ;; *) exit 46;; esac
printf '%s' '{"streams":[{"index":0,"codec_type":"video","width":320,"height":240}],"format":{"duration":"1.25","format_name":"mov,mp4"}}'
"#,
    );
    let report = child_report(directory.path(), &backend, &backend, "probe");
    assert_eq!(report["ok"]["duration"], 1.25, "{report}");
    assert_eq!(report["ok"]["size_bytes"], 7);
    assert_eq!(report["ok"]["streams"][0]["width"], 320);
}

#[cfg(unix)]
#[test]
fn successful_doctor_reports_configured_version_and_empty_path_does_not_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let backend = directory.path().join("version backend");
    executable(
        &backend,
        "#!/bin/sh\n[ \"$1\" = \"-version\" ] || exit 41\nprintf 'ffmpeg version test-1\\nmore lines\\n'\n",
    );
    let report = child_report(directory.path(), &backend, &backend, "doctor");
    assert_eq!(report["healthy"], true, "{report}");
    assert_eq!(report["ffmpeg"]["version"], "ffmpeg version test-1");
    assert_eq!(report["ffprobe"]["error"], Value::Null);
    let fallback = directory.path().join("ffmpeg");
    executable(&fallback, "#!/bin/sh\nprintf 'should not be used\\n'\n");
    let report = child_report(directory.path(), Path::new(""), &backend, "ffmpeg");
    assert!(report["error"].as_str().unwrap().contains("KITO_FFMPEG"));
}
