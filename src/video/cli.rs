//! Vido 的命令行契约。这里只解析参数，媒体探测、处理与安全发布交给视频模块。
use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct VideoArgs {
    #[command(subcommand)]
    pub command: VideoCommand,
}

#[derive(Subcommand, Debug)]
pub enum VideoCommand {
    /// 检查 FFmpeg / ffprobe 的路径与版本
    Doctor {
        /// 输出 JSON 报告
        #[arg(long)]
        json: bool,
    },
    /// 查看容器、音视频轨道、尺寸与时长
    Info {
        /// 本地视频文件路径
        input: PathBuf,
        /// 输出 JSON 报告
        #[arg(long)]
        json: bool,
    },
    /// 重新编码为 MP4、MKV、WebM 或 MOV
    Convert(ConvertArgs),
    /// 以体积优先的默认质量压缩视频；结果不保证更小
    #[command(mut_arg("quality", |arg| arg.default_value("small")))]
    Compress(EncodeInput),
    /// 按起点和终点或时长裁剪片段
    Trim(TrimArgs),
    /// 等比调整视频尺寸，允许放大
    Resize(ResizeArgs),
    /// 从画面左上角坐标裁切矩形区域
    Crop(CropArgs),
    /// 顺时针旋转 90、180 或 270 度
    Rotate(RotateArgs),
    /// 调整播放速度，同步音频并保持音调
    Speed(SpeedArgs),
    /// 按输入顺序拼接视频，统一画面与音频规格
    Concat(ConcatArgs),
    /// 移除视频中的音频轨道
    Mute(EncodeInput),
    /// 音轨提取与替换，以及音乐转换、裁剪、拼接、混音和响度处理
    Audio(AudioArgs),
    /// 截取指定时刻的 PNG 图片
    Snapshot(SnapshotArgs),
    /// 将短片段生成为使用调色板的 GIF
    Gif(GifArgs),
}

/// 输出参数共用同一安全规则：默认生成新文件，验证候选后再原子发布。
#[derive(Args, Debug)]
pub struct OutputArgs {
    /// 输出路径；默认在输入旁生成带操作名的新文件
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// 允许替换已存在的输出文件；仍禁止覆盖任何输入
    #[arg(long)]
    pub overwrite: bool,
    /// 输出 JSON 报告
    #[arg(long)]
    pub json: bool,
    /// 每次 FFmpeg / ffprobe 子进程的时限，单位秒
    #[arg(long, default_value_t = 3600, value_parser = clap::value_parser!(u64).range(1..))]
    pub time_limit: u64,
    /// 只探测输入并展示结构化 FFmpeg 参数，不编码、不创建目标
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct EncodingArgs {
    /// 质量档位；high 更清晰、small 更注重体积，均为有损编码
    #[arg(long, value_enum, default_value = "balanced")]
    pub quality: Quality,
    /// 编码速度档位；slow 通常耗时更长，可能改善压缩效率
    #[arg(long, value_enum, default_value = "medium")]
    pub preset: Preset,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum Quality {
    High,
    Balanced,
    Small,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum Preset {
    Fast,
    Medium,
    Slow,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum VideoFormat {
    Mp4,
    Mkv,
    Webm,
    Mov,
}

impl VideoFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::Webm => "webm",
            Self::Mov => "mov",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    M4a,
    Wav,
    Flac,
}

impl AudioFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::M4a => "m4a",
            Self::Wav => "wav",
            Self::Flac => "flac",
        }
    }
}

