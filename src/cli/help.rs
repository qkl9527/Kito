//! 统一帮助布局：先给可复制的案例，再列出 clap 生成的子命令和完整参数。

use clap::{Command, CommandFactory, FromArgMatches};

mod video;

const TEMPLATE: &str =
    "{before-help}{about-with-newline}\n{usage-heading} {usage}{after-help}\n\n{all-args}";

/// 递归设置显式命令的布局，避免深层命令退回“案例排在所有参数之后”的默认格式。
/// 案例由各领域的 after_help 维护；参数解析、互斥规则和错误退出仍交给 clap。
fn with_layout(command: Command) -> Command {
    command.help_template(TEMPLATE).mut_subcommands(with_layout)
}

pub(super) fn parse() -> super::Cli {
    let command = super::Cli::command().mut_subcommand("video", video::with_examples);
    let matches = with_layout(command).get_matches();
    super::Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
}

pub(super) const ROOT: &str = r#"使用案例:
  网页模式：在本机浏览器中使用工具小队
    kito web
  Lumi：无损优化 PNG，保留原文件
    kito image compress icon.png
  Tiko：将毫秒时间戳转为上海时间
    kito time from-timestamp 1704067200123 --unit ms --timezone Asia/Shanghai
  Codi：解析 URL，并显示可复制的 Query JSON
    kito codec url parse "https://example.com/?tag=a&tag=b"
  Sumi：生成 3 个 UUID v7
    kito id uuid --version v7 --count 3
  Kavi：检查 OpenSSL 3 后端和密钥算法能力
    kito key doctor
  Echo：一键生成成功提示音，无需外部后端
    kito sound sfx success
  Vido：检查 FFmpeg / ffprobe 后端和视频编码能力
    kito video doctor
  Echo：查看声音创作的音效、乐器和鼓型预设
    kito sound presets
  文本工具：将文本转为大写
    kito text case upper "Hello, Kito"
  查看任意层级的具体案例和参数
    kito codec base64 encode --help"#;

pub(super) const TEXT: &str = r#"使用案例:
  count：按 Unicode 字符计数；加 --bytes 改为 UTF-8 字节数
    kito text count "你好，Kito"
  dedupe：按行去重，保留首次出现的顺序，保存到新文件
    kito text dedupe --file lines.txt -o unique.txt
  case：每个单词首字母大写，其余字母小写
    kito text case capitalize "hello WORLD" --words"#;

pub(super) const COUNT: &str = r#"使用案例:
  统计字符串的 Unicode 字符数，结果为 7
    kito text count "你好，Kito"
  统计同一字符串的 UTF-8 字节数，结果为 13
    kito text count "你好，Kito" --bytes
  统计文件；--file 与位置参数文本二选一
    kito text count --file notes.txt"#;

pub(super) const DEDUPE: &str = r#"使用案例:
  按行去重，结果写到终端；保留第一次出现的行及顺序
    kito text dedupe --file lines.txt
  保存到新文件，默认不覆盖已有文件
    kito text dedupe --file lines.txt -o unique.txt
  明确允许替换已有结果；输入文件仍受保护
    kito text dedupe --file lines.txt -o unique.txt --overwrite"#;

pub(super) const CASE: &str = r#"使用案例:
  upper：所有字母转为大写
    kito text case upper "Hello, Kito"
  lower：所有字母转为小写
    kito text case lower "Hello, KITO"
  capitalize：只转换首字符，得到 Hello WORLD
    kito text case capitalize "hello WORLD"
  capitalize --words：逐词处理，得到 Hello World
    kito text case capitalize "hello WORLD" --words
  从文件读取并保存；--overwrite 才允许覆盖已有结果
    kito text case upper --file notes.txt -o upper.txt"#;

pub(super) const IMAGE: &str = r#"使用案例:
  compress：优化静态 PNG，默认保留原文件
    kito image compress icon.png --effort fast
  resize：等比放入 1280×720 尺寸框，允许放大
    kito image resize photo.png --resize 1280x720
  convert：无损编码为 WebP，保存到指定文件
    kito image convert icon.png --to webp --lossless -o icon.webp
  info：查看图片格式、尺寸等信息
    kito image info photo.png --json
  exif：读取相机、拍摄时间等已有标签，无需 ImageMagick
    kito image exif photo.jpg
  formats：查看本机后端支持的格式与 CLI 输出范围
    kito image formats
  doctor：检查可选后端及版本
    kito image doctor
  benchmark：比较 PNG 优化器，保存 JSON 报告
    kito image benchmark ./images -o benchmark.json"#;

