# 图片工具 Implementation Plan

> 命名说明：本文记录采用暂名 `ctk` 时的设计或验证。项目现名为 Kito 奇托，当前命令入口为 `kito`；最新用法见 [README](../../../README.md)。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 交付 `ctk image` 的严格无损 PNG 优化、静态格式转换、能力诊断与可复现基准。

**Architecture:** Rust CLI 调用内置 PNG 多策略优化器，独立解码器验证候选。ImageMagick 处理其他格式转换，统一文件发布、资源约束和 JSON 报告。

**Tech Stack:** Rust stable、clap、oxipng、png、serde、tempfile、rayon、ImageMagick 7。

**Spec:** `docs/superpowers/specs/2026-10-28-image-tool-design.md`（用户已在 2026-10-28 同意直接实现）。

## Global Constraints

- macOS arm64／x86_64、Linux x86_64／arm64 glibc、Windows x86_64。
- 严格无损，保留 16-bit 和透明 RGB；不支持的动画、页、图层须显式拒绝或选择。
- 默认不覆盖；无合格收益返回原内容；不执行 git commit。
- 默认 max 搜索，512 MiB 工作集估算预算；`--jobs` 默认 1，时间预算以秒配置。
- 外部命令参数数组执行；后端可由 CTK_MAGICK、CTK_ZOPFLIPNG 指定。
- 使用本地 `.toolchain` 安装 Rust，不修改用户 shell 配置。

## Review Focus

- 全透明 RGB、16-bit 低位差异不能被误判无损（任务 1）。
- 元数据、未知 PNG chunk 与候选表示变更不能改变语义（任务 1）。
- 输出路径别名、硬链接、符号链接、目录重入不能破坏原图（任务 2）。
- 转换到不支持 Alpha／精度／多页的格式不能静默丢数据（任务 3）。
- 时间预算、缺失引擎及失败的基线不能生成虚假的比较结论（任务 4）。

## 文件结构

`src/main.rs` 为薄入口；`src/cli.rs` 为命令及调度；`src/image/png.rs` 为 PNG 结构与验证；`src/image/optimize.rs` 为搜索；`src/image/convert.rs` 为转换；`src/backend.rs` 为受控子进程；`src/files.rs` 为路径及发布；`src/image/benchmark.rs` 为基准。`tests/` 使用真实合成图片及 CLI，`scripts/` 提供语料和开发入口，`.github/workflows/` 运行跨平台检查。

### Task 1: PNG 无损内核与资源预检

**Files:** `Cargo.toml`, `src/image/{mod,png,optimize}.rs`, `tests/png_contract.rs`。
**Interfaces:** `PngImage::decode(bytes, budget) -> Result<PngImage>`；`equivalent(original, candidate, strip) -> Result<()>`；`optimize(bytes, &OptimizeConfig) -> Result<Optimized>`。

- [x] 写入严格无损测试：透明 RGB、16-bit、palette/tRNS、gamma/text、CRC、APNG、资源超限。
- [x] `cargo test --test png_contract` 观察尚未实现的行为失败。
- [x] 实现结构校验、RGBA16 规范化（8-bit 精确扩展）、元数据保留及多策略搜索。
- [x] 增加原图候选、ZopfliPNG 可选适配、调色板候选，报告候选来源和搜索状态。
- [x] 同一测试通过；输出大小 <= 输入且像素与要求保留的 chunk 一致。

### Task 2: CLI 与安全文件处理

**Files:** `src/{main,lib,cli,files,backend}.rs`, `tests/cli_contract.rs`。
**Interfaces:** `publish(bytes, path, overwrite) -> Result<()>`；`run_process(program, args, timeout) -> Result<ProcessOutput>`；`cli::run() -> Result<bool>`。

- [x] 写 CLI 真实调用测试：默认输出、已存在输出拒绝、in-place、中文路径、目录结构、部分失败。
- [x] `cargo test --test cli_contract` 确认失败后实现命令及文件预规划、原子发布。
- [x] 加入 JSON、jobs、预算、info、doctor，确认错误退出非零且原图不变。
- [x] 运行当前完整 `cargo test`，预期全部通过。

### Task 3: 广泛静态格式转换

**Files:** `src/image/convert.rs`, `tests/conversion.rs`。
**Interfaces:** `formats() -> Result<Vec<FormatCapability>>`；`convert(input, &ConvertConfig) -> Result<Converted>`。

- [x] 写 PNG→WebP 像素保真、JPEG 必须允许有损、透明转 JPEG 必须背景、多帧必须选择、精度和元数据测试。
- [x] `cargo test --test conversion` 先观察失败，再实现 ImageMagick 检查及编码。
- [x] 通过临时安全文件名输入后端，检查目标格式、帧数、像素和颜色信息；不满足无损时拒绝发布。
- [x] 动图、PSD、SVG、RAW 的选择／栅格化参数明确，formats 显示后端实际能力及 CLI 范围。
- [x] 本地有后端时完整测试；CI 安装后端后同样验证，缺失时专门测试错误路径。

### Task 4: 基准、文档与三平台构建配置

**Files:** `src/image/benchmark.rs`, `tests/benchmark.rs`, `scripts/generate-corpus.py`, `README.md`, `.github/workflows/ci.yml`。
**Interfaces:** `benchmark(paths, &OptimizeConfig) -> Result<BenchmarkReport>`，逐文件记录基线合法性、bytes、耗时、配置、预算。

- [x] 写基线缺失／无效及汇总真实性测试，先失败后实现。
- [x] 比较 original、OxiPNG max、OxiPNG max+Zopfli、可选 ZopfliPNG、ctk；同一验证器和元数据策略。
- [x] 生成明确标为合成的多类型语料，运行 release 基准；峰值内存无法可靠测量时写 null，不捏造数字。
- [x] 文档覆盖安装、命令、质量边界、后端依赖和性能结果；CI 覆盖五个目标平台的原生构建。
- [x] `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、release CLI 冒烟通过。
- [x] 独立代码审阅并修复重要问题；保留所有改动未提交。

## 执行记录

详见 `.superpowers/sdd/2026-10-28-image-tool/progress.md`。当前目录无 Git 历史，初始化 Git 后在原目录开发，不为了建立 worktree 创建用户未批准的初始提交。
