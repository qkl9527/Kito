//! Vido 维多：先构建可检查的 FFmpeg 参数，再验证候选并原子发布。
//! 媒体文件始终在磁盘上流式处理；stdout 只承载小型探测和结果报告。
mod audio;
pub mod cli;
mod filters;
pub mod probe;

use anyhow::{Context, Result, bail, ensure};
use cli::*;
use probe::MediaInfo;
use serde_json::json;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub fn run(command: VideoCommand) -> Result<bool> {
    match command {
        VideoCommand::Doctor { json } => {
            let report = probe::doctor();
            emit(&report, json)?;
            Ok(report["healthy"] == true)
        }
        VideoCommand::Info { input, json } => {
            let info = probe::probe(&input, Duration::from_secs(30))?;
            if json {
                emit(&info, true)?;
            } else {
                println!(
                    "文件：{}\n容器：{}\n大小：{} bytes",
                    info.path.display(),
                    info.format.as_deref().unwrap_or("未知"),
                    info.size_bytes
                );
                if let Some(d) = info.duration {
                    println!("时长：{d:.3} 秒");
                }
                for stream in &info.streams {
                    println!(
                        "轨道 #{}：{} / {}",
                        stream.index,
                        stream.codec_type,
                        stream.codec.as_deref().unwrap_or("未知")
                    );
                    if stream.codec_type == "video" && !stream.attached_pic {
                        let (w, h) = stream.display_size()?;
                        println!(
                            "  显示尺寸：{w}×{h}；帧率：{}",
                            stream
                                .fps
                                .map(|v| format!("{v:.3}"))
                                .unwrap_or("未知".into())
                        );
                    }
                    if let Some(rate) = stream.sample_rate {
                        println!(
                            "  采样率：{rate} Hz；声道：{}",
                            stream
                                .channels
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| "未知".into())
                        );
                    }
                }
            }
            Ok(true)
        }
        VideoCommand::Convert(a) => video(
            &a.input,
            &a.output,
            &a.encoding,
            "convert",
            Some(a.to),
            Effect::Plain,
        ),
        VideoCommand::Compress(a) => video(
            &a.input,
            &a.output,
            &a.encoding,
            "compress",
            None,
            Effect::Plain,
        ),
        VideoCommand::Mute(a) => {
            video(&a.input, &a.output, &a.encoding, "mute", None, Effect::Mute)
        }
        VideoCommand::Trim(a) => video(
            &a.input,
            &a.output,
            &a.encoding,
            "trim",
            None,
            Effect::Trim {
                start: a.start,
                end: a.end,
                duration: a.duration,
            },
        ),
        VideoCommand::Resize(a) => video(
            &a.input,
            &a.output,
            &a.encoding,
            "resize",
            None,
            Effect::Resize(a.size),
        ),
        VideoCommand::Crop(a) => video(
            &a.input,
            &a.output,
            &a.encoding,
            "crop",
            None,
            Effect::Crop(a.width, a.height, a.x, a.y),
        ),
        VideoCommand::Rotate(a) => video(
            &a.input,
            &a.output,
            &a.encoding,
            "rotate",
            None,
            Effect::Rotate(a.angle),
        ),
        VideoCommand::Speed(a) => video(
            &a.input,
            &a.output,
            &a.encoding,
            "speed",
            None,
            Effect::Speed(a.factor),
        ),
        VideoCommand::Concat(a) => concat(a),
        VideoCommand::Snapshot(a) => snapshot(a),
        VideoCommand::Gif(a) => gif(a),
        VideoCommand::Audio(a) => audio::run(a.command),
    }
}

pub(super) fn emit(value: &impl serde::Serialize, compact: bool) -> Result<()> {
    if compact {
        println!("{}", serde_json::to_string(value)?);
    } else {
        println!("{}", serde_json::to_string_pretty(value)?);
    }
    Ok(())
}

/// args 不包含最终输出位置；dry-run 展示目标，执行时替换为同目录随机候选。
pub(super) struct Plan {
    pub operation: &'static str,
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub args: Vec<OsString>,
    pub expect_video: bool,
    pub expect_audio: bool,
    pub lossy: bool,
    pub notes: Vec<String>,
}

