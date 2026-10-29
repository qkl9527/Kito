# 图片缩放验证记录

> 命名说明：本文记录采用暂名 `ctk` 时的设计或验证。项目现名为 Kito 奇托，当前命令入口为 `kito`；最新用法见 [README](../../README.md)。

日期：2026-10-28。环境：macOS arm64，Rust 1.99.1，ImageMagick 7.1.2-18 Q16-HDRI。改动未提交、未推送；其它平台等待 CI 实际运行。

## 完成的行为

- `ctk image resize INPUT --resize SIZE`，以及 compress/convert 的同名参数。
- `WxH` 框内等比、`Wx`、`xH`、最多四位小数的百分比；允许放大、整数安全计算、最小 1 像素。
- 图片原子发布及原图保护保持；compress 的批量和并发预算支持缩放；放大后的文件增长使用有符号 saved_bytes。
- 缩放改变像素时 `verified_lossless: false`；严格编码核验通过时 `encoding_verified_lossless: true`。报告包含原尺寸、目标尺寸和滤镜。
- `compress --resize` 的准备阶段仅生成并验证新尺寸 PNG，外层统一执行一次优化搜索，避免重复执行 OxiPNG。

## 自动验证

`scripts/cargo-local.sh fmt --check`、`clippy --locked --all-targets -- -D warnings`、`test --locked`、`build --locked --release` 均通过。完整测试设置 `CTK_REQUIRE_BACKEND=1`，并配置本地独立 ZopfliPNG。

共 **59 项测试**：已有 37 项、尺寸计算 9 项、缩放集成 13 项。尺寸模块先验证缺失实现失败，再通过全部几何测试；三种 CLI 入口的集成测试先复现缺少命令/参数，再实现。

新增覆盖包含：三种入口、真实格式推断、默认文件名、等比和百分比放大、100% 不重采样、非法/溢出尺寸、源与目标工作集预算、已有输出及原图别名保护、批量相对目录、JPEG 显式有损授权、RGBA16 低位与半透明通道、隐藏 RGB 与可见边缘。

## 发现并修复的边界

1. ImageMagick 正常 Alpha 加权重采样会把全透明输出的隐藏 RGB 置零。先按原精度量化到 MIFF，再仅为零 Alpha 像素回填未加权 RGB；正常可见边缘和 Alpha 不改变。混合透明边缘测试以普通 Alpha 加权结果作独立参考，并检查远离边缘的隐藏恒定颜色。
2. RGB 黑白图编码 PNG 时自动转为 Gray，会丢失 sRGB rendering intent。测试先复现失败，修复为在 RGB 缩放路径显式保留 RGB/RGBA 表示及原样本位深。
3. 默认推断非 PNG 格式时曾使用固定 512 MiB 探测预算。独立审阅复现后改用调用方预算与超时，真实后端 wrapper 测试确认首轮探测的内存参数及超时生效。

独立审阅及最终复核未发现剩余阻断项。时间预算仍是阶段/单次后端调用的软限制，不是整条命令的硬截止；README 已说明。

## Release 实图复核

同一张 1008×2244、1,399,813 bytes 的 PNG，`--resize 50%` 后均为 **504×1122**：

| 命令 | 输出 bytes | 本次耗时 | 编码验证 |
| --- | ---: | ---: | --- |
| compress，fast | 442,629 | 1,625 ms | 通过 |
| convert，WebP lossless | 305,776 | 1,642 ms | 通过 |

两个命令本次并行运行，耗时只作操作验证，不是严格性能基准。PNG 的 OxiPNG 候选改变要求保留的元数据，因此保留已验证的缩放 PNG，报告中的 `engine: original` 指本次压缩输入的缩放图；没有退回原图尺寸。这组体积变化同时包含缩小尺寸的收益，不能当作原尺寸无损压缩的提升。

独立 resize 也用全透明样例运行 release 二进制，验证输出尺寸和编码核验字段。私人图片及实际输出只保存在被忽略的 artifacts 目录，不作为公开语料。
