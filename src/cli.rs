use crate::{
    backend, files,
    image::{
        benchmark, convert, exif,
        optimize::{self, OptimizeConfig},
        png,
        resize::{ResizeReport, ResizeSpec},
    },
};
use anyhow::{Context, Result, ensure};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rayon::prelude::*;
use serde::Serialize;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

mod help;

#[derive(Parser)]
#[command(
    name = "kito",
    version,
    about = "Kito 奇托——各有所长，一起把事情做好。",
    long_about = "Kito 奇托——各有所长，一起把事情做好。\n\n跨 macOS、Linux、Windows 的 CLI 工具宇宙。每位伙伴专注一个领域，通过清晰的命令协作。",
    after_help = help::ROOT
)]
struct Cli {
    #[command(subcommand)]
    command: Tool,
}
#[derive(Subcommand)]
enum Tool {
    #[command(
        about = "在本机浏览器中使用 Kito 工具小队",
        long_about = "启动内嵌网页，在浏览器中选择文件、设置参数并下载真实处理结果。仅监听本机地址，默认保留原文件；无需 Node.js 或前端服务。"
    )]
    Web(crate::web::WebArgs),
    #[command(
        about = "文本统计与整理",
        long_about = "支持统计文本字符或 UTF-8 字节数、按行去除重复内容，以及大小写转换。"
    )]
    Text(Text),
    #[command(
        about = "Lumi 露米 · 图片伙伴",
        long_about = "Lumi 露米 · 图片伙伴（光影师）\n\n负责图片压缩、格式转换、尺寸调整与 EXIF 信息读取。默认保留原文件，明确说明质量变化，验证成功后再输出。"
    )]
    Image(Image),
    #[command(
        about = "Tiko 提可 · 时间伙伴",
        long_about = "Tiko 提可 · 时间伙伴（时刻向导）\n\n负责时间戳与日期时间互转、时区转换和全球时区查询。支持秒、毫秒、微秒、纳秒；时区可用 IANA 名称或固定偏移。\n\n用 kito time timezone set Asia/Shanghai 保存默认时区。\n默认时区：显式参数 > KITO_TIMEZONE > 保存配置 > UTC。带 offset 的日期输入优先使用自身 offset。"
    )]
    Time(crate::time::cli::TimeArgs),
    #[command(
        about = "Sumi 苏米 · 标识伙伴",
        long_about = "Sumi 苏米 · 标识伙伴（印记师）\n\n负责计算字符串和文件的 Hash，以及生成 UUID。说明算法与版本，结果适合脚本使用。"
    )]
    Id(crate::id::cli::IdArgs),
    #[command(
        about = "Codi 可迪 · 编解码伙伴",
        long_about = "Codi 可迪 · 编解码伙伴（译码师）\n\n负责 Base64 编解码、URL 参数解析，以及 HTML/XML 实体转换。按明确规范处理输入，保留重复参数。"
    )]
    Codec(crate::codec::cli::CodecArgs),
    #[command(
        about = "Kavi 卡维 · 密钥伙伴",
        long_about = "Kavi 卡维 · 密钥伙伴（钥匙匠）\n\n通过 OpenSSL 3.x 生成和检查 RSA / Ed25519 密钥对，导出公钥。私钥采用 PKCS#8，公钥采用 SPKI；Ed25519 用于签名。用 KITO_OPENSSL 指定后端路径。"
    )]
    Key(crate::key::cli::KeyArgs),
    #[command(
        about = "Vido 维多 · 视频伙伴",
        long_about = "Vido 维多 · 视频伙伴（剪影师）\n\n负责视频转换、压缩、裁剪、缩放、旋转、变速、拼接、音轨处理、截图和 GIF。通过 FFmpeg / ffprobe 处理本地文件，默认保留输入；重编码有损，压缩不保证文件更小。\n\n用 kito video doctor 检查后端，KITO_FFMPEG / KITO_FFPROBE 可指定程序路径。"
    )]
    Video(crate::video::cli::VideoArgs),
    #[command(
        about = "Echo 艾可 · 声音伙伴",
        long_about = "Echo 艾可 · 声音伙伴（音效师）\n\n从零创作提示音、游戏音效、旋律和鼓点。内置 Rust 合成器，无需 Node.js、声卡或 FFmpeg；输出 WAV，旋律也可保存为 MIDI。现有音频的转码、裁剪和音轨编辑见 video audio。"
    )]
    Sound(crate::sound::cli::SoundArgs),
}
#[derive(Args)]
#[command(after_help = help::TEXT)]
struct Text {
    #[command(subcommand)]
    command: TextCommand,
}
#[derive(Subcommand)]
enum TextCommand {
    #[command(about = "统计字符串或文件中的字符数", after_help = help::COUNT)]
    Count(CountText),
    #[command(about = "按行去除重复内容，默认输出到终端", after_help = help::DEDUPE)]
    Dedupe(DedupeText),
    #[command(about = "转换文本大小写，默认输出到终端", after_help = help::CASE)]
    Case(CaseText),
}
#[derive(Args)]
struct CountText {
    /// 要统计的字符串；与 --file 二选一。
    #[arg(required_unless_present = "file", conflicts_with = "file")]
    text: Option<String>,
    /// 要统计的 UTF-8 文本文件路径；与字符串参数二选一。
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,
    /// 按 UTF-8 字节计数，而不是按 Unicode 字符计数。
    #[arg(long)]
    bytes: bool,
}
#[derive(Args)]
struct DedupeText {
    /// 要去重的文本；与 --file 二选一。
    #[arg(required_unless_present = "file", conflicts_with = "file")]
    text: Option<String>,
    /// 要去重的 UTF-8 文本文件路径；与字符串参数二选一。
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,
    /// 将结果写入文件；默认直接输出到终端。
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// 允许覆盖已存在的输出文件。
    #[arg(long, requires = "output")]
    overwrite: bool,
}
#[derive(Clone, Copy, ValueEnum)]
enum CaseMode {
    /// 将所有字符转为大写。
    Upper,
    /// 将所有字符转为小写。
    Lower,
    /// 仅将首字符转为大写，后续字符保持原样。
    Capitalize,
}
#[derive(Args)]
struct CaseText {
    /// 转换模式：upper、lower 或 capitalize。
    #[arg(value_enum)]
    mode: CaseMode,
    /// 要转换的文本；与 --file 二选一。
    #[arg(required_unless_present = "file", conflicts_with = "file")]
    text: Option<String>,
    /// 要转换的 UTF-8 文本文件路径；与字符串参数二选一。
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,
    /// capitalize 模式下逐词首字母大写，并将单词其余字符转为小写。
    #[arg(long)]
    words: bool,
    /// 将结果写入文件；默认直接输出到终端。
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// 允许覆盖已存在的输出文件。
    #[arg(long, requires = "output")]
    overwrite: bool,
}
#[derive(Args)]
#[command(after_help = help::IMAGE)]
struct Image {
    #[command(subcommand)]
    command: ImageCommand,
}
#[derive(Subcommand)]
enum ImageCommand {
    #[command(about = "优化静态 PNG；可先 --resize，默认保留原文件", after_help = help::COMPRESS)]
    Compress(Compress),
    #[command(about = "静态图片格式转换；默认要求像素无损", after_help = help::CONVERT)]
    Convert(Convert),
    #[command(about = "等比缩放静态图片，允许放大；默认保留原文件", after_help = help::RESIZE)]
    Resize(Resize),
    #[command(about = "列出后端读写能力及 CLI 输出范围", after_help = help::FORMATS)]
    Formats {
        /// 输出紧凑 JSON，便于脚本读取。
        #[arg(long)]
        json: bool,
    },
    #[command(about = "比较 PNG 优化器，生成可复现 JSON 报告", after_help = help::BENCHMARK)]
    Benchmark(Benchmark),
    #[command(about = "查看图片信息", after_help = help::INFO)]
    Info {
        /// 输入图片路径；PNG 内置解析，其他格式需要 ImageMagick。
        input: PathBuf,
        /// 输出紧凑 JSON；默认输出缩进 JSON。
        #[arg(long)]
        json: bool,
    },
    #[command(
        about = "读取 EXIF 信息，无需 ImageMagick",
        long_about = "只读 JPEG、TIFF、PNG、WebP 的 EXIF 标签，包括相机、拍摄时间、曝光和 GPS 等已有信息。\n按文件内容识别格式，不修改原图；无 EXIF 正常返回，损坏元数据报错。不解码验证图片像素。",
        after_help = help::EXIF
    )]
    Exif {
        /// JPEG、TIFF、PNG 或 WebP 图片路径，按文件内容识别格式。
        input: PathBuf,
        /// 输出包含原类型、精确分数和展示值的 JSON。
        #[arg(long)]
        json: bool,
        /// 解析内存预算；文件不超过预算的 1/8，累计字段数据不超过 1/256。
        #[arg(long,default_value_t=512,value_parser=clap::value_parser!(u64).range(1..))]
        memory_mib: u64,
    },
    #[command(about = "检查可选后端", after_help = help::DOCTOR)]
    Doctor {
        /// 输出紧凑 JSON，便于脚本检查后端状态。
        #[arg(long)]
        json: bool,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum Effort {
    /// 使用较快的优化预设。
    Fast,
    /// 多策略搜索后择优，不代表全局最优压缩。
    Max,
}
#[derive(Args)]
struct Compress {
    /// 静态 PNG 文件或目录；目录默认只处理顶层 PNG。
    input: PathBuf,
    /// 单文件输出路径；不能与 --output-dir 或 --in-place 同用。
    #[arg(short,long,conflicts_with_all=["output_dir","in_place"])]
    output: Option<PathBuf>,
    /// 批量输出目录；保持相对于输入目录的结构。
    #[arg(long, conflicts_with = "in_place")]
    output_dir: Option<PathBuf>,
    /// 递归处理输入目录及其子目录中的 PNG。
    #[arg(short, long)]
    recursive: bool,
    /// 明确允许验证成功后替换输入文件；不能与 --overwrite 同用。
    #[arg(long, conflicts_with = "overwrite")]
    in_place: bool,
    /// 允许覆盖已有输出文件；不能借此覆盖输入。
    #[arg(long)]
    overwrite: bool,
    /// 优化强度：fast 较快，max 尝试多种策略后择优。
    #[arg(long, value_enum, default_value = "max")]
    effort: Effort,
    /// 移除 PNG 文本和时间元数据，保留颜色等显示相关元数据。
    #[arg(long)]
    strip_metadata: bool,
    #[arg(
        long,
        value_name = "SIZE",
        help = "等比缩放：1280x720、1280x、x720、50%；允许放大，需要 ImageMagick"
    )]
    resize: Option<ResizeSpec>,
    /// 并发任务数，至少 1；任务共享总内存预算。
    #[arg(long,default_value_t=1,value_parser=clap::value_parser!(u16).range(1..))]
    jobs: u16,
    /// 总内存预算（MiB），含文件与像素工作集预检，不是 RSS 硬上限。
    #[arg(long,default_value_t=512,value_parser=clap::value_parser!(u64).range(1..))]
    memory_mib: u64,
    /// 每个文件的优化软预算（秒）；内置阶段边界检查，外部进程有超时。
    #[arg(long)]
    time_limit: Option<u64>,
    /// 输出紧凑 JSON 报告，包含成功/失败、体积与无损验证信息。
    #[arg(long)]
    json: bool,
}
#[derive(Args)]
struct Convert {
    /// 输入图片路径；动画、PSD、矢量或 RAW 需显式选择对应处理方式。
    input: PathBuf,
    /// 目标格式，如 png、webp、jpeg；实际能力用 image formats 查看。
    #[arg(long)]
    to: String,
    #[arg(
        long,
        value_name = "SIZE",
        help = "编码前等比缩放：1280x720、1280x、x720、50%；允许放大"
    )]
    resize: Option<ResizeSpec>,
    #[command(flatten)]
    encoding: EncodingArgs,
}
#[derive(Args)]
struct Resize {
    /// 输入图片路径；默认输出到 文件名.resized.格式。
    input: PathBuf,
    #[arg(
        long,
        value_name = "SIZE",
        help = "等比缩放：1280x720、1280x、x720、50%；允许放大"
    )]
    resize: ResizeSpec,
    #[arg(long, help = "输出格式；默认按输出扩展名或输入实际格式选择")]
    to: Option<String>,
    #[command(flatten)]
    encoding: EncodingArgs,
}
#[derive(Args)]
struct EncodingArgs {
    /// 输出路径；默认生成新文件，不修改原图。
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// 允许覆盖已有输出文件，仍禁止覆盖输入或其链接。
    #[arg(long)]
    overwrite: bool,
    /// 要求编码像素无损（默认行为）；缩放时与重采样后的参考图比较。
    #[arg(long, conflicts_with = "allow_lossy")]
    lossless: bool,
    /// 接受颜色、精度和元数据变化；与 --lossless 互斥。
    #[arg(long)]
    allow_lossy: bool,
    /// 有损质量 1–100，需 --allow-lossy；JPEG/AVIF/HEIC/HEIF 要求显式指定。
    #[arg(long,value_parser=clap::value_parser!(u8).range(1..=100))]
    quality: Option<u8>,
    /// 移除透明度时的背景色，如 white；需 --allow-lossy。
    #[arg(long)]
    background: Option<String>,
    /// 选择动画帧/页面，索引从 0 开始；与 --flatten 互斥。
    #[arg(long, conflicts_with = "flatten")]
    frame: Option<usize>,
    /// 明确使用 PSD 合成图，仅适用于 PSD。
    #[arg(long)]
    flatten: bool,
    /// 明确允许 SVG 等矢量/文档按后端默认设置栅格化。
    #[arg(long)]
    rasterize: bool,
    /// 明确允许相机 RAW 按后端默认设置显影。
    #[arg(long)]
    render_raw: bool,
    /// 文件和像素工作集的内存预算（MiB），不是 RSS 硬上限。
    #[arg(long,default_value_t=512,value_parser=clap::value_parser!(u64).range(1..))]
    memory_mib: u64,
    /// 每次后端调用的超时（秒），不是整条命令的总耗时上限。
    #[arg(long,default_value_t=120,value_parser=clap::value_parser!(u64).range(1..))]
    time_limit: u64,
    /// 输出紧凑 JSON 报告，包含格式、尺寸与无损验证结果。
    #[arg(long)]
    json: bool,
}
#[derive(Args)]
struct Benchmark {
    /// PNG 文件或语料目录；目录递归收集 PNG。
    input: PathBuf,
    /// JSON 报告保存路径；省略时输出到终端。
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// 允许覆盖已有报告文件，不允许覆盖输入图片。
    #[arg(long)]
    overwrite: bool,
    /// 优化软预算（秒）；各基线使用相同设置，未完成结果会标记。
    #[arg(long)]
    time_limit: Option<u64>,
    /// 各引擎统一移除 PNG 文本和时间元数据，保留显示相关信息。
    #[arg(long)]
    strip_metadata: bool,
    /// 每个输入的文件与像素工作集预算（MiB）。
    #[arg(long,default_value_t=512,value_parser=clap::value_parser!(u64).range(1..))]
    memory_mib: u64,
}
#[derive(Serialize)]
struct FileReport {
    input: PathBuf,
    output: PathBuf,
    success: bool,
    input_bytes: usize,
    output_bytes: usize,
    saved_bytes: i128,
    verified_lossless: bool,
    encoding_verified_lossless: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    resize: Option<ResizeReport>,
    saved_percent: f64,
    elapsed_ms: u128,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    optimization: Option<optimize::Optimized>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}