pub(super) const COMPRESS: &str = r#"使用案例:
  默认 max 多策略搜索，生成 icon.optimized.png
    kito image compress icon.png
  fast 快速模式，指定新输出文件
    kito image compress icon.png --effort fast -o optimized.png
  先缩放到 50%，再无损压缩新尺寸；缩放需要 ImageMagick
    kito image compress icon.png --resize 50% -o small.png
  递归处理目录，4 个任务共享 1024 MiB 预算，输出 JSON 报告
    kito image compress ./images --recursive --output-dir ./optimized --jobs 4 --memory-mib 1024 --json
  明确替换原图；只移除文本/时间元数据，搜索软预算为 10 秒
    kito image compress icon.png --in-place --strip-metadata --time-limit 10"#;

pub(super) const CONVERT: &str = r#"使用案例:
  转为 WebP，验证像素无损；需要 ImageMagick 7
    kito image convert icon.png --to webp --lossless -o icon.webp
  接受有损 JPEG 编码；quality 为 1–100，white 用于移除透明度
    kito image convert icon.png --to jpeg --allow-lossy --quality 92 --background white -o icon.jpg
  先等比缩小，再编码为 PNG
    kito image convert photo.webp --to png --resize 50% -o small.png
  从动画选择第 3 帧；--frame 从 0 开始
    kito image convert animation.gif --to png --frame 2 -o frame.png
  显式允许矢量栅格化；PSD 合成用 --flatten，RAW 显影用 --render-raw
    kito image convert icon.svg --to png --rasterize -o rendered.png"#;

pub(super) const RESIZE: &str = r#"使用案例:
  等比放入尺寸框，不拉伸、不裁剪；允许放大
    kito image resize photo.png --resize 1280x720
  固定宽 1280，高按比例计算；固定高可用 x720
    kito image resize photo.png --resize 1280x -o wide.png
  宽高均缩小到 50%，同时编码为 WebP
    kito image resize photo.png --resize 50% --to webp -o small.webp
  从输出扩展名选择 JPEG；透明输入用 white 背景，有损质量为 92
    kito image resize icon.png --resize x720 -o small.jpg --allow-lossy --quality 92 --background white"#;

pub(super) const FORMATS: &str = r#"使用案例:
  查看实际后端读写能力；cli_output 标记 CLI 接受的输出格式
    kito image formats
  输出紧凑 JSON，便于脚本读取
    kito image formats --json"#;

pub(super) const INFO: &str = r#"使用案例:
  查看 PNG 结构和尺寸等信息
    kito image info icon.png
  通过 ImageMagick 查看其他图片格式，输出紧凑 JSON
    kito image info photo.jpg --json"#;

pub(super) const EXIF: &str = r#"使用案例:
  读取 JPEG 中已有的拍摄标签，不修改图片
    kito image exif photo.jpg
  输出原类型、精确分数和展示值；也支持 TIFF、PNG、WebP 的 EXIF
    kito image exif photo.tiff --json
  大文件或较多字段时提高解析内存预算（MiB）
    kito image exif photo.tiff --memory-mib 1024"#;

pub(super) const DOCTOR: &str = r#"使用案例:
  查看 ImageMagick、ZopfliPNG 等可选后端及版本
    kito image doctor
  输出紧凑 JSON，便于脚本检查后端状态
    kito image doctor --json"#;

pub(super) const BENCHMARK: &str = r#"使用案例:
  对一张 PNG 比较各优化器，默认将 JSON 报告写到终端
    kito image benchmark icon.png
  递归处理目录中的 PNG，保存到新报告文件
    kito image benchmark ./images -o benchmark.json
  每张图的各基线与 Kito 优化分别使用 10 秒软预算，内存预算为 1024 MiB
    kito image benchmark ./images --time-limit 10 --memory-mib 1024 -o budget.json
  所有引擎使用相同元数据策略；明确允许覆盖旧报告
    kito image benchmark ./images --strip-metadata -o benchmark.json --overwrite"#;
