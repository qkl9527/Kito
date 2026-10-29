//! 用临时生成的视频验证真实 FFmpeg 管线，不读取开发机上的媒体或设置。
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

struct Backends {
    ffmpeg: OsString,
    ffprobe: OsString,
}

impl Backends {
    fn discover() -> Option<Self> {
        let mut found = Vec::new();
        let mut missing = false;
        for (variable, fallback) in [("KITO_FFMPEG", "ffmpeg"), ("KITO_FFPROBE", "ffprobe")] {
            let explicit = std::env::var_os(variable);
            let executable = explicit.clone().unwrap_or_else(|| fallback.into());
            let available = Command::new(&executable)
                .arg("-version")
                .stdin(Stdio::null())
                .output()
                .is_ok_and(|output| output.status.success());
            if !available {
                assert!(
                    explicit.is_none(),
                    "显式 {variable} 无效，不能跳过真实视频测试"
                );
                assert!(
                    std::env::var_os("KITO_REQUIRE_BACKEND").as_deref() != Some(OsStr::new("1")),
                    "KITO_REQUIRE_BACKEND=1 要求可用的 {fallback}"
                );
                eprintln!("SKIP: video 真实后端测试需要 {fallback}，可设置 {variable}");
                missing = true;
            }
            found.push(executable);
        }
        if missing {
            return None;
        }
        Some(Self {
            ffmpeg: found.remove(0),
            ffprobe: found.remove(0),
        })
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_kito"))
            .env("KITO_FFMPEG", &self.ffmpeg)
            .env("KITO_FFPROBE", &self.ffprobe)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn fixture(&self, path: &Path, size: &str, duration: &str, with_audio: bool) {
        let source = format!("testsrc2=size={size}:rate=30:duration={duration}");
        let audio = format!("sine=frequency=440:sample_rate=48000:duration={duration}");
        let mut command = Command::new(&self.ffmpeg);
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            &source,
        ]);
        if with_audio {
            command.args(["-f", "lavfi", "-i", &audio, "-c:a", "aac"]);
        }
        let output = command
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(path)
            .output()
            .unwrap();
        success(&output);
    }

    fn probe(&self, path: &Path) -> Value {
        let output = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-show_format",
                "-show_streams",
                "-of",
                "json",
            ])
            .arg(path)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        success(&output);
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn transform(&self, args: &[&str], output: &Path) -> Value {
        let mut arguments = args.to_vec();
        arguments.extend(["-o", p(output), "--json"]);
        let result = self.cli(&arguments);
        success(&result);
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["output"], p(output));
        assert!(report["operation"].is_string(), "{report}");
        assert!(report["output_info"].is_object(), "{report}");
        assert_eq!(report["lossy"], true);
        assert_eq!(report["verified_decodable"], true);
        // 独立解码最终发布文件；只看到 ffprobe 元数据不足以证明编码结果完整。
        let decoded = Command::new(&self.ffmpeg)
            .args(["-v", "error", "-nostdin", "-xerror", "-i"])
            .arg(output)
            .args(["-f", "null", "-"])
            .output()
            .unwrap();
        success(&decoded);
        self.probe(output)
    }

    fn audio_fixture(&self, output: &Path, duration: &str) {
        let source = format!("sine=frequency=880:sample_rate=48000:duration={duration}");
        let result = Command::new(&self.ffmpeg)
            .args(["-v", "error", "-nostdin", "-f", "lavfi", "-i", &source])
            .arg(output)
            .output()
            .unwrap();
        success(&result);
    }

    fn samples(&self, input: &Path) -> Vec<f32> {
        let result = Command::new(&self.ffmpeg)
            .args(["-v", "error", "-nostdin", "-i"])
            .arg(input)
            .args(["-vn", "-ac", "1", "-ar", "48000", "-f", "f32le", "-"])
            .output()
            .unwrap();
        success(&result);
        result
            .stdout
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect()
    }

    fn timeline_samples(&self, input: &Path) -> Vec<f32> {
        let result = Command::new(&self.ffmpeg)
            .args(["-v", "error", "-nostdin", "-i"])
            .arg(input)
            .args([
                "-vn",
                "-af",
                "aresample=async=1:first_pts=0",
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
        success(&result);
        result
            .stdout
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect()
    }

    fn pixels(&self, input: &Path, at: &str) -> Vec<u8> {
        let result = Command::new(&self.ffmpeg)
            .args(["-v", "error", "-nostdin", "-ss", at, "-i"])
            .arg(input)
            .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap();
        success(&result);
        result.stdout
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

fn stream<'a>(probe: &'a Value, kind: &str) -> &'a Value {
    probe["streams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|stream| stream["codec_type"] == kind)
        .unwrap_or_else(|| panic!("missing {kind} stream: {probe}"))
}

fn duration(probe: &Value) -> f64 {
    probe["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual}, expected={expected} ± {tolerance}"
    );
}

fn dimensions(probe: &Value, width: u64, height: u64) {
    let video = stream(probe, "video");
    assert_eq!(video["width"], width);
    assert_eq!(video["height"], height);
}

fn synchronized(probe: &Value, expected: f64) {
    close(duration(probe), expected, 0.12);
    for kind in ["video", "audio"] {
        let track = stream(probe, kind);
        close(
            track["start_time"].as_str().unwrap().parse().unwrap(),
            0.0,
            0.06,
        );
        close(
            track["duration"].as_str().unwrap().parse().unwrap(),
            expected,
            0.12,
        );
    }
}

fn rms(samples: &[f32]) -> f64 {
    (samples
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt()
}

#[test]
fn real_info_and_doctor_report_available_media_and_backends() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.mp4");
    backends.fixture(&input, "160x120", "1.2", true);
    let doctor = backends.cli(&["video", "doctor", "--json"]);
    success(&doctor);
    let report: Value = serde_json::from_slice(&doctor.stdout).unwrap();
    assert!(report.is_object());
    let info = backends.cli(&["video", "info", p(&input), "--json"]);
    success(&info);
    let report: Value = serde_json::from_slice(&info.stdout).unwrap();
    assert!(report.is_object());
    let rendered = report.to_string();
    assert!(rendered.contains("h264"), "{rendered}");
    assert!(rendered.contains("aac"), "{rendered}");
}

#[test]
fn real_convert_formats_and_compress_publish_decodable_video() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    // 引号、空格及 shell 元字符必须始终作为路径参数传递，不能被解释成命令。
    let input = temp.path().join("源片段 ' [a];$value.mp4");
    backends.fixture(&input, "160x120", "1.2", true);
    for (format, video_codec, audio_codec) in [
        ("mp4", "h264", "aac"),
        ("mkv", "h264", "aac"),
        ("mov", "h264", "aac"),
        ("webm", "vp9", "opus"),
    ] {
        let output = temp.path().join(format!("转换 [x] '{format}.{format}"));
        let probe = backends.transform(&["video", "convert", p(&input), "--to", format], &output);
        dimensions(&probe, 160, 120);
        close(duration(&probe), 1.2, 0.12);
        assert_eq!(stream(&probe, "video")["codec_name"], video_codec);
        assert_eq!(stream(&probe, "audio")["codec_name"], audio_codec);
    }
    let compressed = temp.path().join("compressed.mp4");
    let probe = backends.transform(&["video", "compress", p(&input)], &compressed);
    dimensions(&probe, 160, 120);
    synchronized(&probe, 1.2);
    let multi = temp.path().join("multiple-streams.mp4");
    let mapped = Command::new(&backends.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-i",
            p(&input),
            "-map",
            "0:v",
            "-map",
            "0:v",
            "-map",
            "0:a",
            "-map",
            "0:a",
            "-c",
            "copy",
            p(&multi),
        ])
        .output()
        .unwrap();
    success(&mapped);
    assert_eq!(
        backends.probe(&multi)["streams"].as_array().unwrap().len(),
        4
    );
    let output = temp.path().join("first-streams.mp4");
    let probe = backends.transform(&["video", "convert", p(&multi), "--to", "mp4"], &output);
    assert_eq!(probe["streams"].as_array().unwrap().len(), 2);
    stream(&probe, "video");
    stream(&probe, "audio");
}

