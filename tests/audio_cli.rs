use serde_json::Value;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct AudioFixture {
    _directory: tempfile::TempDir,
    first: PathBuf,
    second: PathBuf,
    ffmpeg: OsString,
    ffprobe: OsString,
}

impl AudioFixture {
    fn new() -> Option<Self> {
        let ffmpeg = std::env::var_os("KITO_FFMPEG").unwrap_or_else(|| "ffmpeg".into());
        let ffprobe = std::env::var_os("KITO_FFPROBE").unwrap_or_else(|| "ffprobe".into());
        let available = [&ffmpeg, &ffprobe].into_iter().all(|program| {
            Command::new(program)
                .arg("-version")
                .output()
                .is_ok_and(|output| output.status.success())
        });
        assert!(
            available || std::env::var_os("KITO_REQUIRE_BACKEND").is_none(),
            "需要真实 FFmpeg 和 ffprobe 后端"
        );
        if !available {
            return None;
        }
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first ; [audio].wav");
        let second = directory.path().join("second.wav");
        for (path, length, frequency) in [(&first, "2", "440"), (&second, "1", "660")] {
            let output = Command::new(&ffmpeg)
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("sine=frequency={frequency}:sample_rate=44100"),
                    "-t",
                    length,
                    "-c:a",
                    "pcm_s16le",
                    "-metadata",
                    "title=discard source title",
                ])
                .arg(path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Some(Self {
            _directory: directory,
            first,
            second,
            ffmpeg,
            ffprobe,
        })
    }

    fn output(&self, name: &str) -> PathBuf {
        self._directory.path().join(name)
    }

    fn command(&self, command: &str, inputs: &[&Path], options: &[&str], output: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_kito"))
            .env("KITO_FFMPEG", &self.ffmpeg)
            .env("KITO_FFPROBE", &self.ffprobe)
            .args(["video", "audio", command])
            .args(inputs)
            .args(options)
            .arg("-o")
            .arg(output)
            .arg("--json")
            .output()
            .unwrap()
    }

