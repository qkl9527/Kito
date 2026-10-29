# 图片工具首版验证记录

> 命名说明：本文记录采用暂名 `ctk` 时的设计或验证。项目现名为 Kito 奇托，当前命令入口为 `kito`；最新用法见 [README](../../README.md)。

日期：2026-10-28。实现版本：`ctk 0.1.0`。改动保持未提交。

## 本机验证

- 环境：macOS arm64、Rust 1.99.1、ImageMagick 7.1.2-18 Q16-HDRI。
- PNG 引擎：OxiPNG 10.2.1（Cargo.lock 固定）、内置 Zopfli。
- 独立 ZopfliPNG：Google zopfli 源码提交 `df1517dd8518113a245e4d203a04b8a0e02ab493`，本地编译；报告保存二进制 SHA-256。
- `cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo test --locked`、`cargo build --locked --release` 通过。
- 完整测试开启 `CTK_REQUIRE_BACKEND=1`，使用真实 ImageMagick；独立 ZopfliPNG 路径也已配置。用户反馈修复后，37 项测试全部通过。

| 测试组 | 数量 | 主要检查 |
| --- | ---: | --- |
| PNG 合同 | 9 | 透明 RGB、16-bit 低位、palette/tRNS、低位深、元数据、CRC、结构错误、APNG 拒绝、资源预算 |
| CLI 和文件 | 5 | 默认保留原图、硬链接别名、显式 in-place、中文路径、批量部分失败、父级输出目录 |
| 格式转换 | 12 | WebP 往返、JPEG 有损授权、透明背景、16-bit 截断拒绝、GIF 选帧、BMP/ICO/TGA 往返、显示元数据与浮点检查、sBIT 冗余与非冗余声明 |
| 可选后端 | 8 | 默认缺失、显式错误／空配置、缺失基线、fast、非零退出、无输出、权限不足、有效候选 |
| 基准 | 3 | 汇总真实性、预算不完整状态、报告不能覆盖输入 |

额外逐格式调用验证了本机 PNG、WebP、BMP、TIFF、GIF、ICO、AVIF、HEIC、TGA、QOI、PPM 的静态输出路径。AVIF/HEIC 使用显式有损参数。JXL 编码器会补充 profile，严格验证保守拒绝，不能据此宣称其严格无损路径已通过。

## 压缩结果

10 张确定性合成图片由 `scripts/generate-corpus.py` 生成，包含 UI 色块、渐变、噪声、透明 RGB、棋盘格、纯色、16-bit、调色板、1-bit 灰度及 gamma/text。输入共 **38,272 bytes**，`ctk max` 输出共 **13,649 bytes**，减少 **64.34%**。

这个比例针对该合成集合及其原始编码，不代表真实素材上的平均效果。更有意义的是与同质量基线比较：

| 策略 | 总输出 bytes | ctk 对它的胜／平／负 |
| --- | ---: | --- |
| OxiPNG o6 | 13,659 | 5 / 5 / 0 |
| OxiPNG o6 + Zopfli | 13,650 | 1 / 9 / 0 |
| ZopfliPNG -m | 13,653 | 2 / 8 / 0 |
| 逐图选择以上基线的最小结果 | 13,649 | 0 / 10 / 0 |
| ctk max | 13,649 | — |

各策略均包含原文件保底，并经过相同的像素和元数据检查。`ctk` 比单个基线略小，但**与逐图最佳基线持平，尚未证明额外算法收益或全面优于现有工具**。本轮 ctk 搜索总耗时约 1.7 秒，明显高于仅运行 OxiPNG o6；具体逐图耗时见原始 JSON。

相同 1 秒／文件预算另跑一轮，输出仍为 13,649 bytes，搜索和三个基线均完成。它只说明这批小图在该预算下的结果，不代表大图片能在 1 秒内压缩完。

- [完整合成集合报告](2026-10-28-benchmark.json)
- [相同时间预算报告](2026-10-28-benchmark-budget.json)

## 公开样本

