//! FFmpeg/ffprobe 后端和本地媒体元数据。
//!
//! 这里只探测容器与流信息，不把元数据读取成功当作完整解码验证。

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

/// 只允许直接读取单个普通媒体文件的 demuxer。尤其不能让 HLS、DASH、concat、
/// image2 等从已选文件继续打开播放列表条目或文件序列。
/// 输出验证也会读取单张图片，因此保留对应的 pipe demuxer。
pub(super) const INPUT_FORMAT_WHITELIST: &str = "mov,mp4,m4a,3gp,3g2,mj2,matroska,webm,avi,mpeg,mpegts,mpegvideo,h264,hevc,flv,ogg,wav,mp3,flac,aiff,amr,amrnb,amrwb,caf,ac3,eac3,asf,png_pipe,jpeg_pipe,gif_pipe,gif,apng";
pub(super) const INPUT_PROTOCOL_WHITELIST: &str = "file";

#[derive(Clone, Debug, Serialize)]
pub struct MediaInfo {
    pub path: PathBuf,
    pub duration: Option<f64>,
    pub size_bytes: u64,
    pub format: Option<String>,
    pub streams: Vec<StreamInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StreamInfo {
    pub index: u32,
    pub codec_type: String,
    pub codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<f64>,
    /// 容器时间轴中的流起点，可能为负；用于保留视频与音频之间的相对延迟。
    pub start_time: Option<f64>,
    pub fps: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub attached_pic: bool,
    pub rotation: i32,
    pub sample_aspect_ratio: Option<String>,
}

impl MediaInfo {
    /// attached_pic 是专辑封面等静态附图，不能作为视频轨参与抽帧或尺寸计算。
    pub fn video(&self) -> Option<&StreamInfo> {
        self.streams
            .iter()
            .find(|stream| stream.codec_type == "video" && !stream.attached_pic)
    }

    pub fn audio(&self) -> Option<&StreamInfo> {
        self.streams
            .iter()
            .find(|stream| stream.codec_type == "audio")
    }

