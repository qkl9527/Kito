# 图片工具 · Lumi 露米

[返回总入口](../../README.md) · [时间工具](time.md)

Lumi 露米 · 图片伙伴（光影师）负责静态 PNG 严格无损优化、等比缩放、静态图片格式转换、EXIF 读取、后端诊断和压缩基准，入口为 `kito image`。安装与构建方式见[总入口](../../README.md#构建与运行)。

PNG 优化内置 OxiPNG 和 Zopfli；缩放与格式转换使用 ImageMagick 7。工具会重新解码验证候选，默认保留原文件，只有显式 `--in-place` 才替换输入。

## 命令速查

| 命令 | 用途 |
| --- | --- |
| `kito image compress INPUT` | 优化静态 PNG，可组合 `--resize` |
| `kito image resize INPUT --resize SIZE` | 等比调整静态图片尺寸 |
| `kito image convert INPUT --to FORMAT` | 转换静态图片格式，可组合 `--resize` |
| `kito image info INPUT` | 查看图片信息 |
| `kito image exif INPUT` | 读取拍摄信息、相机、镜头、GPS 等 EXIF 标签，支持 `--json` |
| `kito image formats` | 查看后端读写能力及 CLI 输出范围 |
| `kito image doctor` | 检查可选后端及版本 |
| `kito image benchmark INPUT` | 比较 PNG 优化器并生成 JSON 报告 |

使用 `kito image --help` 查看各项能力的入门案例，使用 `kito image <子命令> --help` 查看对应命令的具体案例和完整参数，例如 `kito image convert --help`。`-h` 同样提供案例，均紧接在 `Usage` 后、参数列表前。涉及 `scripts/` 的示例在仓库根目录运行。

## 本文导航

- [后端与依赖](#后端与依赖)
- [PNG 无损压缩](#png-无损压缩)
- [调整图片尺寸](#调整图片尺寸)
- [格式转换](#格式转换)
- [读取 EXIF 信息](#读取-exif-信息)
- [图片信息与基准](#图片信息与基准)

## 后端与依赖

转换图片需要 [ImageMagick 7](https://imagemagick.org/script/download.php)，严格无损转换要求至少 Q16 精度：

```sh
# macOS
brew install imagemagick
# Windows（Chocolatey）
choco install imagemagick
```

Linux 安装发行版提供的 ImageMagick **7** 或按官方文档构建；只有 `convert` 的 ImageMagick 6 不满足要求。PNG 压缩和 EXIF 读取不依赖 ImageMagick。

可选安装独立 [ZopfliPNG](https://github.com/google/zopfli) 增加一个搜索引擎。使用 `KITO_MAGICK`、`KITO_ZOPFLIPNG` 指定可执行文件完整路径；值为程序路径，不包含额外参数。原有 `CTK_MAGICK`、`CTK_ZOPFLIPNG` 保留为后备配置，同类新旧变量同时设置时优先使用 `KITO_*`。用以下命令检查：

```sh
kito image doctor
kito image formats --json
```

`formats` 展示后端读写能力；`cli_output` 表示当前 CLI 接受的输出格式。一个格式能读写，并不代表它能保留所有输入的精度、动画或图层。

## PNG 无损压缩

```sh
# 默认 max，多策略择优，生成 icon.optimized.png
kito image compress icon.png

# 快速模式、指定输出
kito image compress icon.png --effort fast -o out.png

# 先缩小，再在新尺寸下压缩（缩放需要 ImageMagick）
kito image compress icon.png --resize 50% --effort fast -o small.png

# 递归批量，保留相对目录结构；总预算在并发任务之间均分
kito image compress ./images --recursive --output-dir ./optimized --jobs 4 --memory-mib 1024

# 只在明确指定时替换原文件
kito image compress icon.png --in-place

# 时间预算以秒计；输出结构化报告
kito image compress icon.png --time-limit 10 --json

# 可选清理文本和时间元数据，保留颜色相关信息
kito image compress icon.png --strip-metadata
```

- 保持尺寸、颜色、Alpha，包括透明像素的隐藏 RGB 和 16-bit 低位。不进行颜色量化或有损降位深。
- 默认保留元数据；无法确认复制安全的未知 chunk、签名或相关编码变换会使该候选被拒绝，必要时保持原文件。
- `--strip-metadata` 只清理 `tEXt/zTXt/iTXt/tIME`，清理节省量单独记录，不使用引擎的宽泛 `strip all`。
- `max` 搜索 OxiPNG 2/4/6、OxiPNG+Zopfli、适用时的两种无损调色板顺序，以及可用的独立 ZopfliPNG。每个候选必须通过独立像素和元数据检查。
- 没有更小且合格的结果时，输出原内容。`max` 表示本工具的有限搜索预设，不是全局最优证明。
- 已有输出默认报错；`--overwrite` 只允许覆盖输出，不能绕过原图保护。硬链接、符号链接以及批量输出冲突会被检查。
- 单文件失败不影响其他独立文件，最终存在失败则返回非零退出码。

以上“像素不变”描述针对未指定 `--resize` 的压缩。指定缩放时，先生成新的尺寸，再对该结果无损优化；即使放大后文件变大，也不会退回原尺寸。报告 `saved_bytes` 和 `saved_percent` 的负值表示文件增大，体积变化包含缩放带来的影响。批量 compress 同样支持 `--resize`。

默认内存预算为 512 MiB、并发为 1。预算包含文件大小与解码工作集预检，不是操作系统 RSS 硬上限。时间预算在内置优化阶段边界检查，单次 DEFLATE 调用不能立即抢占；预算耗尽时报告 `search_complete: false` 并保留已经验证的结果。外部进程有超时终止处理。

## 调整图片尺寸

三个入口共用 `--resize SIZE`，默认保持宽高比，**允许放大，不裁剪、不拉伸**：

| 参数 | 含义 |
| --- | --- |
| `1280x720` | 等比放入 1280×720 的尺寸框内 |
| `1280x` | 宽为 1280，高按比例计算 |
| `x720` | 高为 720，宽按比例计算 |
| `50%`、`12.5%` | 宽高按百分比缩放，百分比最多四位小数 |

例如 1600×1000 输入指定 `1280x720` 后为 1152×720。像素尺寸四舍五入，最小为 1；零尺寸、负数和其它 geometry 修饰符会被拒绝。

```sh
# 独立缩放，默认生成 input.resized.png
kito image resize input.png --resize 1280x720

# 同时缩放和无损编码成 WebP
kito image convert input.png --to webp --resize 50% --lossless -o small.webp
kito image resize input.png --resize x720 --to webp -o small.webp

# 由输出扩展名选择 JPEG，仍需明确允许有损编码
kito image resize input.png --resize 50% -o small.jpg --allow-lossy --quality 92
```

独立 resize 的格式优先级为 `--to`、输出文件扩展名、输入实际格式；未指定输出时生成 `文件名.resized.格式`。JPEG/AVIF/HEIC 等格式沿用 convert 的显式有损参数要求，透明图输出 JPEG 仍需 `--background`。

缩放使用 ImageMagick 7 的 Lanczos，8-bit/16-bit 输入分别使用相应精度的中间参考图。实际改变尺寸会重采样像素，因此 `verified_lossless: false`；`encoding_verified_lossless: true` 表示编码结果与缩放后的参考图像素一致。`--lossless` 在组合命令中约束的是编码阶段，报告仍明确列出缩放操作。缩放前后的色彩/profile/显示元数据继续核对；复杂扩展无法可靠保留时会拒绝发布，必要时可明确使用 `--allow-lossy`。

JSON 的 `resize` 字段包含请求值、输入/输出宽高、`changed` 和滤镜。目标尺寸与原尺寸相同时跳过重采样；PNG 的这一无操作路径仍可使用内置优化。目标尺寸在启动重采样前计入内存预算。完整动画缩放暂不支持，需使用 convert/resize 的 `--frame N` 显式选帧。

`convert`/`resize` 的 `--time-limit` 限制单次后端调用。`compress --resize` 在缩放前后检查并扣除已用时间，再将剩余预算交给压缩搜索；组合流程仍为软预算，不保证整条命令在指定秒数内立即退出。

## 格式转换

```sh
# 默认严格校验无损；显式 --lossless 可帮助阅读脚本
kito image convert input.png --to webp --lossless -o output.webp
kito image convert input.webp --to png -o output.png
kito image convert input.png --to tiff -o output.tiff

# JPEG 明确接受有损，透明输入还需要背景色
kito image convert input.png --to jpeg --allow-lossy --quality 92 --background white -o output.jpg

# 选帧／页，索引从 0 开始；动画帧先合成到画布
kito image convert animation.gif --to png --frame 2 -o frame.png

# 明确选择图层合成、矢量栅格化或 RAW 显影
kito image convert design.psd --to png --flatten -o preview.png
kito image convert icon.svg --to png --rasterize -o icon.png
kito image convert photo.nef --to tiff --render-raw -o photo.tiff
```

支持的静态输出包括 PNG、JPEG、WebP、BMP、TIFF、GIF、ICO、AVIF、HEIC/HEIF、JXL、TGA、QOI、PNM/PPM/PGM/PBM、EXR、HDR，实际可用项取决于 ImageMagick 的编解码器。更多格式可作为后端输入。SVG 输出、完整动画、多页和多尺寸输出尚未实现。APNG 选帧只有在后端完整识别动画结构时才允许；普通 PNG loader 仅返回默认图时会拒绝。

**质量边界：**

- JPEG、AVIF、HEIC/HEIF 首版要求显式 `--allow-lossy --quality`。不会将默认 YUV 编码宣称为像素无损。
- 无损转换回读目标，检查尺寸、RGBA16 像素、颜色空间、gamma、原色、方向、像素纵横比、渲染意图、文本和可验证的 profile；无法证明保真时拒绝发布。
- WebP 的无损路径启用 `exact`，防止丢失透明像素的 RGB；16-bit 输入不能在未授权的情况下截成 8-bit。
- `--background` 会移除透明度，因此需要 `--allow-lossy`。GIF 色数等格式限制也必须经过回读验证。
- `--allow-lossy` 允许颜色、精度和元数据变化，但仍检查输出尺寸与格式；报告 `verified_lossless: false`、`encoding_verified_lossless: false`。
- `--frame`、`--flatten`、`--rasterize`、`--render-raw` 会改变内容结构；“像素无损”只针对明确选取／渲染后的静态图。RAW 与矢量使用后端默认渲染设置。
- 浮点/HDR 输入（包括 16-bit floating-point TIFF）的无损验证暂不支持。压缩 PNG 与跨格式转换的元数据能力不同，复杂颜色扩展可能导致保守拒绝。

同格式转换必须明确指定其他输出文件。所有转换先在临时目录完成，验证成功后原子发布。

## 读取 EXIF 信息

```sh
# 可读文本，列出文件实际包含的 EXIF 标签
kito image exif photo.jpg

# 结构化输出，可重定向保存报告
kito image exif photo.jpg --json
kito image exif photo.jpg --json > photo.exif.json

# 大 TIFF 或包含较多元数据时可提高读取预算
kito image exif photo.tiff --memory-mib 1024
```

内置 Rust 解析器支持 **JPEG APP1、TIFF、PNG eXIf、WebP EXIF**，按文件内容识别，不依赖扩展名、ImageMagick 或 ExifTool，不修改原文件。只显示文件本身记录的字段，不补猜缺失的拍摄时间、时区或 GPS。没有 EXIF 或字段列表为空时正常退出，文本提示“未发现 EXIF 字段”；JSON 返回 `has_exif: false`、`fields: []`。

普通文本示例（实际标签随图片不同）：

```text
图片：photo.jpg
格式：JPEG
EXIF：2 个字段
[IFD0/Tiff] Make (0x010F)："Example Camera"
[IFD0/Exif] ExposureTime (0x829A)：1/125 s
```

JSON 顶层为 `input`、`format`、`has_exif` 和 `fields`。每个字段包含：

| 字段 | 含义 |
| --- | --- |
| `ifd` | 从 0 开始的 IFD 编号，区分主图与后续缩略图/页 |
| `context` | `Tiff`、`Exif`、`Gps` 或 `Interop` 标签组 |
| `tag` / `tag_id` | 英文标签名与数值编号；未知标签仍保留 |
| `type` / `value` | EXIF 类型和按该类型解析的值 |
| `display_value` | 带单位的可读值；GPS 方位来自对应参考标签 |
| `value_offset` | `Undefined` 数据在 TIFF 数据中的偏移，其他已知类型省略 |

`fields` 使用数组，保留不同 IFD 或重复出现的标签。`Rational` / `SRational` 的 `value` 是 `{ "numerator": 1, "denominator": 125 }` 这样的对象数组，避免把曝光和 GPS 分数转成有损浮点。整数类型为数字数组；`Ascii` 为去掉 NUL 分隔符后的字节数组列表，保留非 UTF-8 字节；`Undefined` 为字节数组。`Float` / `Double` 用字符串数组表示，包含 `NaN` / `inf` 时也不会变成 `null`。未知类型保留 `type_id`、`count`、`offset` 描述。这是标签解析结果，不是可直接写回的完整 EXIF 二进制备份。

当前不支持 HEIC/HEIF/AVIF、BigTIFF、独立 XMP/IPTC 的读取，也不展开厂商私有 MakerNote 内部结构。TIFF 返回基础 TIFF 标签与标准 EXIF 子目录。命令只提取元数据，**不解码验证图片像素完整性**；JPEG 读取首个 EXIF APP1，找到后不继续验证后续图像数据。PNG 会检查 chunk 结构与 CRC，WebP 会检查 RIFF 长度和 chunk 边界；损坏或不支持的元数据返回非零退出码，错误说明写入 stderr，不输出部分报告。

默认 `--memory-mib 512`，输入文件不超过预算的 1/8，累计字段数据量不超过预算的 1/256（默认 2 MiB），重复引用同一段数据仍累计计算；展示值在格式化过程中限制累计长度为预算的 1/16，防止关联单位反复展开。最多 4000 个目录条目、8 个顶层 IFD，重复或循环目录指针会报错。这些是防止解析时资源放大的保守限制，不是 RSS 硬上限；超限时明确报错，不静默截断字段。

## 图片信息与基准

```sh
kito image info input.png --json

# 所有引擎使用相同输入、元数据策略和无损校验
kito image benchmark ./corpus --output report.json

# 相同时间预算下比较；未完成的基线不算有效结果
kito image benchmark ./corpus --time-limit 10 --output report-budget.json

# 可复现的合成语料
python3 scripts/generate-corpus.py artifacts/corpus
kito image benchmark artifacts/corpus --output artifacts/benchmark.json

# 带固定版本、来源和许可记录的公开验证图片
python3 scripts/download-public-corpus.py artifacts/public-corpus
kito image benchmark artifacts/public-corpus --output artifacts/benchmark-public.json
```

报告包含文件 SHA-256、工具版本、实际参数档位、输入/输出字节、逐基线结果、胜平负、耗时和缺失/无效基线。峰值 RSS 当前未测量，值为 `null`。`comparison_complete: false` 意味着引擎缺失、失败或搜索预算不足，不能将该报告解读为击败全部基线。

当前优化属于“现有压缩器 + 额外策略搜索”。压缩收益取决于素材，需要分别观察单一基线和逐图最佳基线。项目不会预先承诺百分比，也不把合成图片的表现泛化到真实照片或所有 PNG。实际本机验证记录见 [首版验证报告](../validation/2026-10-28-image-tool.md) 和 [缩放验证报告](../validation/2026-10-28-image-resize.md)。

旧版 `ctk` 命令、环境变量和 benchmark 字段的迁移规则见 [迁移说明](../migration.md)。