`scripts/download-public-corpus.py` 下载固定版本的 libpng/PngSuite 样本，并同时保存许可和来源 manifest。来源为 [libpng 提交 ac0c353](https://github.com/pnggroup/libpng/tree/ac0c3532476fa4876a83a279127840741cf216f2)，PngSuite 图片作者为 Willem van Schaik，目录 README 明确允许使用、复制、修改和分发。

本次包含 `pngtest.png`、`basn6a08.png`、`basn2c16.png`、`basn0g01.png`。覆盖隔行、RGBA、16-bit、低位深和复杂元数据。4 张输入共 9,481 bytes，输出共 9,401 bytes。

3 张常规样本合计 570 bytes，与逐图最佳基线持平。`pngtest.png` 含复杂显示扩展；各基线候选改变或无法证明保留其元数据，ctk 保留其原始 8,831 bytes。报告将不合格基线标记为无效，`comparison_complete: false`，没有把该样本纳入基线优劣比较。

- [公开样本报告](2026-10-28-benchmark-public.json)

## 独立审阅及修复

完成一次独立代码审阅后，以下问题均由失败测试复现，再修复并运行完整测试：

1. 16-bit 浮点 TIFF 的超范围高光被两边都量化，可能误报无损。现在识别 floating-point sample format，严格模式拒绝。
2. APNG 的独立默认图被误当动画第 0 帧。现在核对声明帧数、默认图是否属于动画及后端实际帧数；无法建立正确映射时拒绝。
3. pHYs 等显示信息丢失后仍报告无损。现在核对分辨率、单位、渲染意图、背景提示和文本。
4. 像素解码成功掩盖 PNG 后部非法结构。现在验证关键块、已知块长度／次序、IDAT 连续性，并完整消费解码器。
5. 输出目录为输入祖先时误剪枝。已修复，并增加该常见路径用法的测试。

没有保留待修复的审阅项。后续扩展应继续使用这些回归测试。

## 用户反馈回归

- PNG 的完整位深 `sBIT` 被通用扩展白名单误拦。已只放行与样本位深相同的冗余声明；indexed PNG 按 8-bit 调色板样本判断，Alpha 通道同样检查。非冗余和非法 sBIT 仍拒绝，且仍执行完整颜色、profile 和像素验证。新增测试先复现失败，再验证 RGB、RGBA、indexed、灰度、灰度 Alpha 和 16-bit 输入。
- 独立 ZopfliPNG 默认未安装时，原先显示容易误解为压缩失败的警告。现在区分 `spawn` 程序缺失与运行失败：前者记录提示和 `skipped: true`，后者保留警告；显式错误配置不能被静默跳过。缺失基线仍不计入有效比较。
- 本机实图为 1008×2244 RGB8 PNG，带 sBIT `[8, 8, 8]`、sRGB 和 eXIf：严格无损 WebP 输出 **855,932 bytes**，原图 **1,399,813 bytes**，缩小 **38.85%**，debug 实测约 **3.1 秒**、最终 release 复跑 **1.965 秒**，`verified_lossless: true`。原图未修改，私人图片和产物未加入验证语料。
- 同图 PNG `fast` 实测 **1,235,784 bytes / 1.15 秒**；用户报告 `max` 为 **1,070,381 bytes / 46.8 秒**。这组数据说明该素材的速度与体积取舍，不代表通用性能。

## 实现时的决定及边界

- 在用户指定的原目录初始化 Git 并开发，保持全部改动未提交；未创建需要初始提交的 worktree。审阅覆盖整个首版源码。
- 对不能证明无损的格式与元数据采取保守拒绝，代价是部分视觉等价的转换也需要显式允许有损。
- 内存预算是预检，内置压缩的超时不能抢占正在运行的 DEFLATE；不将其宣传为硬资源隔离。峰值 RSS 未测量，报告为 `null`。
- 只在本机 macOS arm64 执行了程序。五种原生平台的 CI 配置已提供，尚未推送或运行远程 CI，Linux、Windows 与 macOS x86_64 的实机结果仍待验证。
- 完整动画、多页输出、矢量输出尚未提供。PNG 的“优于所有开源工具”仍是未达成的性能目标。
