# Kito 奇托

**各有所长，一起把事情做好。**

Kito 是跨 macOS、Linux、Windows 的 Rust CLI 工具箱，命令名为 `kito`。每个工具领域由一位伙伴负责，命令按功能命名，角色名用于介绍和帮助信息。

把图片交给 Lumi，把时间换算交给 Tiko，让 Echo 做一段提示音。习惯命令行，就敲一条命令；喜欢看着操作，就运行 `kito web`，在浏览器里找伙伴。

[下载发布版](https://github.com/qkl9527/kito/releases) · [网页模式](#网页模式) · [Agent 集成](#让-agent-带上-kito) · [快速使用](#快速使用) · [底层项目与参考](#底层项目与参考)

![Kito CLI 实际运行示例：时区转换、Base64 编码与 SHA-256 内容指纹](docs/assets/cli-demo.png)

*上图来自实际 CLI 输出，仅添加展示外框与配色；命令可在下方示例中复制。*

## 工具与文档

| 工具 | 伙伴或领域 | 主要能力 | 详细说明 |
| --- | --- | --- | --- |
| `image` | Lumi 露米 · 光影师 | PNG 无损压缩、等比缩放、格式转换、EXIF 读取、信息与基准 | [图片工具](docs/tools/image.md) |
| `text` | 文本工具 | 字符统计、按行去重、大小写转换，支持字符串和文件 | [文本工具](docs/tools/text.md) |
| `time` | Tiko 提可 · 时刻向导 | 时间戳互转、时区转换、默认时区配置、自定义日期格式 | [时间工具](docs/tools/time.md) |
| `codec` | Codi 可迪 · 译码师 | Base64、URL 参数解析、HTML/XML 实体编解码 | [编解码工具](docs/tools/codec.md) |
| `id` | Sumi 苏米 · 印记师 | 六种 Hash 算法、UUID v4/v7 与批量生成 | [标识工具](docs/tools/id.md) |
| `key` | Kavi 卡维 · 钥匙匠 | OpenSSL RSA/Ed25519 密钥生成、公钥导出与配对检查 | [密钥工具](docs/tools/key.md) |
| `video` | Vido 维多 · 剪影师 | 视频剪辑与转换、截图、GIF，以及音乐转换、混音与响度处理 | [视频工具](docs/tools/video.md) |
| `sound` | Echo 艾可 · 音效师 | 一键音效、单音、旋律与和弦、鼓点、WAV/MIDI 创作 | [声音工具](docs/tools/sound.md) |

本页提供安装方法和入门示例。各工具的依赖、完整用法、配置、输出说明及能力边界见对应文档。

## 网页模式

不用记参数，也能让工具小队在浏览器里帮忙：

```sh
kito web
# 指定本机端口，仅打印地址、不自动打开浏览器
kito web --port 4174 --no-open
```

页面随 Kito 二进制提供，无需安装 Node.js 或启动独立前端。选择本机文件、填写参数后执行真实处理，再下载结果；默认保留原文件。服务只监听 `127.0.0.1`，每次启动使用独立会话，按 `Ctrl+C` 停止。首版提供八个领域的常用操作，完整范围、后端依赖与资源上限见[网页模式说明](docs/tools/web.md)。

![运行 kito web 后的真实首页：按功能选择工具小队伙伴](docs/assets/web-home.png)

从选参数到拿到结果：选择「声音创作 → 合成音效」，使用 `success` 预设，点击运行，就能预览并下载生成的 WAV。

<details>
<summary>看看实际操作：Echo 生成提示音，Lumi 处理 PNG</summary>

**Echo：合成完成后，试听并下载。**

![Echo 实际合成 success 音效后的页面，显示 0.90 秒 WAV、下载与音频预览控件](docs/assets/web-sound.png)

**Lumi：选择本机图片，快速优化，查看验证结果。**

![Lumi 实际处理 PNG 后的页面，显示原图预览、优化参数、像素无损验证和下载入口](docs/assets/web-image.png)

这张示例 PNG 已经很紧凑，本次保留原始编码，体积没有下降；压缩效果取决于输入。截图来自本机实际服务，素材与复现方式见[截图说明](docs/assets/README.md)。

</details>

## 安装发布版

tag 工作流发布成功后，可从 [GitHub Releases](https://github.com/qkl9527/kito/releases) 下载对应系统与架构的压缩包，无需安装 Rust；macOS / Linux 使用 `.tar.gz`，Windows 使用 `.zip`。

进入解压后的目录，在 macOS / Linux 运行 `./kito --help` 或 `./kito web`，Windows PowerShell 运行 `.\kito.exe --help` 或 `.\kito.exe web`。将可执行文件所在目录加入 `PATH` 后，即可直接使用 `kito`。

| 系统 | 架构 / 下载包中的 target |
| --- | --- |
| macOS（Apple Silicon） | `aarch64-apple-darwin` |
| macOS（Intel） | `x86_64-apple-darwin` |
| Linux（x86_64） | `x86_64-unknown-linux-gnu` |
| Linux（ARM64） | `aarch64-unknown-linux-gnu` |
| Windows（x86_64） | `x86_64-pc-windows-msvc` |

Release 同时提供 `SHA256SUMS`，可用于校验下载完整性。包内包含 Kito、网页资源及使用文档；FFmpeg、ImageMagick、OpenSSL 等外部后端仍需按需安装。Linux 包使用 Ubuntu 24.04 原生构建，系统兼容边界和校验命令见[发布说明](docs/releasing.md)。

## 构建与运行

安装 [Rust stable](https://rustup.rs/) 后：

```sh
cargo build --locked --release
./target/release/kito --help
# 安装到 Cargo 的 bin 目录，可在 PATH 中直接使用 kito
cargo install --locked --path .
```

Windows 的构建产物是 `target\release\kito.exe`。本工作目录也已安装独立 Rust 工具链，可用 `scripts/cargo-local.sh build --locked --release`；它不会修改全局 shell 配置。

时间、文本工具、PNG 压缩和 EXIF 读取可直接使用。图片缩放与格式转换需要 ImageMagick 7，安装方法见[图片工具的后端与依赖](docs/tools/image.md#后端与依赖)。

编解码与标识工具可离线直接使用。密钥工具需要 OpenSSL 3.x，可通过 `KITO_OPENSSL` 指定程序路径；`kito key doctor` 查看实际后端。macOS 系统自带的 LibreSSL 不作为该工具的 OpenSSL 后端，安装说明见[密钥工具](docs/tools/key.md)。

视频工具需要 FFmpeg 与 ffprobe，建议使用 FFmpeg 6 或更新版本的完整构建。可通过 `KITO_FFMPEG`、`KITO_FFPROBE` 指定程序路径，用 `kito video doctor` 检查后端路径与版本；所需编码器及安装配置见[视频工具](docs/tools/video.md#后端与依赖)。

## 让 Agent 带上 Kito

也可以让你的 agent 来认识工具小队：将下面这段 prompt 复制给能执行本机命令的 agent，让它通过 CLI 探索 Kito，并按自身规范生成可复用的 skill。

```text
请探索 Kito 奇托，把这支工具小队接入你的工作流程。
项目：https://github.com/qkl9527/kito

1. 从 kito --version 和 kito --help 开始，逐级查阅工具与子命令帮助，
   按需阅读本地 docs/tools/。若未安装，先说明适合当前环境的安装方式。
2. 整理「什么任务交给哪位伙伴、该用什么命令」。仅在命令支持时使用
   --json、--dry-run 或 doctor，不猜测参数或格式支持。
3. 在临时目录用自建样本验证少量操作，保留用户原文件与配置；涉及时间
   配置时，将 KITO_CONFIG_DIR 指向临时绝对目录。区分查阅帮助、后端
   检查、dry-run 与真实执行成功。
4. 若支持 skills，按当前 agent 的格式与目录约定生成或更新精简的 Kito
   skill，保留已有自定义内容；记录触发场景、能力路由、后端依赖与已验证
   示例，复杂参数引导按需查看 --help。否则输出一份可复用的调用指引。
5. 返回 skill 路径或调用指引，以及 Kito 版本、验证结果与缺少的后端。
   不将私钥、口令或会话令牌写入 skill，不把未运行的能力写成已验证。
```

## 快速使用

### 图片工具

```sh
# 压缩 PNG，默认保留原文件
kito image compress icon.png

# 等比缩放，允许放大
kito image resize photo.png --resize 1280x720

# 转换为 WebP，默认要求像素无损
kito image convert input.png --to webp -o output.webp

# 查看拍摄信息；加 --json 便于脚本读取
kito image exif photo.jpg
```

详细参数、批量处理、格式限制、无损校验和 JSON 报告见[图片工具说明](docs/tools/image.md)。

### 时间工具

```sh
# 查询并保存默认时区
kito time zones Shanghai
kito time timezone set Asia/Shanghai

# 微秒时间戳转日期时间，使用自定义格式，只输出结果
kito time from-timestamp 1704067200123456 --unit us -f '%F %T%.6f' --value-only
# 2024-01-01 08:00:00.123456

# 将带 offset 的时间转换到指定时区
kito time convert '2024-01-01T00:00:00Z' --to-timezone Asia/Shanghai --value-only
```

默认时区优先级为 **显式参数 > `KITO_TIMEZONE` > 保存配置 > UTC**。更多双向转换示例、时区列表、格式占位符、配置路径和 JSON 字段见[时间工具说明](docs/tools/time.md)。

### 文本工具

```sh
# 默认按 Unicode 字符统计，中文每字计 1
kito text count '你好，Kito'

# 统计文件中的 UTF-8 字节数
kito text count --file notes.txt --bytes

# 按行去重，默认输出到终端；也可通过 -o 写入文件
kito text dedupe --file lines.txt -o unique.txt

# 转为大写；也支持 lower 和 capitalize
kito text case upper 'Hello, world!'

# 所有单词首字母大写，其余转小写
kito text case capitalize --words 'hello WORLD from KITO'
```

更多说明见[文本工具说明](docs/tools/text.md)。

### 编解码与标识

```sh
kito codec base64 encode '你好，Kito'
kito codec url parse 'https://example.com/?tag=a&tag=b'
kito codec html encode '你好 & Kito' --style hex --all
kito id hash 'Kito' --algorithm sha256
kito id hash --file archive.zip --algorithm sha256
kito id uuid --version v7 --count 10
```

Base64 支持标准与 URL-safe；URL 解析完整展示参数，保留重复次序与空值，默认末尾附可复制的 Query JSON 对象（重复同名值合并为数组）；HTML 支持 HTML5/XML 实体与命名、十进制、十六进制表示。完整边界见[编解码工具](docs/tools/codec.md)，Hash/UUID 用法见[标识工具](docs/tools/id.md)。

### 密钥工具

```sh
# RSA 默认 3072 位；口令从已有文件读取，保护 PKCS#8 私钥
kito key generate --out-dir ./keys/app --passphrase-file ./passphrase.txt

# Ed25519 用于签名；明确选择保存未加密私钥
kito key generate --algorithm ed25519 --out-dir ./keys/signing --no-passphrase
kito key check ./keys/signing/private.pem --public ./keys/signing/public.pem --json
```

密钥目录必须不存在；成功后包含 `private.pem` 和 `public.pem`。Ed25519 生成固定参数密钥，不接受 RSA 的 `--bits`。口令 stdin、公钥导出及存储说明见[密钥工具](docs/tools/key.md)。

各层级的 `-h` / `--help` 都按「功能说明 → Usage → 使用案例 → 子命令与参数」展示；案例包含具体参数及其用途，可复制后替换文件路径或输入值。

```sh
kito --help
kito image --help
kito image convert --help
kito codec base64 encode --help
kito time timezone set -h
```

### 视频工具

```sh
# 查看容器、轨道、尺寸与时长
kito video info clip.mov

# 转为 MP4；默认生成 clip.convert.mp4，保留输入
kito video convert clip.mov --to mp4

# 截取从第 10 秒开始、持续 5 秒的片段
kito video trim clip.mp4 --start 10 --duration 5 -o highlight.mp4

# 等比缩放与按顺序拼接
kito video resize clip.mp4 --size 1280x720
kito video concat opening.mp4 main.mp4 ending.mp4 -o joined.mp4

# 生成 GIF；--dry-run 可先查看操作参数
kito video gif clip.mp4 --start 2 --duration 3 --width 480 --fps 12

# 音乐格式转换，以及人声和背景音乐混音
kito video audio convert song.wav --to flac
kito video audio mix voice.wav music.wav --to wav -o mixed.wav

# 音频淡入淡出与单遍响度处理
kito video audio fade song.wav --fade-in 2 --fade-out 3 --to wav
kito video audio normalize podcast.wav --loudness -16 --to wav
```

Vido 对视频重新编码，质量会发生变化，压缩结果不保证更小。常规视频操作使用第一条真实视频轨与第一条音轨，音乐处理使用第一条音轨；字幕、附件和原有元数据不保留。音频统一为 48 kHz 立体声，WAV/FLAC 不能恢复有损音源已经丢失的信息。完整参数、质量档位、音乐处理和安全输出规则见[视频工具说明](docs/tools/video.md)。

### 声音创作工具

Echo 使用内置 Rust 合成器，无需 Node.js、浏览器、声卡或 FFmpeg。它从零生成新声音；现有素材的音频转码、裁剪和音轨编辑由 `video audio` 处理。

```sh
# 查看 8 种音效、5 种音色和 3 种鼓型
kito sound presets

# 一键生成成功提示声，默认 success.wav
kito sound sfx success

# 音名写旋律；: 后是拍数，+ 是和弦，R 是休止
kito sound melody 'C4 E4 G4:2 R:1 C4+E4+G4:2' --instrument bell -o theme.wav

# 导出可继续编曲的 MIDI；音色在编曲软件中选择
kito sound melody 'C4 D4 E4 G4:2' -o theme.mid

# 四小节鼓点；也可用 x 和 - 自定义 16 格节奏
kito sound beat house --bpm 120 --bars 4
```

默认不覆盖已有文件；`--dry-run` 先校验，`--json` 输出报告。完整参数、音乐零基础教程、文件输入、脚本示例和常见问题见[声音工具详尽说明](docs/tools/sound.md)。

## 底层项目与参考

Kito 把成熟项目的能力整理成顺手的命令，也借鉴了开源社区的交互与教学方式。感谢这些项目；以下列出主要依赖与参考，完整 Rust 依赖见 [Cargo.toml](Cargo.toml) 与 [Cargo.lock](Cargo.lock)。

| 类型 | 项目 | 在 Kito 中的用途 |
| --- | --- | --- |
| 按需安装的外部后端 | [FFmpeg / ffprobe](https://ffmpeg.org/) | 视频、音频处理，媒体探测和输出解码验证。 |
| 按需安装的外部后端 | [ImageMagick 7](https://imagemagick.org/)、[OpenSSL 3.x](https://www.openssl.org/) | 分别用于图片缩放与转换、RSA / Ed25519 密钥生成与检查。 |
| 可选外部后端 | [ZopfliPNG](https://github.com/google/zopfli) | 为 PNG 最大压缩增加候选；未安装时仍可使用内置优化。 |
| 内置 Rust 库 | [OxiPNG](https://github.com/oxipng/oxipng)、[zopfli-rs](https://github.com/zopfli-rs/zopfli)、[png](https://github.com/image-rs/image-png)、[kamadak-exif](https://github.com/kamadak/exif-rs) | PNG 优化、编解码、候选验证与 EXIF 读取。 |
| 内置库与时区数据 | [Chrono](https://github.com/chronotope/chrono)、[chrono-tz](https://github.com/chronotope/chrono-tz)、[IANA tzdb](https://www.iana.org/time-zones) | 日期时间、时区转换与全球时区目录。 |
| 内置 Rust 库 | [RustCrypto hashes](https://github.com/RustCrypto/hashes)、[uuid](https://github.com/uuid-rs/uuid)、[getrandom](https://github.com/rust-random/getrandom) | 流式 Hash、UUID 表示与系统安全随机源。 |
| 内置 Rust 基础库 | [clap](https://github.com/clap-rs/clap)、[Serde](https://serde.rs/)、[Axum](https://github.com/tokio-rs/axum)、[Tokio](https://tokio.rs/) | CLI 参数、结构化数据和本机网页服务。 |
| 编解码标准参考 | [RFC 4648](https://www.rfc-editor.org/rfc/rfc4648)、[WHATWG HTML 实体表](https://html.spec.whatwg.org/entities.json)、[XML 1.0](https://www.w3.org/TR/xml/) | Base64 编码规则、HTML 命名实体数据与 XML 字符规则。 |
| 声音设计参考 | [Tone.js](https://tonejs.github.io/)、[jsfxr](https://github.com/chr15m/jsfxr)、[Bfxr](https://www.bfxr.net/v1/)、[Sonic Pi](https://sonic-pi.net/tutorial-02.html) | 借鉴音名、节拍、包络、预设与教学方式；合成器由 Kito 内置 Rust 实现，运行时无需这些项目。 |

## 开发与验证

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

使用仓库独立 Rust 工具链时，将上述 `cargo` 替换为 `scripts/cargo-local.sh`。有 ImageMagick 时转换集成测试会调用真实后端；未安装时跳过依赖后端的测试，仍检查缺失后端错误。设置 `KITO_REQUIRE_BACKEND=1` 会要求后端必须存在，CI 使用该设置。

CI 配置覆盖 macOS arm64/x86_64、Linux arm64/x86_64、Windows x86_64 的原生构建与测试。本地 macOS 通过不代表远程 CI 已通过。

## tag 自动发布

将版本更新到 `Cargo.toml` 和 `Cargo.lock` 后，推送对应的 `v<版本号>` tag（如 `v0.1.0`），会触发 [Release 工作流](.github/workflows/release.yml)：校验版本、构建五个平台、打包并生成 SHA-256 校验和，全部成功后发布 GitHub Release。带预发布标识的 tag（如 `v0.2.0-rc.1`）会标记为 prerelease。

具体命令、权限、失败重试与发布边界见[发布指南](docs/releasing.md)。只提交工作流文件不会创建 Release，推送 tag 才会触发发布。

## 代码结构

`src/image/`、`src/text.rs`、`src/time/`、`src/codec/`、`src/id/`、`src/key/`、`src/video/` 与 `src/sound/` 分别维护各领域业务。`src/files.rs` 负责安全输出，`src/io_args.rs` 提供新伙伴的字节输入输出；视频因随机访问需要仅接收本地普通文件。图片子进程由 `src/backend.rs` 管理，密钥口令管道及存储权限由 `src/key/` 单独管理。新增工具应使用独立模块，并在 `docs/tools/` 添加对应说明、在本页登记入口。

## 其他文档

- [Kito 伙伴约定](docs/brand.md)：品牌理念、伙伴职责与扩展原则。
- [从 ctk 迁移到 Kito](docs/migration.md)：命令、环境变量与 benchmark schema 的迁移规则。
- [发布指南](docs/releasing.md)：tag、平台产物、校验和与发布工作流。
- [验证记录](docs/validation/)：各功能的验证环境、结果与已知限制。

## 许可证

Kito 采用 [MIT License](LICENSE)。第三方依赖与引用内容保留各自的许可证。