#[test]
fn real_geometry_and_timing_filters_preserve_audio_sync() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.mp4");
    backends.fixture(&input, "160x120", "1.2", true);
    for (index, size) in ["80x60", "80x", "x60", "50%"].iter().enumerate() {
        let output = temp.path().join(format!("resize-{index}.mp4"));
        let probe = backends.transform(&["video", "resize", p(&input), "--size", size], &output);
        dimensions(&probe, 80, 60);
        synchronized(&probe, 1.2);
    }
    for (operation, options, expected) in [
        (
            "crop",
            vec!["--width", "64", "--height", "48", "--x", "0", "--y", "0"],
            (64, 48),
        ),
        ("rotate", vec!["--angle", "90"], (120, 160)),
        ("rotate", vec!["--angle", "180"], (160, 120)),
        ("rotate", vec!["--angle", "270"], (120, 160)),
    ] {
        let output = temp
            .path()
            .join(format!("{operation}-{}.mp4", options.join("_")));
        let mut args = vec!["video", operation, p(&input)];
        args.extend(options);
        let probe = backends.transform(&args, &output);
        dimensions(&probe, expected.0, expected.1);
        synchronized(&probe, 1.2);
        if operation == "rotate" {
            let source = backends.pixels(&input, "0");
            let rotated = backends.pixels(&output, "0");
            let angle = args[4];
            let mut total_difference = 0_u64;
            // 手工坐标映射验证转向，避免只测宽高却放过 180° 被忽略或 90°/270° 反向。
            for y in 0..120_usize {
                for x in 0..160_usize {
                    let (rx, ry) = match angle {
                        "90" => (119 - y, x),
                        "180" => (159 - x, 119 - y),
                        "270" => (y, 159 - x),
                        _ => unreachable!(),
                    };
                    for channel in 0..3 {
                        total_difference += u64::from(
                            source[(y * 160 + x) * 3 + channel]
                                .abs_diff(rotated[(ry * expected.0 as usize + rx) * 3 + channel]),
                        );
                    }
                }
            }
            assert!(
                total_difference as f64 / (source.len() as f64) < 18.0,
                "rotation direction/content mismatch: {angle}"
            );
        }
    }
    for (flag, value) in [("--duration", "0.5"), ("--end", "0.7")] {
        let output = temp.path().join(format!("trim{flag}.mp4"));
        let probe = backends.transform(
            &["video", "trim", p(&input), "--start", "0.2", flag, value],
            &output,
        );
        synchronized(&probe, 0.5);
    }
    // 4 倍速与 1/4 倍速超过单个 atempo 的传统范围，可发现音频 filter 串缺失。
    for (factor, expected) in [("2", 0.6), ("4", 0.3), ("0.25", 4.8)] {
        let output = temp.path().join(format!("speed{factor}.mp4"));
        let probe = backends.transform(&["video", "speed", p(&input), "--factor", factor], &output);
        synchronized(&probe, expected);
    }
}