fn emit<T: Serialize>(value: &T, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(value)?)
    } else {
        println!("{}", serde_json::to_string_pretty(value)?)
    }
    Ok(())
}
pub fn run() -> Result<bool> {
    match help::parse().command {
        Tool::Web(args) => crate::web::run(args),
        Tool::Text(Text { command }) => match command {
            TextCommand::Count(args) => count_text(args),
            TextCommand::Dedupe(args) => dedupe_text(args),
            TextCommand::Case(args) => convert_text_case(args),
        },
        Tool::Image(Image { command }) => run_image(command),
        Tool::Time(args) => crate::time::cli::run(args),
        Tool::Id(args) => crate::id::cli::run(args),
        Tool::Codec(args) => crate::codec::cli::run(args),
        Tool::Key(args) => crate::key::cli::run(args),
        Tool::Video(args) => crate::video::cli::run(args),
        Tool::Sound(args) => crate::sound::cli::run(args),
    }
}

/// 对文本按行去重；指定输出文件时沿用共享文件安全规则原子发布。
fn dedupe_text(args: DedupeText) -> Result<bool> {
    let input = match (args.text, args.file) {
        (Some(text), None) => text,
        (None, Some(path)) => {
            if let Some(output) = &args.output {
                files::validate_destination(&path, output, false, args.overwrite)?;
            }
            crate::text::read_utf8_file(&path)?
        }
        _ => anyhow::bail!("请提供文本参数或 --file PATH"),
    };

    let result = crate::text::deduplicate_lines(&input);
    if let Some(output) = args.output {
        files::publish(result.as_bytes(), &output, args.overwrite)?;
        println!("输出：{}", output.display());
    } else {
        // 保留处理后的原始换行状态，避免额外添加换行影响管道和重定向结果。
        print!("{result}");
    }
    Ok(true)
}

