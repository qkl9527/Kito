//! 参数层只规划新声音；完成全部校验后才渲染，最后复用共享原子文件发布。
use super::{
    pattern::{self, BeatPreset, Effect, Timbre},
    score,
    synth::{self, Voice},
};
use crate::io_args::{InputArgs, OutputArgs, write_stdout};
use anyhow::{Context, Result, ensure};
use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Args)]
#[command(
    after_help = "使用案例:\n  查看音效、乐器与鼓型\n    kito sound presets\n  一键生成成功提示声\n    kito sound sfx success\n  按音名生成 1 秒单音\n    kito sound tone A4 --duration 1\n  用音名写旋律与和弦\n    kito sound melody \"C4 E4 G4 C4+E4+G4:2\"\n  生成四小节鼓点\n    kito sound beat house --bpm 120 --bars 4\n\n无需 Node.js、声卡或 FFmpeg。输出新 WAV；melody 也支持 MIDI。详细教程：docs/tools/sound.md"
)]
pub struct SoundArgs {
    #[command(subcommand)]
    command: SoundCommand,
}

#[derive(Subcommand)]
enum SoundCommand {
    #[command(
        about = "查看音效、乐器和鼓型及其用途",
        after_help = "使用案例:\n  kito sound presets\n  kito sound presets --json"
    )]
    Presets {
        #[arg(long, help = "输出 JSON 目录")]
        json: bool,
    },
    #[command(
        about = "一键创作通知、游戏和转场音效",
        after_help = "使用案例:\n  kito sound sfx coin\n  kito sound sfx laser --duration 0.6 --pitch -5 -o laser.wav\n  kito sound sfx explosion --seed 42 --json\n\n时长单位为秒；pitch 为半音，+12 高一个八度。"
    )]
    Sfx(SfxArgs),
    #[command(
        about = "按音名或 Hz 生成单音",
        after_help = "使用案例:\n  kito sound tone\n  kito sound tone 440Hz --duration 2\n  kito sound tone Bb3 --instrument bell -o bell.wav\n\n音名包含八度：C4 是中央 C，A4 是 440 Hz；支持 F#4、Bb3。"
    )]
    Tone(ToneArgs),
    #[command(
        about = "用音符、和弦和休止符写旋律，导出 WAV 或 MIDI",
        after_help = "使用案例:\n  kito sound melody \"C4 D4:0.5 E4:0.5 G4:2\" --bpm 120\n  kito sound melody \"C4+E4+G4:2 R:1 C5\" --instrument bell\n  kito sound melody --file score.txt -o song.mid\n  kito sound melody --file - --repeat 2 -o song.wav\n\n空格分隔事件，: 后为拍数（默认 1），+ 组合和弦，R 是休止，独立 | 分隔小节。\n一拍是四分音符。输出 .mid/.midi 为 MIDI（只保存音符与节奏，不保存合成音色）。\nMIDI 不接受 --instrument、--sample-rate、--volume、--seed。"
    )]
    Melody(MelodyArgs),
    #[command(
        about = "用预设或 16 格节奏生成鼓点",
        after_help = "使用案例:\n  kito sound beat\n  kito sound beat hiphop --bpm 90 --bars 8\n  kito sound beat --kick x-------x------- --hat x-x-x-x-x-x-x-x-\n\n每轨恰好 16 格（4/4 一小节），x/X 敲击，- 或 . 休止。\n自定义 kick/snare/hat 只替换预设对应轨，全部休止可用 ----------------。"
    )]
    Beat(BeatArgs),
}

#[derive(Args, Default)]
struct OutputOptions {
    /// 输出文件；默认按命令命名，不自动播放。仅 melody 支持 .mid/.midi。
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// 允许原子替换已有结果，仍不允许覆盖乐谱输入或符号链接。
    #[arg(long)]
    overwrite: bool,
    /// 完整校验并显示计划，不合成、不创建文件或目录。
    #[arg(long)]
    dry_run: bool,
    /// 输出一个 JSON 报告，错误仅写 stderr。
    #[arg(long)]
    json: bool,
}