#[test]
fn real_concat_normalizes_dimensions_and_pads_missing_audio() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.mp4");
    let second = temp.path().join("second.mp4");
    backends.fixture(&first, "160x120", "1.2", true);
    backends.fixture(&second, "120x80", "0.6", false);
    for (name, options, expected_size) in [
        ("explicit", vec!["--size", "80x60", "--fps", "30"], (80, 60)),
        ("defaults", vec![], (160, 120)),
    ] {
        let output = temp.path().join(format!("concat-{name}.mp4"));
        let mut args = vec!["video", "concat", p(&first), p(&second)];
        args.extend(options);
        let probe = backends.transform(&args, &output);
        dimensions(&probe, expected_size.0, expected_size.1);
        synchronized(&probe, 1.8);
        assert_eq!(stream(&probe, "video")["avg_frame_rate"], "30/1");
        let samples = backends.samples(&output);
        assert!(rms(&samples[4_800..38_400]) > 0.03);
        assert!(
            rms(&samples[67_200..76_800]) < 0.002,
            "无音轨的第二段必须补静音"
        );
    }
}

#[test]
fn regression_delayed_audio_keeps_its_position_on_the_video_timeline() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("delayed-audio.mp4");
    let generated = Command::new(&backends.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x120:rate=30:duration=1.5",
            "-itsoffset",
            "0.4",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=0.7",
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            "-t",
            "1.5",
            p(&input),
        ])
        .output()
        .unwrap();
    success(&generated);
    let original = backends.probe(&input);
    let start: f64 = stream(&original, "audio")["start_time"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (0.35..0.45).contains(&start),
        "fixture 必须确实带有延迟音轨: {original}"
    );
    for (name, options, silence_end, tone_start, total) in [
        ("compress", vec![], 0.30, 0.55, 1.5),
        (
            "trim",
            vec!["--start", "0.2", "--duration", "0.8"],
            0.13,
            0.35,
            0.8,
        ),
        ("speed", vec!["--factor", "2"], 0.12, 0.30, 0.75),
    ] {
        let output = temp.path().join(format!("{name}.mp4"));
        let mut args = vec!["video", name, p(&input)];
        args.extend(options);
        let probe = backends.transform(&args, &output);
        close(duration(&probe), total, 0.12);
        let samples = backends.timeline_samples(&output);
        let quiet = &samples[2_400..(silence_end * 48_000.0) as usize];
        assert!(rms(quiet) < 0.002, "{name} 提前播放了原本延迟的音轨");
        let start = (tone_start * 48_000.0) as usize;
        assert!(
            rms(&samples[start..start + 4_800]) > 0.03,
            "{name} 丢失原本应播放的音轨"
        );
    }
    let second = temp.path().join("silent.mp4");
    backends.fixture(&second, "160x120", "0.6", false);
    let output = temp.path().join("concat.mp4");
    let probe = backends.transform(&["video", "concat", p(&input), p(&second)], &output);
    close(duration(&probe), 2.1, 0.12);
    let samples = backends.timeline_samples(&output);
    assert!(
        rms(&samples[2_400..14_400]) < 0.002,
        "concat 提前播放了延迟音轨"
    );
    assert!(rms(&samples[26_400..31_200]) > 0.03);
}