/// 按指定的 Unicode 大小写规则转换，输出路径沿用安全发布策略。
fn convert_text_case(args: CaseText) -> Result<bool> {
    ensure!(
        !args.words || matches!(args.mode, CaseMode::Capitalize),
        "--words 仅适用于 capitalize 模式"
    );

    let input = match (args.text, args.file) {
        (Some(text), None) => text,
        (None, Some(path)) => {
            if let Some(output) = &args.output {
                files::validate_destination(&path, output, false, args.overwrite)?;
            }
            crate::text::read_utf8_file(&path)?
        }
        _ => anyhow::bail!("请提供文本参数或 --file PATH"),
    };

    let result = match args.mode {
        CaseMode::Upper => input.to_uppercase(),
        CaseMode::Lower => input.to_lowercase(),
        CaseMode::Capitalize if args.words => crate::text::capitalize_words(&input),
        CaseMode::Capitalize => crate::text::capitalize_first(&input),
    };
    if let Some(output) = args.output {
        files::publish(result.as_bytes(), &output, args.overwrite)?;
        println!("输出：{}", output.display());
    } else {
        print!("{result}");
        // 终端输出以换行收尾，避免 shell 提示符紧贴在文本后面；不重复追加空行。
        if !result.ends_with('\n') {
            println!();
        }
    }
    Ok(true)
}

