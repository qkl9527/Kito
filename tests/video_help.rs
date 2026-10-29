//! 视频 CLI 的公开入口与参数约束无需安装 FFmpeg 即可验证。
use std::process::{Command, Output, Stdio};

fn kito(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(args)
        .stdin(Stdio::null())
        // 帮助与参数错误应在后端发现之前处理，不依赖开发机安装情况。
        .env("KITO_FFMPEG", "kito-test-missing-ffmpeg")
        .env("KITO_FFPROBE", "kito-test-missing-ffprobe")
        .output()
        .unwrap()
}

#[test]
fn video_partner_is_discoverable_from_top_level_help() {
    let output = kito(&["--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("video"), "{help}");
    assert!(help.contains("Vido 维多"), "{help}");
}

#[test]
fn every_video_operation_has_help_without_a_backend() {
    for command in [
        "doctor", "info", "convert", "compress", "trim", "resize", "crop", "rotate", "speed",
        "concat", "mute", "snapshot", "gif",
    ] {
        let output = kito(&["video", command, "--help"]);
        assert!(
            output.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
    }
    for command in [
        "extract",
        "replace",
        "volume",
        "convert",
        "trim",
        "concat",
        "mix",
        "speed",
        "fade",
        "normalize",
    ] {
        let output = kito(&["video", "audio", command, "--help"]);
        assert!(
            output.status.success(),
            "audio {command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn trim_requires_one_unambiguous_end_condition() {
    for args in [
        vec!["video", "trim", "input.mp4"],
        vec![
            "video",
            "trim",
            "input.mp4",
            "--end",
            "5",
            "--duration",
            "3",
        ],
    ] {
        let output = kito(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("--duration"), "{error}");
    }
}

#[test]
fn invalid_video_choices_are_rejected_by_the_cli() {
    for args in [
        vec!["video", "convert", "input.mp4", "--to", "avi"],
        vec!["video", "rotate", "input.mp4", "--angle", "45"],
        vec!["video", "concat", "input.mp4"],
        vec!["video", "compress", "input.mp4", "--time-limit", "0"],
    ] {
        let output = kito(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            !error.contains("unrecognized subcommand 'video'"),
            "{error}"
        );
        assert!(!error.contains("kito-test-missing"), "{error}");
    }
}
