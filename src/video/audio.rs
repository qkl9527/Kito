//! 音频处理沿用视频模块的候选验证与原子发布；所有路径都保持为独立 argv。
use super::{
    Effect, Plan, base_args, cli::*, execute, filters, input_args, probe, probe::MediaInfo, push,
    target, timeout, validate_volume, video,
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};

pub fn run(command: AudioCommand) -> Result<bool> {
    match command {
        AudioCommand::Extract(a) => single(&a.input, &a.output, a.to, "audio", AudioEffect::Plain),
        AudioCommand::Convert(a) => single(
            &a.input,
            &a.output,
            a.to,
            "audio-convert",
            AudioEffect::Plain,
        ),
        AudioCommand::Replace(a) => video(
            &a.input,
            &a.output,
            &a.encoding,
            "replace-audio",
            None,
            Effect::ReplaceAudio(a.audio),
        ),
        AudioCommand::Volume(a) => {
            validate_volume(a.factor)?;
            let info = probe::probe(&a.input, timeout(&a.output))?;
            // 未指定音频目标格式时，有视频轨的输入保持视频输出；封面图片不算视频轨。
            if a.to.is_none() && info.video().is_some() {
                video(
                    &a.input,
                    &a.output,
                    &a.encoding,
                    "volume",
                    None,
                    Effect::Volume(a.factor),
                )
            } else {
                single(
                    &a.input,
                    &a.output,
                    a.to.unwrap_or(AudioFormat::Mp3),
                    "volume",
                    AudioEffect::Volume(a.factor),
                )
            }
        }
        AudioCommand::Trim(a) => single(
            &a.input,
            &a.output,
            a.to,
            "audio-trim",
            AudioEffect::Trim {
                start: a.start,
                end: a.end,
                duration: a.duration,
            },
        ),
        AudioCommand::Concat(a) => combine(&a.inputs, &a.output, a.to, false),
        AudioCommand::Mix(a) => combine(&a.inputs, &a.output, a.to, true),
        AudioCommand::Speed(a) => single(
            &a.input,
            &a.output,
            a.to,
            "audio-speed",
            AudioEffect::Speed(a.factor),
        ),
        AudioCommand::Fade(a) => single(
            &a.input,
            &a.output,
            a.to,
            "audio-fade",
            AudioEffect::Fade {
                fade_in: a.fade_in,
                fade_out: a.fade_out,
            },
        ),
        AudioCommand::Normalize(a) => single(
            &a.input,
            &a.output,
            a.to,
            "audio-normalize",
            AudioEffect::Normalize(a.loudness),
        ),
    }
}

enum AudioEffect {
    Plain,
    Volume(f64),
    Trim {
        start: String,
        end: Option<String>,
        duration: Option<String>,
    },
    Speed(f64),
    Fade {
        fade_in: String,
        fade_out: String,
    },
    Normalize(f64),
}

/// 音频操作以首音轨自己的时长为准，不能被容器内更长的视频轨或封面误导。
fn duration(info: &MediaInfo, time_limit: Duration) -> Result<f64> {
    let stream = info.audio().context("输入没有可处理的音频轨道")?;
    probe::stream_duration(info, stream, time_limit)
}

fn trim_range(
    info: &MediaInfo,
    start: &str,
    end: Option<&str>,
    length: Option<&str>,
    time_limit: Duration,
) -> Result<(f64, f64)> {
    let total = duration(info, time_limit)?;
    let start = filters::timestamp(start)?;
    let length = match (end, length) {
        (Some(end), None) => filters::timestamp(end)? - start,
        (None, Some(length)) => filters::timestamp(length)?,
        _ => bail!("--end 与 --duration 必须且只能提供一个"),
    };
    ensure!(length > 0.0, "音频片段时长必须大于 0，终点必须晚于起点");
    ensure!(
        start < total && start + length <= total + 0.000_001,
        "片段范围超出首音轨时长（{total:.6} 秒）"
    );
    Ok((start, length))
}