/// 统一处理命令行字符串与文件输入，输出单个数字便于后续管道处理。
fn count_text(args: CountText) -> Result<bool> {
    let text = match (args.text, args.file) {
        (Some(text), None) => text,
        (None, Some(path)) => crate::text::read_utf8_file(&path)?,
        _ => anyhow::bail!("请提供字符串参数或 --file PATH"),
    };

    let count = crate::text::count(&text, args.bytes);
    println!("{count}");
    Ok(true)
}

fn run_image(command: ImageCommand) -> Result<bool> {
    match command {
        ImageCommand::Compress(args) => compress(args),
        ImageCommand::Convert(args) => conversion(args),
        ImageCommand::Resize(args) => resizing(args),
        ImageCommand::Benchmark(args) => {
            let budget = usize::try_from(args.memory_mib)?
                .checked_mul(1024 * 1024)
                .context("预算溢出")?;
            if let Some(path) = &args.output {
                files::validate_destination(&args.input, path, false, args.overwrite)?;
                benchmark::validate_report_path(&args.input, path)?;
            }
            let report = benchmark::run(
                &args.input,
                &OptimizeConfig {
                    max: true,
                    strip: args.strip_metadata,
                    memory_bytes: budget,
                    time_limit: args.time_limit.map(Duration::from_secs),
                },
            )?;
            if let Some(path) = args.output {
                files::publish(&serde_json::to_vec_pretty(&report)?, &path, args.overwrite)?;
                println!("报告：{}", path.display());
            } else {
                emit(&report, true)?;
            }
            Ok(report["totals"]["failed"] == 0)
        }
        ImageCommand::Formats { json } => {
            emit(&convert::formats()?, json)?;
            Ok(true)
        }
        ImageCommand::Info { input, json } => {
            let bytes = files::read_bounded(&input, 512 * 1024 * 1024)?;
            if bytes.starts_with(png::SIGNATURE) {
                emit(&png::inspect(&bytes)?, json)?;
            } else {
                emit(
                    &convert::info(&bytes, input.extension().and_then(|e| e.to_str()))?,
                    json,
                )?;
            }
            Ok(true)
        }
        ImageCommand::Exif {
            input,
            json,
            memory_mib,
        } => {
            let budget = usize::try_from(memory_mib)?
                .checked_mul(1024 * 1024)
                .context("内存预算溢出")?;
            let report = exif::read(&input, budget)?;
            if json {
                emit(&report, true)?;
            } else {
                print!("{}", exif::render_text(&report));
            }
            Ok(true)
        }
        ImageCommand::Doctor { json } => {
            emit(
                &serde_json::json!({"kito":env!("CARGO_PKG_VERSION"),"png":"内置 OxiPNG 10.2.1 + Zopfli","magick":backend::magick_version().map_err(|e|e.to_string()),"zopflipng":backend::zopflipng_version().map_err(|e|e.to_string())}),
                json,
            )?;
            Ok(true)
        }
    }
}
fn compress(args: Compress) -> Result<bool> {
    let jobs = files::plan(
        &args.input,
        args.output.as_deref(),
        args.output_dir.as_deref(),
        args.recursive,
        args.in_place,
        args.overwrite,
    )?;
    let budget = usize::try_from(args.memory_mib)
        .context("内存预算过大")?
        .checked_mul(1024 * 1024)
        .context("内存预算溢出")?
        / usize::from(args.jobs);
    ensure!(budget > 0, "每个并发任务的内存预算不足");
    let config = OptimizeConfig {
        max: matches!(args.effort, Effort::Max),
        strip: args.strip_metadata,
        memory_bytes: budget,
        time_limit: args.time_limit.map(Duration::from_secs),
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(usize::from(args.jobs))
        .build()?;
    let reports: Vec<FileReport> = pool.install(|| {
        jobs.par_iter()
            .map(|job| {
                let start = Instant::now();
                let mut report = FileReport {
                    input: job.input.clone(),
                    output: job.output.clone(),
                    success: false,
                    input_bytes: 0,
                    output_bytes: 0,
                    saved_bytes: 0,
                    verified_lossless: false,
                    encoding_verified_lossless: false,
                    resize: None,
                    saved_percent: 0.0,
                    elapsed_ms: 0,
                    optimization: None,
                    error: None,
                };
                let result = (|| -> Result<()> {
                    let bytes = files::read_bounded(&job.input, budget)?;
                    report.input_bytes = bytes.len();
                    // 原始 PNG 先校验，再执行明确请求的缩放；压缩候选只能在
                    // 缩放后的图片中选择，否则“原文件保底”会悄悄撤销尺寸请求。
                    let mut optimization_source = bytes.clone();
                    if let Some(spec) = &args.resize {
                        png::PngImage::decode(&bytes, budget)?;
                        let remaining =
                            config.time_limit.map(|t| t.saturating_sub(start.elapsed()));
                        ensure!(remaining != Some(Duration::ZERO), "缩放前时间预算已用尽");
                        let resized = convert::prepare_png_for_compression(
                            &bytes,
                            &convert::ConvertConfig {
                                source_hint: Some("png".into()),
                                target: "PNG".into(),
                                resize: Some(spec.clone()),
                                allow_lossy: false,
                                quality: None,
                                background: None,
                                frame: None,
                                flatten: false,
                                rasterize: false,
                                render_raw: false,
                                memory_bytes: budget,
                                timeout: remaining.unwrap_or(Duration::from_secs(120)),
                            },
                        )?;
                        report.resize = resized.resize;
                        optimization_source = resized.bytes;
                    }
                    let mut remaining = config.clone();
                    remaining.time_limit =
                        config.time_limit.map(|t| t.saturating_sub(start.elapsed()));
                    let result = optimize::optimize(&optimization_source, &remaining)?;
                    files::publish(&result.bytes, &job.output, args.overwrite || args.in_place)?;
                    report.output_bytes = result.bytes.len();
                    // 放大后文件可能比原图更大，必须以有符号差值报告，不能下溢。
                    report.saved_bytes = bytes.len() as i128 - result.bytes.len() as i128;
                    report.verified_lossless = !report.resize.as_ref().is_some_and(|r| r.changed);
                    report.encoding_verified_lossless = true;
                    report.saved_percent = 100.0 * report.saved_bytes as f64 / bytes.len() as f64;
                    report.optimization = Some(result);
                    Ok(())
                })();
                match result {
                    Ok(()) => report.success = true,
                    Err(e) => report.error = Some(format!("{e:#}")),
                };
                report.elapsed_ms = start.elapsed().as_millis();
                report
            })
            .collect()
    });
    let failed = reports.iter().filter(|r| !r.success).count();
    if args.json {
        emit(
            &serde_json::json!({"succeeded":reports.len()-failed,"failed":failed,"files":reports}),
            true,
        )?;
    } else {
        for r in &reports {
            if r.success {
                println!(
                    "{} → {}：{} → {} bytes，节省 {:.2}%，{} ms [{}]",
                    r.input.display(),
                    r.output.display(),
                    r.input_bytes,
                    r.output_bytes,
                    r.saved_percent,
                    r.elapsed_ms,
                    r.optimization.as_ref().unwrap().engine
                );
                if let Some(resize) = &r.resize {
                    println!(
                        "缩放 {}×{} → {}×{}；编码无损校验通过",
                        resize.source_width, resize.source_height, resize.width, resize.height
                    );
                }
                let o = r.optimization.as_ref().unwrap();
                for note in &o.notes {
                    println!("{}：{}", r.input.display(), note);
                }
                if !o.search_complete {
                    eprintln!(
                        "{}：时间预算已用尽，保留已验证的最小候选",
                        r.input.display()
                    );
                }
                for warning in &o.warnings {
                    eprintln!("{}：{}", r.input.display(), warning);
                }
            } else {
                eprintln!(
                    "{}：{}",
                    r.input.display(),
                    r.error.as_deref().unwrap_or("失败")
                );
            }
        }
        println!("完成 {}，失败 {}", reports.len() - failed, failed);
    }
    Ok(failed == 0)
}

fn resizing(mut args: Resize) -> Result<bool> {
    let target = if let Some(target) = args.to {
        convert::canonical(&target)
    } else if let Some(extension) = args
        .encoding
        .output
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
    {
        convert::canonical(extension)
    } else {
        let budget = usize::try_from(args.encoding.memory_mib)?
            .checked_mul(1024 * 1024)
            .context("内存预算溢出")?;
        let bytes = files::read_bounded(&args.input, budget)?;
        if bytes.starts_with(png::SIGNATURE) {
            "PNG".into()
        } else {
            let info = convert::info_with_limits(
                &bytes,
                args.input.extension().and_then(|e| e.to_str()),
                budget,
                Duration::from_secs(args.encoding.time_limit),
            )?;
            convert::canonical(&info[0].format)
        }
    };
    if args.encoding.output.is_none() {
        let mut name = args.input.file_stem().context("无文件名")?.to_os_string();
        name.push(format!(".resized.{}", target.to_ascii_lowercase()));
        args.encoding.output = Some(args.input.with_file_name(name));
    }
    conversion(Convert {
        input: args.input,
        to: target,
        resize: Some(args.resize),
        encoding: args.encoding,
    })
}

fn conversion(command: Convert) -> Result<bool> {
    let Convert {
        input: input_path,
        to,
        resize,
        encoding: args,
    } = command;
    let target = convert::canonical(&to);
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| input_path.with_extension(target.to_ascii_lowercase()));
    if args.output.is_none()
        && input_path
            .extension()
            .is_some_and(|e| convert::canonical(&e.to_string_lossy()) == target)
    {
        anyhow::bail!("同格式转换必须显式指定不同的 --output");
    }
    files::validate_destination(&input_path, &output, false, args.overwrite)?;
    let budget = usize::try_from(args.memory_mib)?
        .checked_mul(1024 * 1024)
        .context("内存预算溢出")?;
    let input = files::read_bounded(&input_path, budget)?;
    let config = convert::ConvertConfig {
        source_hint: input_path
            .extension()
            .map(|e| e.to_string_lossy().into_owned()),
        target,
        resize,
        allow_lossy: args.allow_lossy,
        quality: args.quality,
        background: args.background,
        frame: args.frame,
        flatten: args.flatten,
        rasterize: args.rasterize,
        render_raw: args.render_raw,
        memory_bytes: budget,
        timeout: Duration::from_secs(args.time_limit),
    };
    let result = convert::convert(&input, &config)?;
    files::publish(&result.bytes, &output, args.overwrite)?;
    emit(
        &serde_json::json!({"input":input_path,"output":output,"input_bytes":input.len(),"output_bytes":result.bytes.len(),"conversion":result}),
        args.json,
    )?;
    Ok(true)
}