#[derive(Args, Default)]
struct WaveOptions {
    /// WAV 采样率：22050、44100、48000 Hz；默认 44100。
    #[arg(long, value_name = "HZ")]
    sample_rate: Option<u32>,
    /// 新声音输出音量 0–1；默认 0.7，不用于修改现有音频。
    #[arg(long, value_name = "0..1")]
    volume: Option<f64>,
    /// 合成噪声种子；默认 0。同一构建、参数与种子可重复生成。
    #[arg(long)]
    seed: Option<u64>,
}
impl WaveOptions {
    fn rate(&self) -> u32 {
        self.sample_rate.unwrap_or(44100)
    }
    fn volume(&self) -> f64 {
        self.volume.unwrap_or(0.7)
    }
    fn seed(&self) -> u64 {
        self.seed.unwrap_or(0)
    }
    fn explicit(&self) -> bool {
        self.sample_rate.is_some() || self.volume.is_some() || self.seed.is_some()
    }
}

#[derive(Args)]
struct SfxArgs {
    #[arg(value_enum, help = "音效场景；kito sound presets 查看说明")]
    preset: Effect,
    /// 音效总时长（0.05–10 秒）；省略时使用场景默认值。
    #[arg(long)]
    duration: Option<f64>,
    /// 升降音高，范围 -24–24 半音；不改变噪声型声部。
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    pitch: f64,
    #[command(flatten)]
    output: OutputOptions,
    #[command(flatten)]
    wave: WaveOptions,
}

#[derive(Args)]
struct ToneArgs {
    /// 音名或 Hz：A4、C#5、Bb3、440 或 440Hz。
    #[arg(default_value = "A4")]
    pitch: String,
    /// 时长，0.01–120 秒。
    #[arg(long, default_value_t = 1.0)]
    duration: f64,
    #[arg(long, value_enum, default_value = "sine", help = "合成音色")]
    instrument: Timbre,
    #[command(flatten)]
    output: OutputOptions,
    #[command(flatten)]
    wave: WaveOptions,
}

#[derive(Args)]
struct MelodyArgs {
    #[command(flatten)]
    input: InputArgs,
    /// 每分钟四分音符拍数，20–400。
    #[arg(long, default_value_t = 120)]
    bpm: u16,
    /// 完整乐谱重复次数，1–64；总时长不超过 120 秒。
    #[arg(long, default_value_t = 1)]
    repeat: u16,
    /// WAV 合成音色；默认 soft；MIDI 不接受此参数。
    #[arg(long, value_enum)]
    instrument: Option<Timbre>,
    #[command(flatten)]
    output: OutputOptions,
    #[command(flatten)]
    wave: WaveOptions,
}

#[derive(Args)]
struct BeatArgs {
    #[arg(value_enum, default_value = "basic", help = "鼓型预设")]
    preset: BeatPreset,
    #[arg(long, default_value_t = 120, help = "每分钟拍数，20–400")]
    bpm: u16,
    #[arg(
        long,
        default_value_t = 4,
        help = "4/4 小节数，1–64；总时长不超过 120 秒"
    )]
    bars: u16,
    #[arg(long, allow_hyphen_values = true, help = "底鼓模式，16 格 x/X/-/.")]
    kick: Option<String>,
    #[arg(long, allow_hyphen_values = true, help = "军鼓模式，16 格 x/X/-/.")]
    snare: Option<String>,
    #[arg(long, allow_hyphen_values = true, help = "踩镲模式，16 格 x/X/-/.")]
    hat: Option<String>,
    #[command(flatten)]
    output: OutputOptions,
    #[command(flatten)]
    wave: WaveOptions,
}

#[derive(Serialize)]
struct Report {
    schema_version: u8,
    partner: &'static str,
    operation: &'static str,
    format: &'static str,
    output: PathBuf,
    dry_run: bool,
    duration_seconds: f64,
    event_count: usize,
    sample_rate: Option<u32>,
    channels: Option<u8>,
    frames: Option<usize>,
    instrument: Option<Timbre>,
    seed: Option<u64>,
    volume: Option<f64>,
    peak: Option<f64>,
    normalization_gain: Option<f64>,
    parameters: Value,
}

