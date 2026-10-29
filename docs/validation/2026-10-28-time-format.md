# 自定义日期格式验证记录（2026-10-28）

## 实现范围

`now`、`from-timestamp`、`to-timestamp`、`convert` 增加 `--format/-f`，采用现有 Chrono 的 strftime 语法。支持文本、单值和 JSON 输出，未指定模板时保持默认格式。

自定义格式只改变 `datetime` 和日期类型的 `value`，JSON 额外记录 `format`；完整 UTC、各精度时间戳、时区元数据不变。时间戳类型的 `value` 不受影响，`precision_lost` 继续表示时间戳单位取整。输入解析和日期范围检查保持原有规则。

## 最终验证

本机 macOS arm64，仓库独立 Rust 工具链。测试配置均隔离到临时目录，未读写用户的真实默认时区配置。未执行其他平台的远程 CI。

| 检查 | 结果 |
| --- | --- |
| `scripts/cargo-local.sh fmt --check` | 通过 |
| `scripts/cargo-local.sh clippy --locked --all-targets -- -D warnings` | 通过 |
| `scripts/cargo-local.sh test --locked` | 最终完整运行 98 项通过，0 失败，0 ignored |
| `scripts/cargo-local.sh build --locked --release` | 通过，更新 `target/release/kito` |

新增 7 项真实 CLI 回归，时间转换测试总数增至 22，另有 10 项时区配置测试。先确认成功路径因缺少 `--format` 参数失败，再实现功能并验证通过。

覆盖用户要求的空格分隔、带/不带时区、中文与字面量百分号、小数位补零/截取、负值纳秒、JSON 完整精度、时间戳主结果不变、DST 缩写、历史秒级 offset、非法格式无 panic/部分输出，以及输入格式和日期边界不被输出模板绕过。

独立 review 未发现实质问题，额外探测了格式错误、Unix 秒、日期边界与时区相关行为。格式化使用 `write_to` 返回错误，避免 `.to_string()` 对不可用于输出的格式标记触发 panic。

## 首次完整运行出现的既有失败

`tests/resize.rs` 中 `inferred_non_png_format_uses_the_requested_memory_and_time_limits` 在第一次全套运行中失败：后端参数日志未得到期望的首项 `identify`，断言为 `None != Some("identify")`。

检查该测试和后端调用路径后，执行以下单项验证通过：

```sh
scripts/cargo-local.sh test --locked --test resize inferred_non_png_format_uses_the_requested_memory_and_time_limits -- --exact --nocapture
```

随后在不修改图片实现或该测试的情况下完整复跑，98 项全部通过。首次失败原因未稳定复现，不能宣称已定位或修复；首次输出保存在本地临时记录 `.superpowers/sdd/2026-10-28-time-format/full-tests-first-failure.log`。

## Release 输出示例

输入 `1704067200123456`、目标时区 `Asia/Shanghai`：

| `--format` | `--value-only` 结果 |
| --- | --- |
| `%Y-%m-%d %H:%M:%S%.6f%:z` | `2024-01-01 08:00:00.123456+08:00` |
| `%Y-%m-%d %H:%M:%S%.6f` | `2024-01-01 08:00:00.123456` |

两个示例均已使用本机 release 产物执行。若需要读取完整时刻，可使用 JSON 的 `utc` 或 `timestamps.ns`；自定义输出省略的信息不影响这些字段。