pub(super) fn base_args() -> Vec<OsString> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-y",
        "-xerror",
        // FFmpeg 可对只有 WAV 文件头、没有任何采样的数据返回成功。
        // 编码和复核均要求每条输出流确实有数据，不能将零帧结果发布为成功。
        "-abort_on",
        "empty_output+empty_output_stream",
    ]
    .map(Into::into)
    .to_vec()
}
pub(super) fn push(args: &mut Vec<OsString>, values: &[&str]) {
    args.extend(values.iter().map(OsString::from));
}
pub(super) fn input_args(args: &mut Vec<OsString>, path: &Path) {
    // 每次输入都绑定同一组限制；探测、编码和输出复核不能各自放宽间接资源访问。
    push(
        args,
        &[
            "-protocol_whitelist",
            probe::INPUT_PROTOCOL_WHITELIST,
            "-format_whitelist",
            probe::INPUT_FORMAT_WHITELIST,
            "-i",
        ],
    );
    args.push(path.as_os_str().into());
}
pub(super) fn timeout(opts: &OutputArgs) -> Duration {
    Duration::from_secs(opts.time_limit)
}

pub(super) fn target(
    input: &Path,
    opts: &OutputArgs,
    op: &str,
    extension: &str,
) -> Result<PathBuf> {
    let path = opts.output.clone().unwrap_or_else(|| {
        let mut name = input.file_stem().unwrap_or_default().to_os_string();
        name.push(format!(".{op}.{extension}"));
        input.with_file_name(name)
    });
    ensure!(
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(extension)),
        "输出扩展名必须是 .{extension}，请修改 -o 或目标格式"
    );
    validate_outputs(&[input.to_path_buf()], &path, opts.overwrite)?;
    Ok(path)
}
fn validate_outputs(inputs: &[PathBuf], output: &Path, overwrite: bool) -> Result<()> {
    for input in inputs {
        ensure!(
            !crate::files::same(input, output),
            "输出不能覆盖输入文件：{}",
            input.display()
        );
        crate::files::validate_destination(input, output, false, overwrite)?;
    }
    Ok(())
}
fn video_format(opts: &OutputArgs, explicit: Option<VideoFormat>) -> Result<VideoFormat> {
    if let Some(format) = explicit {
        return Ok(format);
    }
    match opts
        .output
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None => Ok(VideoFormat::Mp4),
        Some("mp4") => Ok(VideoFormat::Mp4),
        Some("mkv") => Ok(VideoFormat::Mkv),
        Some("mov") => Ok(VideoFormat::Mov),
        Some("webm") => Ok(VideoFormat::Webm),
        _ => bail!("视频输出仅支持 .mp4、.mkv、.mov、.webm"),
    }
}