/// 路径预检不创建目录，保证 dry-run 与失败输入没有文件副作用。
fn output_path(
    options: &OutputOptions,
    default: &str,
    input: &InputArgs,
    allow_midi: bool,
) -> Result<(PathBuf, bool)> {
    let path = options
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(default));
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let midi = matches!(extension.as_str(), "mid" | "midi");
    ensure!(
        extension == "wav" || (allow_midi && midi),
        "输出必须使用 .wav{} 扩展名",
        if allow_midi { " 或 .mid/.midi" } else { "" }
    );
    if let Some(source) = input.file.as_deref().filter(|p| *p != Path::new("-")) {
        ensure!(
            !crate::files::same(source, &path),
            "输出与输入指向同一文件，不能覆盖乐谱输入"
        );
    }
    match fs::symlink_metadata(&path) {
        Ok(meta) => {
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "输出必须是普通文件，不能是符号链接"
            );
            ensure!(
                options.overwrite,
                "输出已存在：{}；使用新文件名或 --overwrite",
                path.display()
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("无法检查输出路径"),
    }
    // 最近存在的祖先必须可作为目录；完整权限由真实写入时的系统错误报告。
    for parent in path
        .ancestors()
        .skip(1)
        .filter(|p| !p.as_os_str().is_empty())
    {
        match fs::symlink_metadata(parent) {
            Ok(meta) => {
                // symlink_metadata 可区分尚未创建的目录和已经存在的悬空链接。
                // 后者不能通过 create_dir_all 修复，dry-run 也必须明确拒绝。
                let target = if meta.file_type().is_symlink() {
                    fs::metadata(parent).with_context(|| {
                        format!("输出父路径的符号链接无效：{}", parent.display())
                    })?
                } else {
                    meta
                };
                ensure!(
                    target.is_dir(),
                    "输出的父路径不是目录：{}",
                    parent.display()
                );
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("无法检查输出父路径"),
        }
    }
    Ok((path, midi))
}

struct Creation {
    operation: &'static str,
    default_output: String,
    voices: Vec<Voice>,
    duration: f64,
    instrument: Option<Timbre>,
    parameters: Value,
}

fn write_report(report: &Report, json_output: bool) -> Result<()> {
    if json_output {
        let mut bytes = serde_json::to_vec(report)?;
        bytes.push(b'\n');
        return write_stdout(&bytes);
    }
    let mut message = format!(
        "{} {}\n格式：{}，时长：{:.3} 秒，声音事件：{}\n",
        if report.dry_run {
            "计划生成："
        } else {
            "已生成："
        },
        report.output.display(),
        report.format.to_uppercase(),
        report.duration_seconds,
        report.event_count
    );
    if let Some(rate) = report.sample_rate {
        message.push_str(&format!(
            "PCM 16-bit，单声道，{rate} Hz；音量 {:.2}\n",
            report.volume.unwrap_or(0.7)
        ));
    } else {
        message.push_str("MIDI 保存音符与节奏；请在编曲软件中选择音色。\n");
    }
    if report.dry_run {
        message.push_str("校验通过；未合成或创建文件。\n");
    }
    if let Some(gain) = report.normalization_gain.filter(|g| *g < 1.0) {
        message.push_str(&format!(
            "混音峰值保护：整体增益调整为 {gain:.4}，避免削波。\n"
        ));
    }
    write_stdout(message.as_bytes())
}

fn create_wave(
    creation: Creation,
    wave: WaveOptions,
    output: OutputOptions,
    input: InputArgs,
) -> Result<bool> {
    let (path, _) = output_path(&output, &creation.default_output, &input, false)?;
    let rate = wave.rate();
    let frames = synth::validate(&creation.voices, creation.duration, rate, wave.volume())?;
    let mut report = Report {
        schema_version: 1,
        partner: "Echo",
        operation: creation.operation,
        format: "wav",
        output: path.clone(),
        dry_run: output.dry_run,
        duration_seconds: frames as f64 / f64::from(rate),
        event_count: creation.voices.len(),
        sample_rate: Some(rate),
        channels: Some(1),
        frames: Some(frames),
        instrument: creation.instrument,
        seed: Some(wave.seed()),
        volume: Some(wave.volume()),
        peak: None,
        normalization_gain: None,
        parameters: creation.parameters,
    };
    if !output.dry_run {
        let rendered = synth::render(
            &creation.voices,
            creation.duration,
            rate,
            wave.volume(),
            wave.seed(),
        )?;
        report.peak = Some(rendered.peak);
        report.normalization_gain = Some(rendered.normalization_gain);
        OutputArgs {
            output: Some(path),
            overwrite: output.overwrite,
        }
        .write(&input, &rendered.wav, false)?;
    }
    write_report(&report, output.json)?;
    Ok(true)
}

fn melody(args: MelodyArgs) -> Result<bool> {
    let (path, midi) = output_path(&args.output, "melody.wav", &args.input, true)?;
    ensure!(
        !midi || (!args.wave.explicit() && args.instrument.is_none()),
        "MIDI 只保存音符和节奏，不接受 --instrument、--sample-rate、--volume 或 --seed；需要音色请输出 WAV"
    );
    let text = args.input.read_utf8(64 * 1024)?;
    let score = score::parse_score(&text, args.bpm, args.repeat)?;
    let parameters = json!({"bpm": args.bpm, "repeat": args.repeat, "total_ticks": score.total_ticks, "ticks_per_beat": score::TICKS_PER_BEAT});
    if midi {
        // 编码也会验证直接构造的 Score；dry-run 不分配输出文件内容。
        let report = Report {
            schema_version: 1,
            partner: "Echo",
            operation: "melody",
            format: "midi",
            output: path.clone(),
            dry_run: args.output.dry_run,
            duration_seconds: score.duration_seconds(),
            event_count: score.events.len(),
            sample_rate: None,
            channels: None,
            frames: None,
            instrument: None,
            seed: None,
            volume: None,
            peak: None,
            normalization_gain: None,
            parameters,
        };
        if !args.output.dry_run {
            let bytes = score::encode_midi(&score)?;
            OutputArgs {
                output: Some(path),
                overwrite: args.output.overwrite,
            }
            .write(&args.input, &bytes, false)?;
        }
        write_report(&report, args.output.json)?;
        return Ok(true);
    }
    let timbre = args.instrument.unwrap_or(Timbre::Soft);
    let seconds_per_tick = 60.0 / f64::from(score.bpm) / f64::from(score::TICKS_PER_BEAT);
    let voices = score
        .events
        .iter()
        .map(|event| {
            let hz = score::frequency(event.midi);
            Voice {
                start: f64::from(event.start_tick) * seconds_per_tick,
                duration: f64::from(event.duration_ticks) * seconds_per_tick,
                frequency: hz,
                end_frequency: hz,
                gain: 0.8,
                instrument: timbre.instrument(),
            }
        })
        .collect();
    create_wave(
        Creation {
            operation: "melody",
            default_output: "melody.wav".into(),
            voices,
            duration: score.duration_seconds(),
            instrument: Some(timbre),
            parameters,
        },
        args.wave,
        args.output,
        args.input,
    )
}

pub fn run(args: SoundArgs) -> Result<bool> {
    match args.command {
        SoundCommand::Presets { json: json_output } => {
            let catalog = json!({"partner": "Echo", "effects": [
                {"name":"notify","description":"通知提示","duration_seconds":0.45},
                {"name":"success","description":"成功提示","duration_seconds":0.9},
                {"name":"error","description":"错误提示","duration_seconds":0.65},
                {"name":"coin","description":"收集金币","duration_seconds":0.4},
                {"name":"jump","description":"跳跃","duration_seconds":0.45},
                {"name":"laser","description":"激光","duration_seconds":0.35},
                {"name":"explosion","description":"爆炸","duration_seconds":1.2},
                {"name":"whoosh","description":"掠过转场","duration_seconds":0.8}],
                "instruments":[{"name":"sine","description":"干净正弦音"},{"name":"soft","description":"柔和合成音"},{"name":"bell","description":"钟铃合成音"},{"name":"pluck","description":"拨弦合成音"},{"name":"chip","description":"复古电子音"}],
                "beats":[{"name":"basic","description":"基础四拍"},{"name":"house","description":"四拍与反拍踩镲"},{"name":"hiphop","description":"切分底鼓"}]});
            if json_output {
                let mut bytes = serde_json::to_vec(&catalog)?;
                bytes.push(b'\n');
                write_stdout(&bytes)?;
            } else {
                let mut text = String::from("Echo 艾可 · 声音伙伴（音效师）\n\n");
                for (section, label) in [
                    ("effects", "音效（sound sfx <名称>）"),
                    ("instruments", "音色（--instrument <名称>）"),
                    ("beats", "鼓型（sound beat <名称>）"),
                ] {
                    text.push_str(&format!("{label}\n"));
                    if let Some(entries) = catalog[section].as_array() {
                        for entry in entries {
                            text.push_str(&format!(
                                "  {:<12} {}\n",
                                entry["name"].as_str().unwrap_or(""),
                                entry["description"].as_str().unwrap_or("")
                            ));
                        }
                    }
                    text.push('\n');
                }
                text.push_str("入门：kito sound sfx success\n旋律：kito sound melody \"C4 E4 G4:2\"\n完整教程：docs/tools/sound.md\n");
                write_stdout(text.as_bytes())?;
            }
            Ok(true)
        }
        SoundCommand::Sfx(args) => {
            let duration = args.duration.unwrap_or_else(|| args.preset.duration());
            let voices = pattern::effect(args.preset, duration, args.pitch)?;
            create_wave(
                Creation {
                    operation: "sfx",
                    default_output: format!("{}.wav", args.preset.name()),
                    voices,
                    duration,
                    instrument: None,
                    parameters: json!({"preset":args.preset,"pitch":args.pitch,"requested_duration_seconds":duration}),
                },
                args.wave,
                args.output,
                InputArgs::default(),
            )
        }
        SoundCommand::Tone(args) => {
            ensure!(
                args.duration.is_finite() && (0.01..=120.0).contains(&args.duration),
                "单音时长必须在 0.01–120 秒之间"
            );
            let numeric = args
                .pitch
                .strip_suffix("Hz")
                .or_else(|| args.pitch.strip_suffix("hz"))
                .unwrap_or(&args.pitch);
            let hz = match numeric.parse::<f64>() {
                Ok(hz) => hz,
                Err(_) => score::frequency(
                    score::parse_note(&args.pitch)
                        .context("音高应为音名（如 A4、Bb3）或 Hz（如 440）")?,
                ),
            };
            let voice = Voice {
                start: 0.0,
                duration: args.duration,
                frequency: hz,
                end_frequency: hz,
                gain: 0.8,
                instrument: args.instrument.instrument(),
            };
            create_wave(
                Creation {
                    operation: "tone",
                    default_output: "tone.wav".into(),
                    voices: vec![voice],
                    duration: args.duration,
                    instrument: Some(args.instrument),
                    parameters: json!({"pitch":args.pitch,"frequency_hz":hz,"requested_duration_seconds":args.duration}),
                },
                args.wave,
                args.output,
                InputArgs::default(),
            )
        }
        SoundCommand::Melody(args) => melody(args),
        SoundCommand::Beat(args) => {
            let defaults = args.preset.patterns();
            let patterns = [
                args.kick.as_deref().unwrap_or(defaults[0]),
                args.snare.as_deref().unwrap_or(defaults[1]),
                args.hat.as_deref().unwrap_or(defaults[2]),
            ];
            let (voices, duration) = pattern::beat(patterns, args.bpm, args.bars)?;
            let parameters = json!({"preset":args.preset,"bpm":args.bpm,"bars":args.bars,"kick":patterns[0],"snare":patterns[1],"hat":patterns[2]});
            create_wave(
                Creation {
                    operation: "beat",
                    default_output: "beat.wav".into(),
                    voices,
                    duration,
                    instrument: None,
                    parameters,
                },
                args.wave,
                args.output,
                InputArgs::default(),
            )
        }
    }
}