#[derive(Args, Debug)]
pub struct EncodeInput {
    /// 本地视频文件路径
    pub input: PathBuf,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

#[derive(Args, Debug)]
pub struct ConvertArgs {
    /// 本地视频文件路径
    pub input: PathBuf,
    /// 目标容器格式；MP4/MKV/MOV 使用 H.264/AAC，WebM 使用 VP9/Opus
    #[arg(long, value_enum)]
    pub to: VideoFormat,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

#[derive(Args, Debug)]
pub struct TrimArgs {
    /// 本地视频文件路径
    pub input: PathBuf,
    /// 起点，支持秒数或 HH:MM:SS[.小数]
    #[arg(long, default_value = "0")]
    pub start: String,
    /// 终点；与 --duration 二选一
    #[arg(long, conflicts_with = "duration")]
    pub end: Option<String>,
    /// 片段时长；与 --end 二选一
    #[arg(long, required_unless_present = "end")]
    pub duration: Option<String>,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

#[derive(Args, Debug)]
pub struct ResizeArgs {
    /// 本地视频文件路径
    pub input: PathBuf,
    /// 等比缩放：1280x720、1280x、x720 或 50%；允许放大
    #[arg(long, value_name = "SIZE")]
    pub size: String,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

#[derive(Args, Debug)]
pub struct CropArgs {
    /// 本地视频文件路径
    pub input: PathBuf,
    /// 裁切区域宽度，单位像素，必须为偶数
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub width: u32,
    /// 裁切区域高度，单位像素，必须为偶数
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub height: u32,
    /// 裁切区域左边缘坐标，单位像素，必须为偶数
    #[arg(long, default_value_t = 0)]
    pub x: u32,
    /// 裁切区域上边缘坐标，单位像素，必须为偶数
    #[arg(long, default_value_t = 0)]
    pub y: u32,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

#[derive(Args, Debug)]
pub struct RotateArgs {
    /// 本地视频文件路径
    pub input: PathBuf,
    /// 顺时针旋转角度：90、180 或 270
    #[arg(long, value_parser = parse_angle)]
    pub angle: u16,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

fn parse_angle(value: &str) -> std::result::Result<u16, String> {
    match value {
        "90" => Ok(90),
        "180" => Ok(180),
        "270" => Ok(270),
        _ => Err("旋转角度仅支持 90、180 或 270".into()),
    }
}

#[derive(Args, Debug)]
pub struct SpeedArgs {
    /// 本地视频文件路径
    pub input: PathBuf,
    /// 速度倍数 0.125..=8，例如 2 表示两倍速、0.5 表示半速
    #[arg(long)]
    pub factor: f64,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

#[derive(Args, Debug)]
pub struct ConcatArgs {
    /// 按播放顺序排列的本地视频文件，2 至 64 个
    #[arg(num_args = 2.., value_name = "INPUTS")]
    pub inputs: Vec<PathBuf>,
    /// 统一画布 WIDTHxHEIGHT（偶数）；默认首段显示尺寸，等比缩放并补黑边
    #[arg(long, value_name = "SIZE")]
    pub size: Option<String>,
    /// 统一输出帧率，范围 1..=120
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u16).range(1..))]
    pub fps: u16,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

#[derive(Args, Debug)]
pub struct AudioArgs {
    #[command(subcommand)]
    pub command: AudioCommand,
}

#[derive(Subcommand, Debug)]
pub enum AudioCommand {
    /// 将第一条音轨导出为 MP3、M4A、WAV 或 FLAC
    Extract(AudioExtract),
    /// 以另一文件的第一条音轨替换原音轨；短音轨补静音、长音轨截断
    Replace(AudioReplace),
    /// 按线性倍数调整音量；1 保持原音量，0 静音
    Volume(AudioVolume),
    /// 将第一条音轨转换为 MP3、M4A、WAV 或 FLAC
    Convert(AudioConvert),
    /// 按起点和终点或时长裁剪音频
    Trim(AudioTrim),
    /// 按顺序拼接音频，统一为 48 kHz 立体声
    Concat(AudioConcat),
    /// 等权混合多个音频，以最长轨为准
    Mix(AudioMix),
    /// 调整音频播放速度并保持音调
    Speed(AudioSpeed),
    /// 为音频添加淡入与淡出
    Fade(AudioFade),
    /// 按目标 LUFS 做单遍响度处理
    Normalize(AudioNormalize),
}

#[derive(Args, Debug)]
pub struct AudioExtract {
    /// 包含音轨的本地文件路径
    pub input: PathBuf,
    /// 输出音频格式
    #[arg(long, value_enum, default_value = "mp3")]
    pub to: AudioFormat,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct AudioReplace {
    /// 本地视频文件路径
    pub input: PathBuf,
    /// 用作替换音轨的本地文件
    #[arg(long, value_name = "PATH")]
    pub audio: PathBuf,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

#[derive(Args, Debug)]
pub struct AudioVolume {
    /// 包含音轨的本地视频或音频文件
    pub input: PathBuf,
    /// 线性音量倍数 0..=16；1 保持原音量，0 静音
    #[arg(long)]
    pub factor: f64,
    /// 仅输出指定音频格式；纯音频输入省略时默认 MP3
    #[arg(long, value_enum)]
    pub to: Option<AudioFormat>,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub encoding: EncodingArgs,
}

#[derive(Args, Debug)]
pub struct AudioConvert {
    /// 包含音轨的本地文件
    pub input: PathBuf,
    /// 输出音频格式；有损源转为 WAV/FLAC 不会恢复丢失的信息
    #[arg(long, value_enum, default_value = "mp3")]
    pub to: AudioFormat,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct AudioTrim {
    /// 包含音轨的本地文件
    pub input: PathBuf,
    /// 起点，支持秒数或 HH:MM:SS[.小数]
    #[arg(long, default_value = "0")]
    pub start: String,
    /// 终点；与 --duration 二选一
    #[arg(long, conflicts_with = "duration")]
    pub end: Option<String>,
    /// 片段时长；与 --end 二选一
    #[arg(long, required_unless_present = "end")]
    pub duration: Option<String>,
    /// 输出音频格式
    #[arg(long, value_enum, default_value = "mp3")]
    pub to: AudioFormat,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct AudioConcat {
    /// 按播放顺序排列的本地文件，2 至 64 个；每个文件取第一条音轨
    #[arg(num_args = 2.., value_name = "INPUTS")]
    pub inputs: Vec<PathBuf>,
    /// 输出音频格式
    #[arg(long, value_enum, default_value = "mp3")]
    pub to: AudioFormat,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct AudioMix {
    /// 同时混合的本地文件，2 至 64 个；每个文件取第一条音轨
    #[arg(num_args = 2.., value_name = "INPUTS")]
    pub inputs: Vec<PathBuf>,
    /// 输出音频格式
    #[arg(long, value_enum, default_value = "mp3")]
    pub to: AudioFormat,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct AudioSpeed {
    /// 包含音轨的本地文件
    pub input: PathBuf,
    /// 速度倍数 0.125..=8，例如 2 表示两倍速、0.5 表示半速
    #[arg(long)]
    pub factor: f64,
    /// 输出音频格式
    #[arg(long, value_enum, default_value = "mp3")]
    pub to: AudioFormat,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct AudioFade {
    /// 包含音轨的本地文件
    pub input: PathBuf,
    /// 从开头开始淡入的时长，支持秒数或 HH:MM:SS[.小数]
    #[arg(long, default_value = "0")]
    pub fade_in: String,
    /// 在结束前淡出的时长，支持秒数或 HH:MM:SS[.小数]
    #[arg(long, default_value = "0")]
    pub fade_out: String,
    /// 输出音频格式
    #[arg(long, value_enum, default_value = "mp3")]
    pub to: AudioFormat,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct AudioNormalize {
    /// 包含音轨的本地文件
    pub input: PathBuf,
    /// 目标综合响度 -70..=-5 LUFS；单遍处理不保证精确达到目标
    #[arg(long, default_value_t = -16.0, allow_hyphen_values = true)]
    pub loudness: f64,
    /// 输出音频格式
    #[arg(long, value_enum, default_value = "mp3")]
    pub to: AudioFormat,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct SnapshotArgs {
    /// 本地视频文件路径
    pub input: PathBuf,
    /// 截图时刻，支持秒数或 HH:MM:SS[.小数]
    #[arg(long, default_value = "0")]
    pub at: String,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct GifArgs {
    /// 本地视频文件路径
    pub input: PathBuf,
    /// 片段起点，支持秒数或 HH:MM:SS[.小数]
    #[arg(long, default_value = "0")]
    pub start: String,
    /// 片段时长，最多 60 秒，支持秒数或 HH:MM:SS[.小数]
    #[arg(long, default_value = "5")]
    pub duration: String,
    /// 输出宽度 2..=4096，按原画面比例计算高度
    #[arg(long, default_value_t = 480, value_parser = clap::value_parser!(u32).range(1..))]
    pub width: u32,
    /// 输出帧率 1..=50
    #[arg(long, default_value_t = 12, value_parser = clap::value_parser!(u16).range(1..))]
    pub fps: u16,
    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn run(args: VideoArgs) -> Result<bool> {
    super::run(args.command)
}
