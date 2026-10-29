//! 声音工具使用真实 CLI 和文件字节验证；不依赖声卡、FFmpeg 或开发机素材。
use std::{
    fs,
    io::Write,
    process::{Command, Output, Stdio},
};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kito"))
        .arg("sound")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}
fn ok(args: &[&str]) -> serde_json::Value {
    let out = run(args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn samples(bytes: &[u8]) -> Vec<i16> {
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8,
        bytes.len()
    );
    assert_eq!(u16::from_le_bytes(bytes[20..22].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(bytes[34..36].try_into().unwrap()), 16);
    assert_eq!(
        u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize + 44,
        bytes.len()
    );
    bytes[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect()
}

#[test]
fn tone_produces_correct_pitch_frames_and_machine_report() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("tone.wav");
    let report = ok(&[
        "tone",
        "A4",
        "--duration",
        "1",
        "-o",
        path.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(report["format"], "wav");
    assert_eq!(report["frames"], 44100);
    assert_eq!(report["sample_rate"], 44100);
    let pcm = samples(&fs::read(path).unwrap());
    assert_eq!(pcm.len(), 44100);
    assert_eq!((pcm[0], pcm[44099]), (0, 0));
    let crossings = pcm.windows(2).filter(|w| w[0] <= 0 && w[1] > 0).count();
    assert!((439..=441).contains(&crossings), "{crossings}");
    assert!(report["peak"].as_f64().unwrap() > 0.1);
}

#[test]
fn melody_chords_rests_and_midi_are_real_outputs() {
    let d = tempfile::tempdir().unwrap();
    let wav = d.path().join("melody.wav");
    let report = ok(&[
        "melody",
        "C4+E4+G4:1 R:1",
        "--bpm",
        "120",
        "--repeat",
        "2",
        "-o",
        wav.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(report["event_count"], 6);
    assert_eq!(report["duration_seconds"], 2.0);
    let pcm = samples(&fs::read(wav).unwrap());
    assert_eq!(pcm.len(), 88200);
    assert!(pcm[23000..43000].iter().all(|&n| n == 0));
    let midi = d.path().join("melody.mid");
    let report = ok(&[
        "melody",
        "C4 E4 G4:2 R:1",
        "-o",
        midi.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(report["format"], "midi");
    assert!(report["sample_rate"].is_null());
    let bytes = fs::read(midi).unwrap();
    assert_eq!(&bytes[..14], b"MThd\0\0\0\x06\0\0\0\x01\x01\xe0");
    assert!(
        bytes
            .windows(6)
            .any(|w| w == [0xff, 0x51, 3, 7, 0xa1, 0x20])
    );
    assert!(bytes.ends_with(&[0xff, 0x2f, 0]));
}

#[test]
fn every_preset_renders_and_seed_controls_noise() {
    let d = tempfile::tempdir().unwrap();
    for preset in [
        "notify",
        "success",
        "error",
        "coin",
        "jump",
        "laser",
        "explosion",
        "whoosh",
    ] {
        let p = d.path().join(format!("{preset}.wav"));
        ok(&["sfx", preset, "-o", p.to_str().unwrap(), "--json"]);
        assert!(samples(&fs::read(p).unwrap()).iter().any(|&s| s != 0));
    }
    let mut outputs = Vec::new();
    for (i, seed) in ["42", "42", "43"].iter().enumerate() {
        let p = d.path().join(format!("seed{i}.wav"));
        ok(&[
            "sfx",
            "explosion",
            "--seed",
            seed,
            "-o",
            p.to_str().unwrap(),
            "--json",
        ]);
        outputs.push(fs::read(p).unwrap());
    }
    assert_eq!(outputs[0], outputs[1]);
    assert_ne!(outputs[0], outputs[2]);
}

#[test]
fn beat_pattern_controls_timing_and_silence() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("beat.wav");
    let report = ok(&[
        "beat",
        "--bars",
        "1",
        "--bpm",
        "120",
        "--kick",
        "x---------------",
        "--snare",
        "----------------",
        "--hat",
        "----------------",
        "-o",
        p.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(report["duration_seconds"], 2.0);
    assert_eq!(report["event_count"], 1);
    let pcm = samples(&fs::read(p).unwrap());
    assert!(pcm[..10000].iter().any(|&n| n != 0));
    assert!(pcm[44100..].iter().all(|&n| n == 0));
}

#[test]
fn dry_run_does_not_create_directories_and_output_defaults_are_safe() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("missing/deep/tone.wav");
    let report = ok(&["tone", "--dry-run", "-o", p.to_str().unwrap(), "--json"]);
    assert_eq!(report["dry_run"], true);
    assert!(!d.path().join("missing").exists());
    let out = Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(["sound", "sfx", "coin", "--json"])
        .current_dir(d.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(d.path().join("coin.wav").exists());
}

#[test]
fn failures_preserve_files_and_emit_no_partial_report() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("keep.wav");
    fs::write(&p, b"KEEP").unwrap();
    let invalid = [
        vec!["tone", "NaN"],
        vec!["tone", "--duration", "inf"],
        vec!["tone", "--volume", "NaN"],
        vec!["tone", "--duration", "1000000000"],
        vec!["tone", "--sample-rate", "1"],
        vec!["melody", "C4 nope"],
        vec!["melody", "C4:0"],
        vec!["melody", "C4", "--bpm", "0"],
        vec!["beat", "--kick", "x---"],
        vec!["beat", "--bars", "0"],
        vec!["sfx", "coin", "--pitch", "25"],
    ];
    for mut args in invalid {
        args.extend(["-o", p.to_str().unwrap(), "--overwrite", "--json"]);
        let out = run(&args);
        assert!(!out.status.success(), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
        assert_eq!(fs::read(&p).unwrap(), b"KEEP");
    }
    assert!(!run(&["tone", "-o", p.to_str().unwrap()]).status.success());
    assert_eq!(fs::read(&p).unwrap(), b"KEEP");
    ok(&[
        "tone",
        "--duration",
        "0.01",
        "-o",
        p.to_str().unwrap(),
        "--overwrite",
        "--json",
    ]);
    assert_eq!(&fs::read(p).unwrap()[..4], b"RIFF");
}

#[test]
fn score_file_and_stdin_preserve_strict_input_and_protect_source() {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("score.wav");
    fs::write(&input, "C4 E4 G4").unwrap();
    let out = run(&[
        "melody",
        "--file",
        input.to_str().unwrap(),
        "-o",
        input.to_str().unwrap(),
        "--overwrite",
    ]);
    assert!(!out.status.success());
    assert_eq!(fs::read(&input).unwrap(), b"C4 E4 G4");
    let dest = d.path().join("stdin.wav");
    let mut child = Command::new(env!("CARGO_BIN_EXE_kito"))
        .args([
            "sound",
            "melody",
            "--file",
            "-",
            "-o",
            dest.to_str().unwrap(),
            "--json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"C4\nE4 G4").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["event_count"],
        3
    );
    fs::write(&input, [0xff, 0xfe]).unwrap();
    assert!(
        !run(&[
            "melody",
            "--file",
            input.to_str().unwrap(),
            "-o",
            dest.to_str().unwrap(),
            "--overwrite"
        ])
        .status
        .success()
    );
}

#[test]
fn unsupported_formats_and_ignored_midi_controls_are_rejected() {
    let d = tempfile::tempdir().unwrap();
    for (command, filename, extra) in [
        ("tone", "a.mp3", vec![]),
        ("beat", "a.mid", vec![]),
        ("melody", "a.mid", vec!["C4", "--volume", "0.5"]),
        ("melody", "a.mid", vec!["C4", "--instrument", "bell"]),
        ("melody", "a.mid", vec!["C4", "--sample-rate", "44100"]),
    ] {
        let p = d.path().join(filename);
        let mut args = vec![command];
        args.extend(extra);
        args.extend(["-o", p.to_str().unwrap()]);
        assert!(!run(&args).status.success(), "{args:?}");
        assert!(!p.exists());
    }
}

#[test]
fn help_and_catalog_make_all_creation_paths_discoverable() {
    let out = run(&["--help"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    for term in [
        "Echo",
        "presets",
        "sfx",
        "tone",
        "melody",
        "beat",
        "kito sound",
    ] {
        assert!(text.contains(term), "{term}: {text}");
    }
    let report = ok(&["presets", "--json"]);
    assert_eq!(report["effects"].as_array().unwrap().len(), 8);
    assert_eq!(report["instruments"].as_array().unwrap().len(), 5);
    assert_eq!(report["beats"].as_array().unwrap().len(), 3);
}

#[test]
fn beat_accepts_exact_120_seconds_without_float_rounding_rejection() {
    for (bpm, bars) in [("58", "29"), ("116", "58")] {
        let report = ok(&["beat", "--bpm", bpm, "--bars", bars, "--dry-run", "--json"]);
        assert_eq!(report["duration_seconds"], 120.0);
        assert_eq!(report["frames"], 5292000);
    }
}

#[cfg(unix)]
#[test]
fn symlink_and_hardlinked_score_are_never_overwritten() {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("score.txt");
    let link = d.path().join("score.wav");
    fs::write(&input, "C4").unwrap();
    std::os::unix::fs::symlink(&input, &link).unwrap();
    assert!(
        !run(&["tone", "-o", link.to_str().unwrap(), "--overwrite"])
            .status
            .success()
    );
    fs::remove_file(&link).unwrap();
    fs::hard_link(&input, &link).unwrap();
    assert!(
        !run(&[
            "melody",
            "--file",
            input.to_str().unwrap(),
            "-o",
            link.to_str().unwrap(),
            "--overwrite"
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(input).unwrap(), b"C4");
}

#[cfg(unix)]
#[test]
fn dry_run_rejects_a_dangling_parent_symlink_without_creating_files() {
    let d = tempfile::tempdir().unwrap();
    let parent = d.path().join("dangling");
    std::os::unix::fs::symlink(d.path().join("missing"), &parent).unwrap();
    let output = parent.join("tone.wav");
    let result = run(&[
        "tone",
        "-o",
        output.to_str().unwrap(),
        "--dry-run",
        "--json",
    ]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!d.path().join("missing").exists());
}