#[test]
fn regression_nonzero_start_time_uses_relative_user_times() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.mp4");
    let shifted = temp.path().join("shifted.mp4");
    backends.fixture(&original, "160x120", "1.2", false);
    let result = Command::new(&backends.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-i",
            p(&original),
            "-c",
            "copy",
            "-output_ts_offset",
            "5",
            p(&shifted),
        ])
        .output()
        .unwrap();
    success(&result);
    close(
        stream(&backends.probe(&shifted), "video")["start_time"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
        5.0,
        0.01,
    );
    for (name, options, expected) in [
        ("compress", vec![], 1.2),
        ("trim", vec!["--start", "0.2", "--duration", "0.5"], 0.5),
    ] {
        let output = temp.path().join(format!("{name}.mp4"));
        let mut args = vec!["video", name, p(&shifted)];
        args.extend(options);
        let probe = backends.transform(&args, &output);
        close(duration(&probe), expected, 0.12);
        close(
            stream(&probe, "video")["start_time"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
            0.0,
            0.01,
        );
    }
    let screenshot = temp.path().join("at-0.2.png");
    backends.transform(
        &["video", "snapshot", p(&shifted), "--at", "0.2"],
        &screenshot,
    );
    assert_eq!(
        backends.pixels(&original, "0.2"),
        backends.pixels(&screenshot, "0")
    );
}

#[test]
fn regression_matroska_offset_does_not_extend_replacement_audio() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("offset.mkv");
    let generated = Command::new(&backends.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x120:rate=30:duration=1",
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
            "-output_ts_offset",
            "5",
            p(&input),
        ])
        .output()
        .unwrap();
    success(&generated);
    let raw = backends.probe(&input);
    assert!(
        stream(&raw, "video")["duration"].is_null(),
        "fixture 必须缺少逐轨 duration: {raw}"
    );
    close(duration(&raw), 6.0, 0.05);
    close(
        stream(&raw, "video")["start_time"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
        5.0,
        0.01,
    );
    let audio = temp.path().join("short.wav");
    backends.audio_fixture(&audio, "0.2");
    let output = temp.path().join("replaced.mp4");
    let probe = backends.transform(
        &["video", "audio", "replace", p(&input), "--audio", p(&audio)],
        &output,
    );
    // 容器结束时间 6 秒包含 5 秒 offset，替换音轨只能填到实际视频的 1 秒末尾。
    synchronized(&probe, 1.0);
}

