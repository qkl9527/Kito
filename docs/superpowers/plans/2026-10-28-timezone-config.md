# 默认时区持久化 Implementation Plan

**Goal:** 增加命令保存、查看、清除默认时区，未传时区参数的时间转换自动使用保存值。

**Architecture:** `src/time/config.rs` 管理时区配置和默认值优先级，复用 `src/files.rs` 的有界读取、原子发布。`src/time/cli.rs` 注册 `timezone set/get/reset`；`convert --to-timezone` 改为可选。

**Tech Stack:** 现有 Rust、clap、serde_json、tempfile，无新增依赖。

**Spec:** 用户本轮要求增加 command 配置默认时区，缺省参数使用默认时区。

## 接口与边界

- `kito time timezone set ZONE`、`get`、`reset`；继续支持 `--value-only` / `--json`。
- 显式参数 > `KITO_TIMEZONE` > 保存的配置 > UTC；输入已带 offset 的源时间规则保持不变。
- 配置位置：macOS `~/Library/Application Support/kito/time.json`；Linux `${XDG_CONFIG_HOME:-~/.config}/kito/time.json`；Windows `%APPDATA%\kito\time.json`。
- `KITO_CONFIG_DIR` 可指定绝对目录，用于独立配置和测试。只在需要读取默认值时访问配置文件。
- 保存为 JSON 对象的 `timezone` 字段；更新/清除该字段保留其他字段；先验证，再同目录临时文件原子替换。
- `get` 返回当前生效时区及来源；`set` 返回保存值；`reset` 清除保存项，单值返回 `reset`，不改变环境变量。
- 无配置只读回退 UTC，不创建文件；坏 JSON/无效时区报错，显式参数和环境变量可覆盖。无效写入和文件异常不能破坏旧配置。
- 中文帮助/说明/核心注释；不 commit、推送、发布。不自动替用户选择并保存某个具体时区。

## 步骤

- [x] 增加真实 CLI 回归，覆盖跨进程持久化、各命令默认值、优先级、reset、无效输入及已有字段保护；先观察缺少命令的失败。
- [x] 实现配置存储、默认值读取和 CLI，隔离现有时间测试的配置目录。
- [x] 同步 README、品牌约定、AGENTS；记录验证结果。
- [x] 执行 fmt、Clippy、完整测试和 release 构建，检查改动及失败路径。