fn single(
    input: &Path,
    opts: &OutputArgs,
    format: AudioFormat,
    operation: &'static str,
    effect: AudioEffect,
) -> Result<bool> {
    let output = target(input, opts, operation, format.extension())?;
    let info = probe::probe(input, timeout(opts))?;
    let stream = info.audio().context("输入没有可处理的音频轨道")?;
    let mut args = base_args();
    input_args(&mut args, &info.path);
    // 独立音频文件以该音轨自身的开始位置为 0；这也使 atrim/afade 的秒数语义一致。
    let mut graph = vec!["asetpts=PTS-STARTPTS".to_owned()];
    let mut notes = common_notes();
    match effect {
        AudioEffect::Plain => {}
        AudioEffect::Volume(factor) => {
            validate_volume(factor)?;
            graph.push(format!("volume={factor}"));
            notes.push("音量使用线性倍数；提高音量可能产生削波。".into());
        }
        AudioEffect::Trim {
            start,
            end,
            duration,
        } => {
            let (start, length) = trim_range(
                &info,
                &start,
                end.as_deref(),
                duration.as_deref(),
                timeout(opts),
            )?;
            graph.push(format!("atrim=start={start}:duration={length}"));
            graph.push("asetpts=PTS-STARTPTS".into());
        }
        AudioEffect::Speed(factor) => {
            graph.push(filters::tempo(factor)?);
            notes.push(format!(
                "以 {factor} 倍速重新采样时间，使用 atempo 保持音调。"
            ));
        }
        AudioEffect::Fade { fade_in, fade_out } => {
            let fade_in = filters::timestamp(&fade_in)?;
            let fade_out = filters::timestamp(&fade_out)?;
            let total = duration(&info, timeout(opts))?;
            ensure!(
                fade_in > 0.0 || fade_out > 0.0,
                "淡入和淡出至少有一项大于 0"
            );
            ensure!(
                fade_in <= total && fade_out <= total,
                "淡入或淡出时长不得超过首音轨时长（{total:.6} 秒）"
            );
            if fade_in > 0.0 {
                graph.push(format!("afade=t=in:st=0:d={fade_in}"));
            }
            if fade_out > 0.0 {
                graph.push(format!("afade=t=out:st={}:d={fade_out}", total - fade_out));
            }
            notes.push("淡入从首音轨开头计算，淡出在结尾结束；区间交叠时两种增益相乘。".into());
        }
        AudioEffect::Normalize(loudness) => {
            ensure!(
                loudness.is_finite() && (-70.0..=-5.0).contains(&loudness),
                "目标响度必须在 -70 至 -5 LUFS 之间"
            );
            graph.push(format!("loudnorm=I={loudness}:TP=-1.5:LRA=11:linear=false"));
            notes.push(format!("单遍动态 loudnorm：目标 {loudness} LUFS、真峰值 -1.5 dBTP、响度范围 11 LU；不保证精确达到目标。"));
        }
    }
    // loudnorm 可能内部升采样至 192 kHz，因此统一采样规格必须放在全部效果之后。
    graph.push("aresample=48000".into());
    graph.push("aformat=sample_fmts=fltp:channel_layouts=stereo".into());
    push(
        &mut args,
        &[
            "-map",
            &format!("0:{}", stream.index),
            "-af",
            &graph.join(","),
        ],
    );
    encoding(&mut args, format);
    execute(
        Plan {
            operation,
            inputs: vec![info.path],
            output,
            args,
            expect_video: false,
            expect_audio: true,
            lossy: true,
            notes,
        },
        opts,
    )
}

fn combine(inputs: &[PathBuf], opts: &OutputArgs, format: AudioFormat, mix: bool) -> Result<bool> {
    ensure!(
        (2..=64).contains(&inputs.len()),
        "音频拼接或混音需要 2 至 64 个输入"
    );
    let operation = if mix { "audio-mix" } else { "audio-concat" };
    let output = target(&inputs[0], opts, operation, format.extension())?;
    let infos = inputs
        .iter()
        .map(|input| probe::probe(input, timeout(opts)))
        .collect::<Result<Vec<_>>>()?;
    let mut args = base_args();
    let mut graph = Vec::with_capacity(infos.len() + 1);
    let mut labels = String::new();
    for (i, info) in infos.iter().enumerate() {
        let stream = info
            .audio()
            .with_context(|| format!("输入没有音频轨道：{}", info.path.display()))?;
        input_args(&mut args, &info.path);
        // concat 要求每段从零开始，并且采样率、采样格式、声道布局一致。
        // 标签只来自受控索引；文件名不会进入 filter 表达式。
        graph.push(format!("[{i}:{}]asetpts=PTS-STARTPTS,aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo[a{i}]", stream.index));
        labels.push_str(&format!("[a{i}]"));
    }
    if mix {
        graph.push(format!(
            "{labels}amix=inputs={}:duration=longest:normalize=1:dropout_transition=0[a]",
            infos.len()
        ));
    } else {
        graph.push(format!("{labels}concat=n={}:v=0:a=1[a]", infos.len()));
    }
    push(
        &mut args,
        &["-filter_complex", &graph.join(";"), "-map", "[a]"],
    );
    encoding(&mut args, format);
    let mut notes = common_notes();
    notes.push(if mix {
        "所有输入从开头同时混音，活动轨道等权归一化；较短轨道结束后其余轨道继续，输出取最长时长。"
            .into()
    } else {
        "各输入只取首音轨，按给定顺序拼接；每段先统一为 48 kHz 立体声。".into()
    });
    execute(
        Plan {
            operation,
            inputs: infos.into_iter().map(|info| info.path).collect(),
            output,
            args,
            expect_video: false,
            expect_audio: true,
            lossy: true,
            notes,
        },
        opts,
    )
}

fn encoding(args: &mut Vec<OsString>, format: AudioFormat) {
    match format {
        AudioFormat::Mp3 => push(args, &["-c:a", "libmp3lame", "-b:a", "192k"]),
        AudioFormat::M4a => push(
            args,
            &["-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"],
        ),
        AudioFormat::Wav => push(args, &["-c:a", "pcm_s16le", "-sample_fmt", "s16"]),
        AudioFormat::Flac => push(args, &["-c:a", "flac", "-sample_fmt", "s16"]),
    }
    push(
        args,
        &[
            "-ar",
            "48000",
            "-ac",
            "2",
            "-vn",
            "-sn",
            "-dn",
            "-map_metadata",
            "-1",
            "-map_metadata:s:a",
            "-1",
            "-map_chapters",
            "-1",
        ],
    );
}

fn common_notes() -> Vec<String> {
    vec![
        "仅输出首音轨；源元数据、章节、视频、封面和其他轨道不保留。".into(),
        "统一为 48 kHz 双声道；MP3/M4A 为 192 kbit/s 有损编码，WAV/FLAC 为 16-bit，可能发生重采样、声道转换和量化；不宣称与源音频无损，也不能恢复有损源已丢失的信息。".into(),
    ]
}