#[test]
fn regression_audio_before_video_is_trimmed_to_the_video_origin() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("early-audio.mp4");
    // FFmpeg 6 的 MP4 默认 CFR 会在 0.4 秒偏移前补帧，使视频起点变为 0。
    // 显式保留输入时间戳，保证各后端版本都生成真正“音轨早于画面”的素材。
    let generated = Command::new(&backends.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-itsoffset",
            "0.4",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x120:rate=30:duration=0.8",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1.2",
            "-af",
            "volume=0:enable='lt(t,0.4)'",
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            "-fps_mode",
            "passthrough",
            p(&input),
        ])
        .output()
        .unwrap();
    success(&generated);
    let original = backends.probe(&input);
    let video_start: f64 = stream(&original, "video")["start_time"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (video_start - 0.4).abs() <= 0.02,
        "fixture 视频必须从 0.4 秒开始，不能被补帧提前到 0: {original}"
    );
    let audio_start: f64 = stream(&original, "audio")["start_time"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        audio_start.abs() <= 0.02,
        "fixture 音轨必须从 0 秒开始，才能验证视频起点前的音频被裁去: {original}"
    );
    let output = temp.path().join("compressed.mp4");
    let probe = backends.transform(&["video", "compress", p(&input)], &output);
    close(duration(&probe), 0.8, 0.12);
    let samples = backends.timeline_samples(&output);
    assert!(
        rms(&samples[4_800..12_000]) > 0.03,
        "必须裁去视频起点前0.4秒音频，不能在输出保留开头静音"
    );
}

#[test]
fn regression_rotation_metadata_and_sar_are_baked_into_display_dimensions() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let anamorphic = temp.path().join("sar.mp4");
    let rotated = temp.path().join("rotated.mp4");
    let generated = Command::new(&backends.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x120:rate=30:duration=0.6",
            "-vf",
            "setsar=2/1",
            "-c:v",
            "libx264",
            p(&anamorphic),
        ])
        .output()
        .unwrap();
    success(&generated);
    let remuxed = Command::new(&backends.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-display_rotation",
            "90",
            "-i",
            p(&anamorphic),
            "-c",
            "copy",
            p(&rotated),
        ])
        .output()
        .unwrap();
    success(&remuxed);
    let source = backends.probe(&rotated);
    assert_eq!(stream(&source, "video")["sample_aspect_ratio"], "2:1");
    assert_eq!(
        stream(&source, "video")["side_data_list"][0]["rotation"],
        90
    );
    for (name, options, extension, expected) in [
        ("compress", vec![], "mp4", (120, 320)),
        ("resize", vec!["--size", "80x60"], "mp4", (22, 60)),
        ("rotate", vec!["--angle", "90"], "mp4", (320, 120)),
        ("snapshot", vec!["--at", "0.2"], "png", (120, 320)),
    ] {
        let output = temp.path().join(format!("{name}.{extension}"));
        let mut args = vec!["video", name, p(&rotated)];
        args.extend(options);
        let probe = backends.transform(&args, &output);
        dimensions(&probe, expected.0, expected.1);
        let video = stream(&probe, "video");
        assert_eq!(video["sample_aspect_ratio"], "1:1");
        if let Some(side_data) = video["side_data_list"].as_array() {
            assert!(
                side_data
                    .iter()
                    .all(|data| data["rotation"].as_i64().unwrap_or(0) == 0)
            );
        }
    }
}

