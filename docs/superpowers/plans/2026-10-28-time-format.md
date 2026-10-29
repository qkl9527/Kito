# 自定义日期输出格式 Implementation Plan

**Goal:** 支持用户给出的空格分隔、带/不带时区等自定义日期时间输出。

**Architecture:** 新增 `src/time/format.rs` 封装经过验证的 Chrono strftime 模板；CLI 的四个转换命令共用 `--format/-f` 参数，核心 report 统一应用格式。

**Tech Stack:** 现有 chrono、clap、serde，无新增依赖。

**Spec:** 用户要求自定义输出格式，例如 `2024-01-01 08:00:00.123456+08:00` 和 `2024-01-01 08:00:00.123456`。

## 行为约定

- 支持 `now`、`from-timestamp`、`to-timestamp`、`convert`，可配合文本、单值、JSON。
- 格式采用 strftime：`%Y-%m-%d %H:%M:%S%.6f%:z` 与 `%Y-%m-%d %H:%M:%S%.6f` 实现用户示例。
- 未指定格式时保留现有默认输出。指定时调整 `datetime` 及日期类型的 `value`，新增 JSON `format` 字段记录模板。
- 时间戳类型的 `value`、`utc`、各精度 timestamps、时区元数据保持原有含义。`precision_lost` 仍只表示时间戳单位换算的取整。
- 自定义展示允许省略字段或降低小数位数，完整时刻以 UTC/时间戳字段为准；不影响输入解析、默认时区设置或日期范围检查。
- 拒绝空白模板、非法占位符及只能用于解析的格式，报错时不产生部分 stdout 或 panic。
- IANA `%Z` 使用当时的真实缩写，历史秒级 offset 可用 `%::z` 保留。
- 中文帮助、说明、核心注释；不 commit、推送或发布。

## 执行步骤

- [x] CLI 测试先覆盖用户示例、精度、JSON 保真、时间戳结果、DST、历史 offset、无效格式，确认失败。
- [x] 实现格式类型及参数/report 接入，运行相关测试。
- [x] 同步帮助与文档，完成针对本轮改动的 review。
- [x] 执行 fmt、严格 Clippy、完整测试和 release 构建，记录结果。
