//! Matroska/ WebM 的容器结束时间不能替代首个有效轨道的持续时间。
use serde_json::Value;
use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::{Command, Output},
};

struct Backends {
    ffmpeg: OsString,
    ffprobe: OsString,
}
impl Backends {
    fn discover() -> Option<Self> {
        let mut paths = Vec::new();
        let mut missing = false;
        for (key, fallback) in [("KITO_FFMPEG", "ffmpeg"), ("KITO_FFPROBE", "ffprobe")] {
            let explicit = std::env::var_os(key);
            let path = explicit.clone().unwrap_or_else(|| fallback.into());
            let available = Command::new(&path)
                .arg("-version")
                .output()
                .is_ok_and(|out| out.status.success());
            if !available {
                assert!(explicit.is_none(), "显式 {key} 无效，不能跳过真实后端测试");
                assert!(
                    std::env::var_os("KITO_REQUIRE_BACKEND").as_deref() != Some(OsStr::new("1")),
                    "严格后端测试缺少 {fallback}"
                );
                eprintln!("SKIP: video duration 测试需要 {fallback}");
                missing = true;
            }
            paths.push(path);
        }
        if missing {
            return None;
        }
        Some(Self {
            ffmpeg: paths.remove(0),
            ffprobe: paths.remove(0),
        })
    }
    fn ffmpeg(&self, args: &[&str]) {
        let out = Command::new(&self.ffmpeg)
            .args(["-v", "error", "-nostdin"])
            .args(args)
            .output()
            .unwrap();
        success(&out);
    }
    fn probe(&self, path: &Path) -> Value {
        let out = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-show_format",
                "-show_streams",
                "-of",
                "json",
                p(path),
            ])
            .output()
            .unwrap();
        success(&out);
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn audio(&self, command: &str, input: &Path, output: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_kito"))
            .env("KITO_FFMPEG", &self.ffmpeg)
            .env("KITO_FFPROBE", &self.ffprobe)
            .args([
                "video",
                "audio",
                command,
                p(input),
                "--to",
                "wav",
                "-o",
                p(output),
                "--json",
            ])
            .args(args)
            .output()
            .unwrap()
    }
    fn samples(&self, path: &Path) -> Vec<f32> {
        let out = Command::new(&self.ffmpeg)
            .args([
                "-v",
                "error",
                "-nostdin",
                "-i",
                p(path),
                "-vn",
                "-ac",
                "1",
                "-ar",
                "48000",
                "-f",
                "f32le",
                "-",
            ])
            .output()
            .unwrap();
        success(&out);
        out.stdout
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v))
            .collect()
    }
    fn long_video(&self, path: &Path) {
        self.ffmpeg(&[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x120:rate=30:duration=4",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "pcm_s16le",
            p(path),
        ]);
        let raw = self.probe(path);
        assert_eq!(raw["format"]["duration"], "4.000000");
        assert!(
            raw["streams"][1]["duration"].is_null(),
            "fixture 音轨必须缺少逐轨duration"
        );
    }
}
fn p(path: &Path) -> &str {
    path.to_str().unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn rms(samples: &[f32]) -> f64 {
    (samples.iter().map(|&v| f64::from(v).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
}
fn assert_one_second_and_faded(samples: &[f32]) {
    assert!(
        (47_800..=48_200).contains(&samples.len()),
        "实际采样数 {}",
        samples.len()
    );
    assert!(
        rms(&samples[43_200..47_000]) / rms(&samples[4_800..9_600]) < 0.2,
        "淡出必须作用于首音轨自己的结尾"
    );
}

#[test]
fn regression_header_only_audio_cannot_be_published_as_verified() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("empty.wav");
    backends.ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "anullsrc=r=48000:cl=mono",
        "-t",
        "0",
        "-c:a",
        "pcm_s16le",
        p(&input),
    ]);
    let raw = backends.probe(&input);
    assert_eq!(raw["streams"][0]["codec_type"], "audio");
    assert!(
        backends.samples(&input).is_empty(),
        "fixture 必须只有header而无音频采样"
    );
    let output = temp.path().join("converted.wav");
    let result = backends.audio("convert", &input, &output, &[]);
    assert!(
        !result.status.success(),
        "空音轨不能被报告为verified_decodable: {}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(!output.exists());
    std::fs::write(&output, b"preserve destination").unwrap();
    let result = backends.audio("convert", &input, &output, &["--overwrite"]);
    assert!(!result.status.success());
    assert_eq!(std::fs::read(&output).unwrap(), b"preserve destination");
}

#[test]
fn regression_matroska_fade_uses_short_audio_instead_of_long_video_duration() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("long-video.mkv");
    backends.long_video(&input);
    let output = temp.path().join("faded.wav");
    let result = backends.audio("fade", &input, &output, &["--fade-out", "0.5"]);
    success(&result);
    assert_one_second_and_faded(&backends.samples(&output));
}

#[test]
fn regression_matroska_audio_trim_rejects_range_beyond_the_audio_track() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("long-video.mkv");
    backends.long_video(&input);
    let output = temp.path().join("empty.wav");
    let result = backends.audio(
        "trim",
        &input,
        &output,
        &["--start", "2", "--duration", "0.5"],
    );
    assert!(
        !result.status.success(),
        "音轨1秒后已结束，不能按4秒视频接受trim: {}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(!output.exists());
    std::fs::write(&output, b"preserve destination").unwrap();
    let result = backends.audio(
        "trim",
        &input,
        &output,
        &["--start", "2", "--duration", "0.5", "--overwrite"],
    );
    assert!(!result.status.success());
    assert_eq!(std::fs::read(&output).unwrap(), b"preserve destination");
}

#[test]
fn regression_unknown_duration_webm_audio_remains_editable() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("unknown-duration.webm");
    // live=1 不写容器或轨道时长；有效的 Opus 音频仍可按实际数据计算边界。
    backends.ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=1",
        "-c:a",
        "libopus",
        "-f",
        "webm",
        "-live",
        "1",
        p(&input),
    ]);
    let raw = backends.probe(&input);
    assert!(raw["format"]["duration"].is_null());
    assert!(raw["streams"][0]["duration"].is_null());
    for (command, options, expected) in [
        ("convert", vec![], 48_000),
        ("fade", vec!["--fade-out", "0.5"], 48_000),
        ("trim", vec!["--start", "0.2", "--duration", "0.5"], 24_000),
    ] {
        let output = temp.path().join(format!("{command}.wav"));
        let result = backends.audio(command, &input, &output, &options);
        success(&result);
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["verified_decodable"], true);
        let samples = backends.samples(&output);
        assert!(
            samples.len().abs_diff(expected) <= 200,
            "{command} 音频长度错误"
        );
        if command == "fade" {
            assert_one_second_and_faded(&samples);
        }
    }
}