#[test]
fn real_audio_operations_change_samples_and_keep_video_duration() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.mp4");
    backends.fixture(&input, "160x120", "1.2", true);
    let muted = temp.path().join("muted.mp4");
    let probe = backends.transform(&["video", "mute", p(&input)], &muted);
    dimensions(&probe, 160, 120);
    assert!(
        probe["streams"]
            .as_array()
            .unwrap()
            .iter()
            .all(|stream| stream["codec_type"] != "audio")
    );
    for format in ["wav", "mp3", "m4a", "flac"] {
        let output = temp.path().join(format!("extracted.{format}"));
        let probe = backends.transform(
            &["video", "audio", "extract", p(&input), "--to", format],
            &output,
        );
        assert!(
            probe["streams"]
                .as_array()
                .unwrap()
                .iter()
                .all(|stream| stream["codec_type"] == "audio")
        );
        close(duration(&probe), 1.2, 0.12);
    }
    let quieter = temp.path().join("quieter.mp4");
    let probe = backends.transform(
        &["video", "audio", "volume", p(&input), "--factor", "0.5"],
        &quieter,
    );
    synchronized(&probe, 1.2);
    let original = backends.samples(&input);
    let lowered = backends.samples(&quieter);
    close(
        rms(&lowered[4_800..43_200]) / rms(&original[4_800..43_200]),
        0.5,
        0.04,
    );
    for (name, length) in [("short", "0.3"), ("long", "2.4")] {
        let audio = temp.path().join(format!("{name}.wav"));
        backends.audio_fixture(&audio, length);
        let output = temp.path().join(format!("replace-{name}.mp4"));
        let probe = backends.transform(
            &["video", "audio", "replace", p(&input), "--audio", p(&audio)],
            &output,
        );
        synchronized(&probe, 1.2);
        if name == "short" {
            let samples = backends.samples(&output);
            assert!(rms(&samples[2_400..9_600]) > 0.03);
            assert!(rms(&samples[28_800..48_000]) < 0.002);
        }
    }
}

#[test]
fn real_snapshot_and_gif_have_expected_pixels_and_timing() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.mp4");
    backends.fixture(&input, "160x120", "1.2", true);
    let snapshot = temp.path().join("frame.png");
    let probe = backends.transform(&["video", "snapshot", p(&input), "--at", "0.2"], &snapshot);
    assert_eq!(stream(&probe, "video")["codec_name"], "png");
    dimensions(&probe, 160, 120);
    assert_eq!(
        backends.pixels(&input, "0.2"),
        backends.pixels(&snapshot, "0")
    );
    let gif = temp.path().join("preview.gif");
    let probe = backends.transform(
        &[
            "video",
            "gif",
            p(&input),
            "--start",
            "0",
            "--duration",
            "0.5",
            "--width",
            "80",
            "--fps",
            "10",
        ],
        &gif,
    );
    assert_eq!(stream(&probe, "video")["codec_name"], "gif");
    dimensions(&probe, 80, 60);
    close(duration(&probe), 0.5, 0.11);
    assert!(
        probe["streams"]
            .as_array()
            .unwrap()
            .iter()
            .all(|stream| stream["codec_type"] != "audio")
    );
}