    fn run(&self, command: &str, inputs: &[&Path], options: &[&str], output: &Path) -> Value {
        let result = self.command(command, inputs, options, output);
        assert!(
            result.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice(&result.stdout).unwrap()
    }

    fn samples(&self, path: &Path) -> Vec<i16> {
        let result = Command::new(&self.ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(path)
            .args([
                "-map", "0:a:0", "-ac", "1", "-ar", "48000", "-f", "s16le", "-",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result
            .stdout
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| i16::from_le_bytes(*v))
            .collect()
    }
}

fn assert_audio(report: &Value, duration: f64) {
    assert_eq!(report["verified_decodable"], true, "{report}");
    assert_eq!(report["lossy"], true, "{report}");
    let streams = report["output_info"]["streams"].as_array().unwrap();
    assert_eq!(streams.len(), 1, "{report}");
    assert_eq!(streams[0]["codec_type"], "audio");
    assert_eq!(streams[0]["sample_rate"], 48000);
    assert_eq!(streams[0]["channels"], 2);
    assert!(
        (report["output_info"]["duration"].as_f64().unwrap() - duration).abs() < 0.08,
        "{report}"
    );
}

#[test]
fn audio_conversion_supports_all_formats_and_discards_source_metadata() {
    let Some(fixture) = AudioFixture::new() else {
        return;
    };
    for (format, codec) in [
        ("mp3", "mp3"),
        ("m4a", "aac"),
        ("wav", "pcm_s16le"),
        ("flac", "flac"),
    ] {
        let output = fixture.output(&format!("converted.{format}"));
        let report = fixture.run("convert", &[&fixture.first], &["--to", format], &output);
        assert_audio(&report, 2.0);
        assert_eq!(report["output_info"]["streams"][0]["codec"], codec);
        let raw = Command::new(&fixture.ffprobe)
            .args(["-v", "error", "-show_format", "-of", "json"])
            .arg(output)
            .output()
            .unwrap();
        let metadata: Value = serde_json::from_slice(&raw.stdout).unwrap();
        assert!(metadata["format"]["tags"]["title"].is_null());
    }
}

#[test]
fn audio_editing_commands_have_expected_duration_and_only_an_audio_stream() {
    let Some(fixture) = AudioFixture::new() else {
        return;
    };
    for (command, extra, expected) in [
        ("extract", vec![], 2.0),
        (
            "trim",
            vec!["--start", "00:00.25", "--duration", "0.75"],
            0.75,
        ),
        ("speed", vec!["--factor", "2"], 1.0),
        ("volume", vec!["--factor", "0.5"], 2.0),
        ("fade", vec!["--fade-in", "0.5", "--fade-out", "0.5"], 2.0),
        ("normalize", vec!["--loudness", "-16"], 2.0),
    ] {
        let output = fixture.output(&format!("{command}.wav"));
        let mut options = vec!["--to", "wav"];
        options.extend(extra);
        let report = fixture.run(command, &[&fixture.first], &options, &output);
        assert_audio(&report, expected);
    }
    let concat = fixture.run(
        "concat",
        &[&fixture.first, &fixture.second],
        &["--to", "wav"],
        &fixture.output("concat.wav"),
    );
    assert_audio(&concat, 3.0);
    let mix = fixture.run(
        "mix",
        &[&fixture.first, &fixture.second],
        &["--to", "wav"],
        &fixture.output("mix.wav"),
    );
    assert_audio(&mix, 2.0);
}

fn mean_square(samples: &[i16]) -> f64 {
    samples.iter().map(|&v| f64::from(v).powi(2)).sum::<f64>() / samples.len() as f64
}

#[test]
fn fade_changes_both_edges_and_mix_contains_both_source_frequencies() {
    let Some(fixture) = AudioFixture::new() else {
        return;
    };
    let fade = fixture.output("fade.wav");
    fixture.run(
        "fade",
        &[&fixture.first],
        &["--to", "wav", "--fade-in", "0.5", "--fade-out", "0.5"],
        &fade,
    );
    let samples = fixture.samples(&fade);
    let middle = mean_square(&samples[24000..36000]);
    assert!(mean_square(&samples[..2400]) < middle / 30.0);
    assert!(mean_square(&samples[samples.len() - 2400..]) < middle / 30.0);
    let mixed = fixture.output("mix.wav");
    fixture.run(
        "mix",
        &[&fixture.first, &fixture.second],
        &["--to", "wav"],
        &mixed,
    );
    let samples = fixture.samples(&mixed);
    // 用独立的正弦投影确认两条输入都存在，避免“只转码第一轨”也通过时长测试。
    let amplitude = |frequency: f64| {
        let (sin, cos) =
            samples[4800..24000]
                .iter()
                .enumerate()
                .fold((0.0, 0.0), |(s, c), (i, &value)| {
                    let phase = 2.0 * std::f64::consts::PI * frequency * i as f64 / 48000.0;
                    (
                        s + f64::from(value) * phase.sin(),
                        c + f64::from(value) * phase.cos(),
                    )
                });
        sin.hypot(cos) / 19200.0
    };
    assert!(amplitude(440.0) > 300.0);
    assert!(amplitude(660.0) > 300.0);
    assert!(amplitude(1000.0) < 20.0);
}

#[test]
fn invalid_audio_requests_preserve_existing_outputs_and_all_inputs() {
    let Some(fixture) = AudioFixture::new() else {
        return;
    };
    let protected = fixture.output("protected.wav");
    fs::write(&protected, b"existing output").unwrap();
    for (command, options) in [
        ("speed", vec!["--factor", "NaN"]),
        ("speed", vec!["--factor", "0"]),
        ("volume", vec!["--factor", "17"]),
        ("fade", vec!["--fade-in", "3"]),
        ("fade", vec!["--fade-in", "0", "--fade-out", "0"]),
        ("fade", vec!["--fade-in", "1;anull"]),
        ("normalize", vec!["--loudness", "NaN"]),
        ("normalize", vec!["--loudness", "-71"]),
        ("trim", vec!["--start", "1.5", "--duration", "1"]),
    ] {
        let mut options = options;
        options.extend(["--to", "wav", "--overwrite"]);
        let result = fixture.command(command, &[&fixture.first], &options, &protected);
        assert!(
            !result.status.success(),
            "{command} accepted invalid parameters"
        );
        assert_eq!(fs::read(&protected).unwrap(), b"existing output");
    }
    let original = fs::read(&fixture.second).unwrap();
    let result = fixture.command(
        "concat",
        &[&fixture.first, &fixture.second],
        &["--to", "wav", "--overwrite"],
        &fixture.second,
    );
    assert!(!result.status.success());
    assert_eq!(fs::read(&fixture.second).unwrap(), original);
    let result = fixture.command(
        "convert",
        &[&fixture.first],
        &["--to", "mp3", "--overwrite"],
        &protected,
    );
    assert!(!result.status.success());
    assert_eq!(fs::read(&protected).unwrap(), b"existing output");
}

#[test]
fn audio_dry_run_does_not_create_destination_and_volume_defaults_to_mp3() {
    let Some(fixture) = AudioFixture::new() else {
        return;
    };
    let output = fixture.output("missing/subdir/volume.mp3");
    let report = fixture.run(
        "volume",
        &[&fixture.first],
        &["--factor", "0.25", "--dry-run"],
        &output,
    );
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["operation"], "volume");
    assert!(!output.parent().unwrap().exists());
    assert!(
        report["args"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "libmp3lame")
    );
}

#[test]
fn audio_operations_select_first_track_and_use_its_duration_in_a_video_container() {
    let Some(fixture) = AudioFixture::new() else {
        return;
    };
    let source = fixture.output("multiple-audio.mp4");
    let built = Command::new(&fixture.ffmpeg)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=64x64:r=10:d=2.5",
            "-i",
        ])
        .arg(&fixture.second)
        .arg("-i")
        .arg(&fixture.first)
        .args([
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-map",
            "2:a",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            "-metadata:s:a:0",
            "title=discard audio title",
        ])
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = fixture.output("first-track.m4a");
    let report = fixture.run("convert", &[&source], &["--to", "m4a"], &output);
    assert_audio(&report, 1.0);
    let raw = Command::new(&fixture.ffprobe)
        .args(["-v", "error", "-show_streams", "-of", "json"])
        .arg(output)
        .output()
        .unwrap();
    let metadata: Value = serde_json::from_slice(&raw.stdout).unwrap();
    assert!(metadata["streams"][0]["tags"]["title"].is_null());
    let destination = fixture.output("invalid-trim.wav");
    let result = fixture.command(
        "trim",
        &[&source],
        &["--start", "0.75", "--duration", "0.5", "--to", "wav"],
        &destination,
    );
    assert!(!result.status.success(), "first audio is only one second");
    assert!(!destination.exists());
    let result = fixture.command(
        "fade",
        &[&source],
        &["--fade-out", "1.5", "--to", "wav"],
        &destination,
    );
    assert!(!result.status.success());
    assert!(!destination.exists());
}