    pub fn require_duration(&self) -> Result<f64> {
        let duration = self
            .duration
            .context("媒体缺少时长，无法执行需要确定时长的操作")?;
        ensure!(
            duration.is_finite() && duration > 0.0,
            "媒体时长必须是大于 0 的有限数值"
        );
        Ok(duration)
    }
}

impl StreamInfo {
    /// 先按像素宽高比恢复显示宽度，再应用旋转；结果才是用户看到的方向和比例。
    pub fn display_size(&self) -> Result<(u32, u32)> {
        let width = self.width.context("视频流缺少宽度")?;
        let height = self.height.context("视频流缺少高度")?;
        ensure!(width > 0 && height > 0, "视频宽高必须大于 0");
        let ratio = match self.sample_aspect_ratio.as_deref() {
            None | Some("N/A") => 1.0,
            Some(value) => {
                let (numerator, denominator) = value
                    .split_once(':')
                    .context("无效的视频 sample_aspect_ratio")?;
                let numerator: u64 = numerator
                    .parse()
                    .context("无效的 sample_aspect_ratio 分子")?;
                let denominator: u64 = denominator
                    .parse()
                    .context("无效的 sample_aspect_ratio 分母")?;
                ensure!(
                    numerator > 0 && denominator > 0,
                    "sample_aspect_ratio 必须为正比例"
                );
                numerator as f64 / denominator as f64
            }
        };
        let display_width = (f64::from(width) * ratio).round();
        ensure!(
            display_width.is_finite()
                && display_width >= 1.0
                && display_width <= f64::from(u32::MAX),
            "视频显示宽度超出有效范围"
        );
        let rotation = self.rotation.rem_euclid(360);
        ensure!(
            rotation % 90 == 0,
            "暂不支持非 90 度倍数的显示旋转：{}",
            self.rotation
        );
        Ok(if rotation == 90 || rotation == 270 {
            (height, display_width as u32)
        } else {
            (display_width as u32, height)
        })
    }
}

/// 先确认文件类型，再打开以确认可读；只读取 metadata，不将视频整体装入内存。
/// canonicalize 同时解析可读符号链接，并让带空格或以 '-' 开头的文件名保持为单个绝对路径参数。
pub fn input_path(path: &Path) -> Result<PathBuf> {
    validate_local_path_syntax(path)?;
    let absolute = path
        .canonicalize()
        .with_context(|| format!("无法访问媒体输入：{}", path.display()))?;
    // 相对路径或符号链接解析后仍须指向本地路径，不能借此绕过网络/设备前缀检查。
    validate_local_path_syntax(&absolute)?;
    ensure!(
        absolute.metadata()?.is_file(),
        "媒体输入必须是普通文件：{}",
        path.display()
    );
    fs::File::open(&absolute).with_context(|| format!("媒体输入不可读：{}", path.display()))?;
    Ok(absolute)
}

fn validate_local_path_syntax(path: &Path) -> Result<()> {
    let text = path.to_string_lossy();
    let scheme = text.split_once(':').is_some_and(|(prefix, _)| {
        prefix.len() > 1
            && prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    #[cfg(windows)]
    let local_prefix = match path.components().next() {
        Some(std::path::Component::Prefix(prefix)) => match prefix.kind() {
            std::path::Prefix::Disk(_) => true,
            // Windows canonicalize 产生 \\?\C:\...；这是本地盘符，不是 UNC 共享。
            // 保留原始前缀以支持长路径，且不能同时放行 \\?\UNC、\\.\ 或其他设备命名空间。
            std::path::Prefix::VerbatimDisk(_) => path.is_absolute(),
            _ => false,
        },
        // 不完整的双反斜杠前缀也应在访问文件之前拒绝。
        _ => !text.starts_with("\\\\"),
    };
    #[cfg(not(windows))]
    let local_prefix = !text.starts_with("\\\\");
    ensure!(
        path.as_os_str() != "-" && !scheme && local_prefix,
        "媒体输入只支持本地普通文件，不支持 URL 或 stdin：{}",
        path.display()
    );
    Ok(())
}

fn run_backend(
    variable: &str,
    default: &str,
    args: &[OsString],
    timeout: Duration,
) -> Result<crate::backend::ProcessOutput> {
    let program = crate::backend::program(variable, default);
    crate::backend::run_process(&program, args, Some(timeout)).with_context(|| {
        format!("{default} 后端执行失败；请检查 {variable} 指定的程序路径和执行权限，或确认 {default} 已安装并可从 PATH 执行")
    })
}

pub fn ffmpeg(args: &[OsString], timeout: Duration) -> Result<crate::backend::ProcessOutput> {
    run_backend("KITO_FFMPEG", "ffmpeg", args, timeout)
}

/// 返回选中音视频轨自身的时长，不能用其他轨道或容器的结束时刻代替。
/// 部分 MKV 没有 stream.duration，且 DURATION tag 包含起点偏移；此时只解码该轨道，
/// 将 PTS 归零后从最终进度测量相对结束时刻。调用者只在裁剪、淡出等确需时长时使用。
pub fn stream_duration(info: &MediaInfo, stream: &StreamInfo, timeout: Duration) -> Result<f64> {
    let video = match stream.codec_type.as_str() {
        "video" if !stream.attached_pic => true,
        "audio" => false,
        _ => bail!("只能测量实际音频或视频轨的时长，封面不作为视频"),
    };
    if let Some(duration) = stream
        .duration
        .filter(|value| value.is_finite() && *value > 0.0)
    {
        return Ok(duration);
    }
    let path = input_path(&info.path)?;
    let mut args: Vec<OsString> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-xerror",
        // progress 没有音频样本计数；FFmpeg 必须确认输出过真实帧/样本，不能只依赖时钟值。
        "-abort_on",
        "empty_output+empty_output_stream",
        "-nostats",
        "-progress",
        "pipe:1",
        "-stats_period",
        "3600",
        "-protocol_whitelist",
        INPUT_PROTOCOL_WHITELIST,
        "-format_whitelist",
        INPUT_FORMAT_WHITELIST,
        "-i",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.push(path.into_os_string());
    args.extend([OsString::from("-map"), format!("0:{}", stream.index).into()]);
    if video {
        args.extend(["-vf", "setpts=PTS-STARTPTS", "-fps_mode", "passthrough"].map(Into::into));
    } else {
        args.extend(["-af", "asetpts=PTS-STARTPTS"].map(Into::into));
    }
    args.extend(["-f", "null", "-"].map(Into::into));
    let output = ffmpeg(&args, timeout).context("无法完整解码选中轨道来测量时长")?;
    progress_duration(&output.stdout, video).context("KITO_FFMPEG 后端未返回可靠的轨道解码时长")
}

fn progress_duration(bytes: &[u8], video: bool) -> Result<f64> {
    let text = std::str::from_utf8(bytes).context("FFmpeg progress 不是有效 UTF-8")?;
    let mut out_time = None;
    let mut frames = None;
    let mut duration = None;
    for line in text.lines().filter(|line| !line.is_empty()) {
        ensure!(duration.is_none(), "FFmpeg 最终 progress 后出现额外输出");
        let (key, value) = line.split_once('=').context("FFmpeg progress 格式无效")?;
        match key {
            "out_time_us" => out_time = Some(value),
            "frame" => frames = Some(value),
            "progress" => {
                if value == "end" {
                    let micros = out_time
                        .context("最终 progress 缺少 out_time_us")?
                        .parse::<i64>()
                        .context("out_time_us 必须是有效整数")?;
                    ensure!(micros > 0, "选中轨道没有大于 0 的解码时长");
                    if video {
                        let frames = frames
                            .context("最终 progress 缺少视频帧数")?
                            .parse::<u64>()
                            .context("视频帧数无效")?;
                        ensure!(frames > 0, "选中轨道没有解码出视频帧");
                    }
                    duration = Some(micros as f64 / 1_000_000.0);
                } else {
                    ensure!(value == "continue", "FFmpeg progress 状态无效");
                }
                // 每份 progress 都独立校验，不能借用先前 continue 块里的时长或帧数。
                out_time = None;
                frames = None;
            }
            _ => {}
        }
    }
    duration.context("FFmpeg 没有完整结束轨道解码")
}

pub fn probe(path: &Path, timeout: Duration) -> Result<MediaInfo> {
    probe_with_demuxer(path, timeout, None)
}

/// 仅供内部验证已知编码格式的候选输出；普通用户输入继续按内容自动探测。
/// 即使显式指定格式，协议和 demuxer 白名单也不能省略。
pub(super) fn probe_with_demuxer(
    path: &Path,
    timeout: Duration,
    demuxer: Option<&str>,
) -> Result<MediaInfo> {
    let path = input_path(path)?;
    let size_bytes = path.metadata()?.len();
    // 协议和 demuxer 双重白名单：阻止网络访问，也阻止播放列表读取其他本地文件。
    let mut args: Vec<OsString> = [
        "-v",
        "error",
        "-protocol_whitelist",
        INPUT_PROTOCOL_WHITELIST,
        "-format_whitelist",
        INPUT_FORMAT_WHITELIST,
        "-show_format",
        "-show_streams",
        "-of",
        "json",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    if let Some(demuxer) = demuxer {
        args.extend([OsString::from("-f"), OsString::from(demuxer)]);
    }
    args.push("-i".into());
    args.push(path.as_os_str().to_owned());
    let output = run_backend("KITO_FFPROBE", "ffprobe", &args, timeout)?;
    parse_metadata(path, size_bytes, &output.stdout)
        .context("KITO_FFPROBE 后端返回的媒体元数据无效")
}

pub fn doctor() -> Value {
    let ffmpeg = backend_report("KITO_FFMPEG", "ffmpeg");
    let ffprobe = backend_report("KITO_FFPROBE", "ffprobe");
    json!({
        "kito": env!("CARGO_PKG_VERSION"),
        "healthy": ffmpeg["error"].is_null() && ffprobe["error"].is_null(),
        "ffmpeg": ffmpeg,
        "ffprobe": ffprobe,
    })
}

fn backend_report(variable: &str, default: &str) -> Value {
    let program = crate::backend::program(variable, default);
    let version = run_backend(
        variable,
        default,
        &["-version".into()],
        Duration::from_secs(5),
    )
    .and_then(|output| {
        let text = String::from_utf8(output.stdout).context("版本输出不是有效 UTF-8")?;
        let line = text
            .lines()
            .find(|line| !line.trim().is_empty())
            .context("后端未返回版本信息")?;
        Ok(line.trim().to_owned())
    });
    match version {
        Ok(version) => {
            json!({"path": program.to_string_lossy(), "version": version, "error": null})
        }
        Err(error) => {
            json!({"path": program.to_string_lossy(), "version": null, "error": format!("{variable}: {error:#}")})
        }
    }
}

// ffprobe 的结构化字段使用严格类型；时长等 FFmpeg 惯用字符串字段单独校验。
// 未提供的可选字段保留 None，而不是伪造 0 或默认时长。
#[derive(Deserialize)]
struct RawProbe {
    #[serde(default)]
    streams: Vec<RawStream>,
    format: Option<RawFormat>,
}

#[derive(Deserialize)]
struct RawFormat {
    format_name: Option<String>,
    duration: Option<Value>,
}

#[derive(Deserialize)]
struct RawStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<Value>,
    start_time: Option<Value>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    disposition: Option<RawDisposition>,
    tags: Option<RawTags>,
    #[serde(default)]
    side_data_list: Vec<RawSideData>,
    sample_aspect_ratio: Option<String>,
}

#[derive(Deserialize)]
struct RawDisposition {
    #[serde(default)]
    attached_pic: u8,
}

#[derive(Deserialize)]
struct RawTags {
    rotate: Option<Value>,
}

#[derive(Deserialize)]
struct RawSideData {
    side_data_type: Option<String>,
    rotation: Option<Value>,
}

fn parse_metadata(path: PathBuf, size_bytes: u64, bytes: &[u8]) -> Result<MediaInfo> {
    let raw: RawProbe = serde_json::from_slice(bytes).context("ffprobe JSON 结构无效")?;
    let format_duration = raw
        .format
        .as_ref()
        .map(|format| number(format.duration.as_ref(), "format.duration"))
        .transpose()?
        .flatten();
    let mut streams = Vec::with_capacity(raw.streams.len());
    for stream in raw.streams {
        let duration = number(stream.duration.as_ref(), "stream.duration")?;
        let start_time = stream
            .start_time
            .as_ref()
            .filter(|value| value.as_str() != Some("N/A"))
            .map(|value| numeric_value(value, "stream.start_time"))
            .transpose()?;
        let fps = frame_rate(stream.avg_frame_rate.as_deref())?
            .or(frame_rate(stream.r_frame_rate.as_deref())?);
        let sample_rate = stream
            .sample_rate
            .as_deref()
            .filter(|value| *value != "N/A")
            .map(|value| value.parse::<u32>().context("无效的音频 sample_rate"))
            .transpose()?;
        let matrix_rotation = stream
            .side_data_list
            .iter()
            .find(|data| {
                data.side_data_type
                    .as_deref()
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("Display Matrix"))
                    && data.rotation.is_some()
            })
            .and_then(|data| data.rotation.as_ref());
        let tag_rotation = stream.tags.as_ref().and_then(|tags| tags.rotate.as_ref());
        let rotation = rotation(matrix_rotation.or(tag_rotation))?;
        streams.push(StreamInfo {
            index: stream.index,
            codec_type: stream.codec_type.unwrap_or_else(|| "unknown".to_owned()),
            codec: stream.codec_name,
            width: stream.width,
            height: stream.height,
            duration,
            start_time,
            fps,
            sample_rate,
            channels: stream.channels,
            attached_pic: stream
                .disposition
                .is_some_and(|value| value.attached_pic != 0),
            rotation,
            sample_aspect_ratio: stream.sample_aspect_ratio.filter(|value| value != "N/A"),
        });
    }
    let duration = format_duration.or_else(|| {
        streams
            .iter()
            .find(|stream| stream.codec_type == "video" && !stream.attached_pic)
            .and_then(|stream| stream.duration)
    });
    Ok(MediaInfo {
        path,
        duration,
        size_bytes,
        format: raw.format.and_then(|format| format.format_name),
        streams,
    })
}

fn numeric_value(value: &Value, field: &str) -> Result<f64> {
    let number = match value {
        Value::String(value) => value
            .parse::<f64>()
            .with_context(|| format!("{field} 不是有效数值"))?,
        Value::Number(value) => value
            .as_f64()
            .with_context(|| format!("{field} 超出数值范围"))?,
        _ => bail!("{field} 必须是数值或数值字符串"),
    };
    ensure!(number.is_finite(), "{field} 必须是有限数值");
    Ok(number)
}

fn number(value: Option<&Value>, field: &str) -> Result<Option<f64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.as_str() == Some("N/A") {
        return Ok(None);
    }
    let number = numeric_value(value, field)?;
    ensure!(number >= 0.0, "{field} 不得为负数");
    Ok(Some(number))
}

fn rotation(value: Option<&Value>) -> Result<i32> {
    let Some(value) = value else {
        return Ok(0);
    };
    let value = numeric_value(value, "rotation")?;
    ensure!(
        value >= f64::from(i32::MIN)
            && value <= f64::from(i32::MAX)
            && (value - value.round()).abs() < 0.000_001,
        "rotation 必须是有效整数角度"
    );
    Ok(value.round() as i32)
}

fn frame_rate(value: Option<&str>) -> Result<Option<f64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if matches!(value, "N/A" | "0/0" | "0") {
        return Ok(None);
    }
    let (numerator, denominator) = value.split_once('/').unwrap_or((value, "1"));
    let numerator = numerator.parse::<f64>().context("无效的帧率分子")?;
    let denominator = denominator.parse::<f64>().context("无效的帧率分母")?;
    ensure!(
        numerator.is_finite() && denominator.is_finite() && numerator >= 0.0 && denominator > 0.0,
        "帧率必须是有效的非负有限比例"
    );
    let rate = numerator / denominator;
    ensure!(rate.is_finite(), "帧率超出有效范围");
    Ok((rate > 0.0).then_some(rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(value: serde_json::Value) -> anyhow::Result<MediaInfo> {
        parse_metadata(
            PathBuf::from("/tmp/example.mp4"),
            1234,
            &serde_json::to_vec(&value)?,
        )
    }

    #[test]
    fn decoded_duration_uses_only_the_final_complete_progress_block() {
        let progress = b"frame=1\nout_time_us=100000\nprogress=continue\nframe=10\nout_time_us=1000000\nprogress=end\n";
        assert_eq!(progress_duration(progress, true).unwrap(), 1.0);
        assert_eq!(
            progress_duration(b"out_time_us=2000000\nprogress=end\n", false).unwrap(),
            2.0
        );
    }

    #[test]
    fn decoded_duration_rejects_missing_nonpositive_or_incomplete_results() {
        for progress in [
            "out_time_us=1000000\nprogress=continue\n",
            "out_time_us=0\nprogress=end\n",
            "out_time_us=-1\nprogress=end\n",
            "out_time_us=NaN\nprogress=end\n",
            "out_time_us=9223372036854775808\nprogress=end\n",
            "progress=end\n",
            "out_time_us=1000000\nprogress=continue\nprogress=end\n",
        ] {
            assert!(
                progress_duration(progress.as_bytes(), false).is_err(),
                "{progress}"
            );
        }
        assert!(progress_duration(b"frame=0\nout_time_us=1000000\nprogress=end\n", true).is_err());
        assert!(progress_duration(b"out_time_us=1000000\nprogress=end\n", true).is_err());
    }

    #[test]
    fn explicit_positive_stream_duration_does_not_require_the_media_file() {
        let info = parse(json!({"streams":[{"index":0,"codec_type":"audio","duration":"1.25"}],"format":{"duration":"99"}})).unwrap();
        assert_eq!(
            stream_duration(&info, info.audio().unwrap(), Duration::from_secs(1)).unwrap(),
            1.25
        );
    }

    #[test]
    fn selects_real_video_instead_of_attached_cover_and_uses_its_duration() {
        let media = parse(json!({"streams": [
            {"index": 0, "codec_type": "video", "codec_name": "mjpeg", "width": 600, "height": 600,
             "duration": "99.0", "disposition": {"attached_pic": 1}},
            {"index": 1, "codec_type": "audio", "codec_name": "aac", "sample_rate": "48000", "channels": 2},
            {"index": 2, "codec_type": "video", "codec_name": "h264", "width": 720, "height": 576,
             "duration": "2.5", "avg_frame_rate": "30000/1001"}
        ], "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2", "duration": "N/A"}})).unwrap();
        assert_eq!(media.video().unwrap().index, 2);
        assert_eq!(media.audio().unwrap().sample_rate, Some(48000));
        assert_eq!(media.duration, Some(2.5));
        assert_eq!(media.size_bytes, 1234);
        assert!((media.video().unwrap().fps.unwrap() - 29.970_029_970).abs() < 0.000_001);
    }

    #[test]
    fn missing_metadata_remains_missing_and_cannot_drive_timed_operations() {
        let media = parse(json!({"streams": [{"index": 0, "codec_type": "audio"}]})).unwrap();
        assert!(media.duration.is_none());
        assert!(media.format.is_none());
        assert!(media.require_duration().is_err());
        assert!(media.video().is_none());
        assert!(media.audio().unwrap().sample_rate.is_none());
    }

    #[test]
    fn format_duration_wins_and_cover_duration_is_never_fallback() {
        let media = parse(json!({"streams": [
            {"index": 0, "codec_type": "video", "duration": "2.5"}
        ], "format": {"duration": "4.0"}}))
        .unwrap();
        assert_eq!(media.require_duration().unwrap(), 4.0);
        let cover = parse(json!({"streams": [
            {"index": 0, "codec_type": "video", "duration": "99", "disposition": {"attached_pic": 1}}
        ]})).unwrap();
        assert!(cover.duration.is_none());
    }

    #[test]
    fn malformed_json_types_and_non_finite_durations_are_rejected() {
        for value in [
            json!([]),
            json!({"streams": {}}),
            json!({"streams": [{"codec_type": "video"}]}),
            json!({"streams": [{"index": -1, "codec_type": "video"}]}),
            json!({"format": {"duration": "NaN"}}),
            json!({"format": {"duration": "Infinity"}}),
            json!({"format": {"duration": "-1"}}),
            json!({"streams": [{"index": 0, "duration": "NaN"}]}),
            json!({"streams": [{"index": 0, "width": "1920"}]}),
        ] {
            assert!(parse(value.clone()).is_err(), "accepted {value}");
        }
        assert!(parse_metadata(PathBuf::from("x"), 1, b"{").is_err());
    }

    #[test]
    fn display_size_applies_sar_before_rotation_and_prefers_displaymatrix() {
        let media = parse(json!({"streams": [{
            "index": 0, "codec_type": "video", "width": 720, "height": 576,
            "sample_aspect_ratio": "16:15", "tags": {"rotate": "180"},
            "side_data_list": [{"side_data_type": "Display Matrix", "rotation": -90}]
        }]}))
        .unwrap();
        let video = media.video().unwrap();
        assert_eq!(video.rotation, -90);
        assert_eq!(video.display_size().unwrap(), (576, 768));
    }

    #[test]
    fn display_size_rounds_sar_and_accepts_tag_rotation() {
        let media = parse(json!({"streams": [{
            "index": 0, "codec_type": "video", "width": 101, "height": 60,
            "sample_aspect_ratio": "4:3", "tags": {"rotate": "270"}
        }]}))
        .unwrap();
        assert_eq!(media.video().unwrap().display_size().unwrap(), (60, 135));
    }

    #[test]
    fn display_size_rejects_absent_zero_overflow_and_invalid_sar() {
        for stream in [
            json!({"index": 0, "codec_type": "video"}),
            json!({"index": 0, "codec_type": "video", "width": 0, "height": 1}),
            json!({"index": 0, "codec_type": "video", "width": 100, "height": 100, "sample_aspect_ratio": "1:0"}),
            json!({"index": 0, "codec_type": "video", "width": 100, "height": 100, "sample_aspect_ratio": "NaN:1"}),
            json!({"index": 0, "codec_type": "video", "width": 4294967295_u64, "height": 1, "sample_aspect_ratio": "2:1"}),
            json!({"index": 0, "codec_type": "video", "width": 1, "height": 1, "sample_aspect_ratio": "1:10"}),
        ] {
            let media = parse(json!({"streams": [stream]})).unwrap();
            assert!(media.video().unwrap().display_size().is_err());
        }
    }

    #[test]
    fn unknown_fps_is_absent_and_valid_rate_can_fall_back() {
        let media = parse(json!({"streams": [
            {"index": 0, "codec_type": "video", "avg_frame_rate": "0/0", "r_frame_rate": "24/1"},
            {"index": 1, "codec_type": "video", "avg_frame_rate": "0/0", "r_frame_rate": "0/0"}
        ]}))
        .unwrap();
        assert_eq!(media.streams[0].fps, Some(24.0));
        assert_eq!(media.streams[1].fps, None);
    }
}