#[test]
fn real_failures_and_overwrite_policy_keep_original_files_intact() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.mp4");
    let output = temp.path().join("existing.mp4");
    backends.fixture(&input, "160x120", "1.2", true);
    let original = std::fs::read(&input).unwrap();
    std::fs::write(&output, b"keep existing output").unwrap();
    let refused = backends.cli(&["video", "compress", p(&input), "-o", p(&output)]);
    assert!(!refused.status.success());
    assert_eq!(std::fs::read(&output).unwrap(), b"keep existing output");
    let same_file = backends.cli(&[
        "video",
        "compress",
        p(&input),
        "-o",
        p(&input),
        "--overwrite",
    ]);
    assert!(!same_file.status.success());
    assert_eq!(std::fs::read(&input).unwrap(), original);
    // 同一 inode 的硬链接也不能让 overwrite 变成原地编辑。
    let alias = temp.path().join("source-alias.mp4");
    std::fs::hard_link(&input, &alias).unwrap();
    let same_inode = backends.cli(&[
        "video",
        "compress",
        p(&input),
        "-o",
        p(&alias),
        "--overwrite",
    ]);
    assert!(!same_inode.status.success());
    assert_eq!(std::fs::read(&input).unwrap(), original);
    let corrupt = temp.path().join("corrupt.mp4");
    std::fs::write(&corrupt, b"not a media file").unwrap();
    let rejected = backends.cli(&[
        "video",
        "compress",
        p(&corrupt),
        "-o",
        p(&output),
        "--overwrite",
    ]);
    assert!(!rejected.status.success());
    assert_eq!(std::fs::read(&output).unwrap(), b"keep existing output");
    // faststart 让元数据位于文件头；截断媒体数据后 ffprobe 仍可读取结构，编码必须失败。
    let truncated = temp.path().join("truncated.mp4");
    let remuxed = Command::new(&backends.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-i",
            p(&input),
            "-c",
            "copy",
            "-movflags",
            "+faststart",
            p(&truncated),
        ])
        .output()
        .unwrap();
    success(&remuxed);
    let bytes = std::fs::read(&truncated).unwrap();
    std::fs::write(&truncated, &bytes[..bytes.len() * 2 / 3]).unwrap();
    assert_eq!(
        backends.probe(&truncated)["streams"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let rejected = backends.cli(&[
        "video",
        "compress",
        p(&truncated),
        "-o",
        p(&output),
        "--overwrite",
    ]);
    assert!(
        !rejected.status.success(),
        "结构可读但已截断的输入不能发布不完整结果"
    );
    assert_eq!(std::fs::read(&output).unwrap(), b"keep existing output");
    for variable in ["KITO_FFMPEG", "KITO_FFPROBE"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kito"));
        command
            .env("KITO_FFMPEG", &backends.ffmpeg)
            .env("KITO_FFPROBE", &backends.ffprobe);
        let rejected = command
            .env(variable, temp.path().join("missing-backend"))
            .args([
                "video",
                "compress",
                p(&input),
                "-o",
                p(&output),
                "--overwrite",
            ])
            .output()
            .unwrap();
        assert!(!rejected.status.success());
        assert_eq!(std::fs::read(&output).unwrap(), b"keep existing output");
    }
    let probe = backends.transform(&["video", "compress", p(&input), "--overwrite"], &output);
    synchronized(&probe, 1.2);
    assert_eq!(std::fs::read(input).unwrap(), original);
}

#[test]
fn real_invalid_values_fail_before_creating_outputs() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.mp4");
    backends.fixture(&input, "160x120", "1.2", true);
    let original = std::fs::read(&input).unwrap();
    for (index, command) in [
        vec!["speed", p(&input), "--factor", "NaN"],
        vec!["speed", p(&input), "--factor", "0"],
        vec!["speed", p(&input), "--factor", "inf"],
        vec!["trim", p(&input), "--start", "2", "--duration", "0.5"],
        vec!["trim", p(&input), "--start", "0.7", "--end", "0.5"],
        vec!["trim", p(&input), "--duration", "0"],
        vec!["trim", p(&input), "--start", "0.2", "--end", "2"],
        vec!["crop", p(&input), "--width", "200", "--height", "48"],
        vec![
            "crop",
            p(&input),
            "--width",
            "64",
            "--height",
            "48",
            "--x",
            "150",
        ],
        vec!["resize", p(&input), "--size", "0x60"],
        vec!["snapshot", p(&input), "--at", "2"],
        vec!["snapshot", p(&input), "--at", "NaN"],
        vec!["gif", p(&input), "--duration", "0"],
        vec!["gif", p(&input), "--start", "2", "--duration", "0.5"],
        vec!["audio", "volume", p(&input), "--factor", "NaN"],
    ]
    .iter()
    .enumerate()
    {
        let extension = match command[0] {
            "snapshot" => "png",
            "gif" => "gif",
            _ => "mp4",
        };
        let output = temp.path().join(format!("invalid-{index}.{extension}"));
        let mut args = vec!["video"];
        args.extend(command);
        args.extend(["-o", p(&output)]);
        let result = backends.cli(&args);
        assert!(
            !result.status.success(),
            "invalid command was accepted: {args:?}"
        );
        assert!(
            !output.exists(),
            "invalid command published output: {args:?}"
        );
        assert_eq!(std::fs::read(&input).unwrap(), original);
    }
}