#[test]
fn audio_concat_order_volume_gain_and_speed_pitch_match_the_requested_operation() {
    let Some(fixture) = AudioFixture::new() else {
        return;
    };
    let amplitude = |samples: &[i16], frequency: f64| {
        let (sin, cos) = samples
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(s, c), (i, &value)| {
                let phase = 2.0 * std::f64::consts::PI * frequency * i as f64 / 48000.0;
                (
                    s + f64::from(value) * phase.sin(),
                    c + f64::from(value) * phase.cos(),
                )
            });
        sin.hypot(cos) / samples.len() as f64
    };
    let concatenated = fixture.output("ordered.wav");
    fixture.run(
        "concat",
        &[&fixture.first, &fixture.second],
        &["--to", "wav"],
        &concatenated,
    );
    let samples = fixture.samples(&concatenated);
    assert!(amplitude(&samples[4800..24000], 440.0) > 1000.0);
    assert!(amplitude(&samples[4800..24000], 660.0) < 20.0);
    assert!(amplitude(&samples[105600..124800], 660.0) > 1000.0);
    assert!(amplitude(&samples[105600..124800], 440.0) < 20.0);

    let quieter = fixture.output("quieter.wav");
    fixture.run(
        "volume",
        &[&fixture.first],
        &["--to", "wav", "--factor", "0.5"],
        &quieter,
    );
    // 同时比较的是音量效果；先生成相同 48 kHz 双声道基准，隔离 mono→stereo 重混增益。
    let baseline = fixture.output("volume-baseline.wav");
    fixture.run("convert", &[&fixture.first], &["--to", "wav"], &baseline);
    let source_samples = fixture.samples(&baseline);
    let quieter_samples = fixture.samples(&quieter);
    let power_ratio =
        mean_square(&quieter_samples[4800..24000]) / mean_square(&source_samples[4800..24000]);
    assert!((power_ratio - 0.25).abs() < 0.01, "{power_ratio}");

    let faster = fixture.output("faster.wav");
    fixture.run(
        "speed",
        &[&fixture.first],
        &["--to", "wav", "--factor", "2"],
        &faster,
    );
    let samples = fixture.samples(&faster);
    assert!(amplitude(&samples[4800..24000], 440.0) > 1000.0);
    assert!(amplitude(&samples[4800..24000], 880.0) < 20.0);
}