pub(super) fn execute(plan: Plan, opts: &OutputArgs) -> Result<bool> {
    validate_outputs(&plan.inputs, &plan.output, opts.overwrite)?;
    if opts.dry_run {
        let mut args = plan.args.clone();
        args.push(plan.output.as_os_str().into());
        let display_args: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
        emit(
            &json!({"operation":plan.operation,"dry_run":true,"program":crate::backend::program("KITO_FFMPEG","ffmpeg").to_string_lossy(),"args":display_args,"output":plan.output,"notes":plan.notes,"lossy":plan.lossy}),
            opts.json,
        )?;
        return Ok(true);
    }
    let parent = plan
        .output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let extension = plan
        .output
        .extension()
        .and_then(|e| e.to_str())
        .context("输出缺少扩展名")?;
    let candidate = tempfile::Builder::new()
        .prefix(".kito-video-")
        .suffix(&format!(".{extension}"))
        .tempfile_in(parent)?;
    // 相对目录转绝对候选路径，避免以 '-' 开头的目录被后端当作选项。
    let candidate_path = fs::canonicalize(candidate.path())?;
    let mut args = plan.args;
    args.push(candidate_path.as_os_str().into());
    if !opts.json {
        eprintln!(
            "Vido 正在处理；完成编码与解码检查后输出：{}",
            plan.output.display()
        );
    }
    probe::ffmpeg(&args, timeout(opts))?;
    ensure!(
        candidate.as_file().metadata()?.len() > 0,
        "后端生成了空文件"
    );
    // snapshot 固定编码 PNG。Windows 的 \\?\ 前缀（以及文件名中的 ?、%03d）
    // 会让 FFmpeg 在读取内容前把它当成 image2 序列；显式用单文件 png_pipe，
    // 同时供探测和完整解码使用。不能放开 image2，也不能按用户提供的扩展名信任格式。
    let output_demuxer = (plan.operation == "snapshot").then_some("png_pipe");
    let mut info = probe::probe_with_demuxer(&candidate_path, timeout(opts), output_demuxer)
        .context("输出媒体探测失败，未发布结果")?;
    ensure!(
        info.video().is_some() == plan.expect_video,
        "输出视频轨道与预期不符，未发布结果"
    );
    ensure!(
        info.audio().is_some() == plan.expect_audio,
        "输出音频轨道与预期不符，未发布结果"
    );
    // ffprobe 只读取结构，不足以证明完整文件可播放；解码所有输出音视频帧并使解码错误返回失败。
    let mut check = base_args();
    if let Some(demuxer) = output_demuxer {
        push(&mut check, &["-f", demuxer]);
    }
    input_args(&mut check, &candidate_path);
    push(
        &mut check,
        &["-map", "0:v?", "-map", "0:a?", "-f", "null", "-"],
    );
    probe::ffmpeg(&check, timeout(opts)).context("输出完整解码检查失败，未发布结果")?;
    validate_outputs(&plan.inputs, &plan.output, opts.overwrite)?;
    crate::files::publish_staged(candidate, &plan.output, opts.overwrite)?;
    info.path = plan.output.clone();
    let input_bytes: u64 = plan
        .inputs
        .iter()
        .filter_map(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    let report = json!({"operation":plan.operation,"inputs":plan.inputs,"output":plan.output,"input_bytes":input_bytes,"output_bytes":info.size_bytes,"output_info":info,"lossy":plan.lossy,"verified_decodable":true,"notes":plan.notes});
    if opts.json {
        emit(&report, true)?;
    } else {
        println!(
            "完成：{}\n大小：{} bytes；已通过完整解码检查。",
            plan.output.display(),
            report["output_bytes"]
        );
        for note in plan.notes {
            println!("说明：{note}");
        }
    }
    Ok(true)
}

pub(super) enum Effect {
    Plain,
    Mute,
    Trim {
        start: String,
        end: Option<String>,
        duration: Option<String>,
    },
    Resize(String),
    Crop(u32, u32, u32, u32),
    Rotate(u16),
    Speed(f64),
    Volume(f64),
    ReplaceAudio(PathBuf),
}
pub(super) fn video(
    input: &Path,
    opts: &OutputArgs,
    encoding: &EncodingArgs,
    operation: &'static str,
    format: Option<VideoFormat>,
    effect: Effect,
) -> Result<bool> {
    let plan = video_plan(input, opts, encoding, operation, format, effect)?;
    execute(plan, opts)
}
fn video_plan(
    input: &Path,
    opts: &OutputArgs,
    encoding: &EncodingArgs,
    operation: &'static str,
    format: Option<VideoFormat>,
    effect: Effect,
) -> Result<Plan> {
    let format = video_format(opts, format)?;
    let output = target(input, opts, operation, format.extension())?;
    let info = probe::probe(input, timeout(opts))?;
    let stream = info
        .video()
        .context("输入没有可处理的视频轨道（封面图片不作为视频）")?;
    let displayed = stream.display_size()?;
    let mut dims = filters::even_size(displayed.0.into(), displayed.1.into())?;
    let mut vf = vec![
        format!("scale={}:{}:flags=lanczos", displayed.0, displayed.1),
        "setsar=1".into(),
        "setpts=PTS-STARTPTS".into(),
    ];
    let mut af = vec![aligned_audio(&info)];
    let mut sources = vec![info.path.clone()];
    let mut args = base_args();
    input_args(&mut args, &info.path);
    let mut audio_map = info.audio().map(|s| format!("0:{}", s.index));
    let mut limit = None;
    let mut notes = vec![
        "视频重新编码为有损格式；只保留首个视频轨和首音轨，不保留字幕、附件和源元数据。".into(),
    ];
    match effect {
        Effect::Plain => {}
        Effect::Mute => audio_map = None,
        Effect::Trim {
            start,
            end,
            duration,
        } => {
            let (start, length) = range(
                &info,
                &start,
                end.as_deref(),
                duration.as_deref(),
                timeout(opts),
            )?;
            vf.push(format!("trim=start={start}:duration={length}"));
            vf.push("setpts=PTS-STARTPTS".into());
            af.push(format!("atrim=start={start}:duration={length}"));
            af.push("asetpts=PTS-STARTPTS".into());
            limit = Some(length);
        }
        Effect::Resize(spec) => dims = filters::size(&spec, displayed)?,
        Effect::Crop(w, h, x, y) => {
            ensure!(
                w % 2 == 0 && h % 2 == 0 && x % 2 == 0 && y % 2 == 0,
                "裁切宽高和坐标必须是偶数，以精确匹配 YUV420 色度网格"
            );
            ensure!(
                x.checked_add(w).is_some_and(|r| r <= displayed.0)
                    && y.checked_add(h).is_some_and(|r| r <= displayed.1),
                "裁切区域超出画面范围"
            );
            dims = filters::even_size(w.into(), h.into())?;
            vf.push(format!("crop={w}:{h}:{x}:{y}:exact=1"));
        }
        Effect::Rotate(angle) => match angle {
            90 => {
                vf.push("transpose=clock".into());
                dims = (dims.1, dims.0);
            }
            180 => {
                vf.push("hflip".into());
                vf.push("vflip".into());
            }
            270 => {
                vf.push("transpose=cclock".into());
                dims = (dims.1, dims.0);
            }
            _ => bail!("旋转角度必须是 90、180 或 270"),
        },
        Effect::Speed(factor) => {
            af.push(filters::tempo(factor)?);
            vf.push(format!("setpts=PTS/{factor}"));
            limit = Some(video_duration(&info, timeout(opts))? / factor);
        }
        Effect::Volume(factor) => {
            validate_volume(factor)?;
            ensure!(audio_map.is_some(), "输入没有音轨可调整音量");
            af.push(format!("volume={factor}"));
        }
        Effect::ReplaceAudio(path) => {
            validate_outputs(std::slice::from_ref(&path), &output, opts.overwrite)?;
            let audio = probe::probe(&path, timeout(opts))?;
            let stream = audio.audio().context("配乐文件没有音频轨道")?;
            input_args(&mut args, &audio.path);
            audio_map = Some(format!("1:{}", stream.index));
            // 新配乐从画面起点开始，不继承原视频音轨的起播偏移。
            af = vec!["asetpts=PTS-STARTPTS".into()];
            sources.push(audio.path);
            limit = Some(video_duration(&info, timeout(opts))?);
        }
    }
    vf.push(format!("scale={}:{}:flags=lanczos", dims.0, dims.1));
    vf.push("setsar=1".into());
    push(
        &mut args,
        &["-map", &format!("0:{}", stream.index), "-vf", &vf.join(",")],
    );
    if let Some(map) = &audio_map {
        if let Some(length) = limit {
            af.push("apad".into());
            af.push(format!("atrim=duration={length}"));
        }
        push(&mut args, &["-map", map, "-af", &af.join(",")]);
    } else {
        push(&mut args, &["-an"]);
    }
    if let Some(length) = limit {
        push(&mut args, &["-t", &length.to_string()]);
    }
    video_encoding(&mut args, format, encoding, audio_map.is_some());
    notes.push(format!(
        "输出画面 {}×{}，显示比例按方形像素处理；奇数尺寸向下对齐偶数。",
        dims.0, dims.1
    ));
    Ok(Plan {
        operation,
        inputs: sources,
        output,
        args,
        expect_video: true,
        expect_audio: audio_map.is_some(),
        lossy: true,
        notes,
    })
}

/// 以视频画面起点为零，保留流间延迟；first_pts=0 为晚起音轨补静音，
/// 并裁掉早于画面起点的音频。async=1 按时间戳填补音频内部空洞。
fn aligned_audio(info: &MediaInfo) -> String {
    let offset = info
        .video()
        .and_then(|v| v.start_time)
        .zip(info.audio().and_then(|a| a.start_time))
        .map(|(video, audio)| audio - video)
        .unwrap_or(0.0);
    format!("asetpts=PTS-STARTPTS+({offset})/TB,aresample=async=1:first_pts=0")
}

pub(super) fn validate_volume(factor: f64) -> Result<()> {
    ensure!(
        factor.is_finite() && (0.0..=16.0).contains(&factor),
        "音量倍数必须在 0 至 16 之间"
    );
    Ok(())
}
fn video_duration(info: &MediaInfo, limit: Duration) -> Result<f64> {
    let video = info.video().context("输入没有视频轨道")?;
    // 容器 duration 可能包含绝对起点、其他更长轨道，不能当作本视频轨时长。
    probe::stream_duration(info, video, limit)
}
pub(super) fn range(
    info: &MediaInfo,
    start: &str,
    end: Option<&str>,
    duration: Option<&str>,
    limit: Duration,
) -> Result<(f64, f64)> {
    let total = video_duration(info, limit)?;
    let start = filters::timestamp(start)?;
    let length = match (end, duration) {
        (Some(end), None) => filters::timestamp(end)? - start,
        (None, Some(duration)) => filters::timestamp(duration)?,
        _ => bail!("--end 与 --duration 必须且只能提供一个"),
    };
    ensure!(length > 0.0, "片段时长必须大于 0，终点必须晚于起点");
    ensure!(
        start < total && start + length <= total + 0.000001,
        "片段范围超出媒体时长（{total:.6} 秒）"
    );
    Ok((start, length))
}
fn video_encoding(
    args: &mut Vec<OsString>,
    format: VideoFormat,
    encoding: &EncodingArgs,
    audio: bool,
) {
    let crf = match encoding.quality {
        Quality::High => 18,
        Quality::Balanced => 23,
        Quality::Small => 28,
    };
    if format == VideoFormat::Webm {
        let speed = match encoding.preset {
            Preset::Fast => 4,
            Preset::Medium => 2,
            Preset::Slow => 0,
        };
        push(
            args,
            &[
                "-c:v",
                "libvpx-vp9",
                "-crf",
                &(crf + 10).to_string(),
                "-b:v",
                "0",
                "-deadline",
                "good",
                "-cpu-used",
                &speed.to_string(),
            ],
        );
        if audio {
            push(args, &["-c:a", "libopus", "-b:a", "128k", "-ar", "48000"]);
        }
    } else {
        let preset = match encoding.preset {
            Preset::Fast => "fast",
            Preset::Medium => "medium",
            Preset::Slow => "slow",
        };
        push(
            args,
            &[
                "-c:v",
                "libx264",
                "-crf",
                &crf.to_string(),
                "-preset",
                preset,
            ],
        );
        if audio {
            push(args, &["-c:a", "aac", "-b:a", "192k"]);
        }
        if format != VideoFormat::Mkv {
            push(args, &["-movflags", "+faststart"]);
        }
    }
    push(
        args,
        &[
            "-pix_fmt",
            "yuv420p",
            "-map_metadata",
            "-1",
            "-map_chapters",
            "-1",
            "-metadata:s:v:0",
            "rotate=0",
        ],
    );
}

fn concat(a: ConcatArgs) -> Result<bool> {
    ensure!(
        (2..=64).contains(&a.inputs.len()),
        "拼接需要 2 至 64 个视频"
    );
    ensure!((1..=120).contains(&a.fps), "拼接帧率必须在 1 至 120 之间");
    let format = video_format(&a.output, None)?;
    let output = target(&a.inputs[0], &a.output, "concat", format.extension())?;
    validate_outputs(&a.inputs, &output, a.output.overwrite)?;
    let infos = a
        .inputs
        .iter()
        .map(|p| probe::probe(p, timeout(&a.output)))
        .collect::<Result<Vec<_>>>()?;
    let first = infos[0]
        .video()
        .context("首段没有视频轨道")?
        .display_size()?;
    let dims = if let Some(size) = a.size {
        let (w, h) = size
            .split_once('x')
            .context("拼接画布须使用 WIDTHxHEIGHT，例如 1280x720")?;
        let dims = filters::even_size(w.parse()?, h.parse()?)?;
        ensure!(
            dims.0.to_string() == w && dims.1.to_string() == h,
            "拼接画布宽高必须是偶数"
        );
        dims
    } else {
        filters::even_size(first.0.into(), first.1.into())?
    };
    let mut args = base_args();
    for info in &infos {
        input_args(&mut args, &info.path);
    }
    let mut graph = Vec::new();
    let mut labels = String::new();
    let mut duration = 0.0;
    for (i, info) in infos.iter().enumerate() {
        let v = info.video().context("每个拼接输入都必须有视频轨道")?;
        let displayed = v.display_size()?;
        let length = video_duration(info, timeout(&a.output))?;
        duration += length;
        // 各片段先归零时间戳，再统一尺寸、SAR、帧率和采样格式，允许来源编码不同。
        graph.push(format!("[{i}:{}]setpts=PTS-STARTPTS,scale={}:{},setsar=1,scale={}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2,pad={}:{}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,fps={},format=yuv420p,trim=duration={length},settb=AVTB[v{i}]",v.index,displayed.0,displayed.1,dims.0,dims.1,dims.0,dims.1,a.fps));
        if let Some(audio) = info.audio() {
            graph.push(format!("[{i}:{}]{},aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo,apad,atrim=duration={length}[a{i}]",audio.index,aligned_audio(info)));
        } else {
            graph.push(format!(
                "anullsrc=r=48000:cl=stereo,atrim=duration={length}[a{i}]"
            ));
        }
        labels.push_str(&format!("[v{i}][a{i}]"));
    }
    graph.push(format!("{labels}concat=n={}:v=1:a=1[v][a]", infos.len()));
    push(
        &mut args,
        &[
            "-filter_complex",
            &graph.join(";"),
            "-map",
            "[v]",
            "-map",
            "[a]",
            "-t",
            &duration.to_string(),
        ],
    );
    video_encoding(&mut args, format, &a.encoding, true);
    execute(
        Plan {
            operation: "concat",
            inputs: infos.into_iter().map(|i| i.path).collect(),
            output,
            args,
            expect_video: true,
            expect_audio: true,
            lossy: true,
            notes: vec![
                format!(
                    "统一为 {}×{} / {} fps；等比黑边补齐；音频为 48 kHz 立体声，无音轨片段补静音。",
                    dims.0, dims.1, a.fps
                ),
                "全部片段重新编码；字幕、附件及源元数据不保留。".into(),
            ],
        },
        &a.output,
    )
}

fn snapshot(a: SnapshotArgs) -> Result<bool> {
    let output = target(&a.input, &a.output, "snapshot", "png")?;
    let info = probe::probe(&a.input, timeout(&a.output))?;
    let video = info.video().context("输入没有视频轨道")?;
    let at = filters::timestamp(&a.at)?;
    ensure!(
        at < video_duration(&info, timeout(&a.output))?,
        "截图时刻必须早于视频结束"
    );
    let (w, h) = video.display_size()?;
    let mut args = base_args();
    input_args(&mut args, &info.path);
    push(
        &mut args,
        &[
            "-ss",
            &at.to_string(),
            "-map",
            &format!("0:{}", video.index),
            "-vf",
            &format!("scale={w}:{h},setsar=1"),
            "-frames:v",
            "1",
            "-an",
            "-c:v",
            "png",
            "-update",
            "1",
            "-map_metadata",
            "-1",
        ],
    );
    execute(Plan{operation:"snapshot",inputs:vec![info.path],output,args,expect_video:true,expect_audio:false,lossy:true,notes:vec!["PNG 保存解码后的画面；可能发生像素格式转换，不宣称与压缩视频或原始拍摄数据像素无损。".into()]},&a.output)
}
fn gif(a: GifArgs) -> Result<bool> {
    ensure!(
        (2..=4096).contains(&a.width),
        "GIF 宽度必须在 2 至 4096 之间"
    );
    ensure!((1..=50).contains(&a.fps), "GIF 帧率必须在 1 至 50 之间");
    let output = target(&a.input, &a.output, "gif", "gif")?;
    let info = probe::probe(&a.input, timeout(&a.output))?;
    let video = info.video().context("输入没有视频轨道")?;
    let (start, length) = range(&info, &a.start, None, Some(&a.duration), timeout(&a.output))?;
    ensure!(
        length <= 60.0,
        "GIF 片段最多 60 秒，请先裁剪或缩短 --duration"
    );
    let (w, h) = video.display_size()?;
    let mut args = base_args();
    input_args(&mut args, &info.path);
    let graph = format!(
        "[0:{}]setpts=PTS-STARTPTS,trim=start={start}:duration={length},setpts=PTS-STARTPTS,scale={w}:{h},setsar=1,fps={},scale={}:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse=dither=sierra2_4a[v]",
        video.index, a.fps, a.width
    );
    push(
        &mut args,
        &[
            "-filter_complex",
            &graph,
            "-map",
            "[v]",
            "-an",
            "-loop",
            "0",
            "-map_metadata",
            "-1",
        ],
    );
    execute(
        Plan {
            operation: "gif",
            inputs: vec![info.path],
            output,
            args,
            expect_video: true,
            expect_audio: false,
            lossy: true,
            notes: vec![
                "GIF 限制为调色板颜色，不含音频，无限循环；缩放、帧率和颜色量化均会改变画面。"
                    .into(),
            ],
        },
        &a.output,
    )
}