#[test]
fn real_dry_run_describes_filters_without_writing_output() {
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.mp4");
    backends.fixture(&input, "160x120", "1.2", true);
    let output = temp.path().join("new-directory/preview.mp4");
    let result = backends.cli(&[
        "video",
        "resize",
        p(&input),
        "--size",
        "80x60",
        "-o",
        p(&output),
        "--dry-run",
        "--json",
    ]);
    success(&result);
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    let plan = report.to_string();
    assert!(plan.contains("scale"), "{plan}");
    assert!(plan.contains("80"), "{plan}");
    assert!(plan.contains("libx264"), "{plan}");
    assert!(!output.exists());
    assert!(!output.parent().unwrap().exists());
}

#[cfg(unix)]
#[test]
fn real_timed_out_encode_never_replaces_existing_output() {
    use std::os::unix::fs::PermissionsExt;
    let Some(backends) = Backends::discover() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.mp4");
    backends.fixture(&input, "160x120", "3", true);
    let output = temp.path().join("existing.mp4");
    std::fs::write(&output, b"keep existing output").unwrap();
    // 仍运行真实 FFmpeg，只用 -re 按实时速率读取，稳定触发一秒 deadline。
    // 版本/能力探测没有 -i，仍按真实后端原速执行，不伪造可用性或编码结果。
    let paced = temp.path().join("paced-ffmpeg");
    std::fs::write(&paced, "#!/bin/sh\nfor argument do\n  if [ \"$argument\" = \"-i\" ]; then\n    exec \"$KITO_TEST_REAL_FFMPEG\" -re \"$@\"\n  fi\ndone\nexec \"$KITO_TEST_REAL_FFMPEG\" \"$@\"\n").unwrap();
    std::fs::set_permissions(&paced, std::fs::Permissions::from_mode(0o755)).unwrap();
    let started = std::time::Instant::now();
    let result = Command::new(env!("CARGO_BIN_EXE_kito"))
        .env("KITO_FFMPEG", &paced)
        .env("KITO_FFPROBE", &backends.ffprobe)
        .env("KITO_TEST_REAL_FFMPEG", &backends.ffmpeg)
        .args([
            "video",
            "compress",
            p(&input),
            "-o",
            p(&output),
            "--overwrite",
            "--time-limit",
            "1",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(started.elapsed() < std::time::Duration::from_secs(8));
    assert_eq!(std::fs::read(&output).unwrap(), b"keep existing output");
    let entries: Vec<_> = std::fs::read_dir(temp.path()).unwrap().collect();
    assert_eq!(entries.len(), 3, "失败后不能遗留临时编码文件");
}

#[test]
fn backend_discovery_child() {
    if std::env::var_os("KITO_TEST_VIDEO_DISCOVERY").is_none() {
        return;
    }
    assert!(Backends::discover().is_none());
}

#[test]
fn explicit_backend_errors_and_required_backends_never_skip() {
    let temp = tempfile::tempdir().unwrap();
    for strict in [
        None,
        Some("KITO_FFMPEG"),
        Some("KITO_FFPROBE"),
        Some("KITO_REQUIRE_BACKEND"),
    ] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "backend_discovery_child"])
            .env("KITO_TEST_VIDEO_DISCOVERY", "1")
            .env("PATH", temp.path())
            .env_remove("KITO_FFMPEG")
            .env_remove("KITO_FFPROBE")
            .env_remove("KITO_REQUIRE_BACKEND");
        if let Some(variable) = strict {
            command.env(
                variable,
                if variable == "KITO_REQUIRE_BACKEND" {
                    OsStr::new("1")
                } else {
                    temp.path().as_os_str()
                },
            );
        }
        let result = command.output().unwrap();
        assert_eq!(
            result.status.success(),
            strict.is_none(),
            "strict={strict:?}: {}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
}
