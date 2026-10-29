# 默认时区配置验证记录（2026-10-28）

## 实现范围

- `kito time timezone set ZONE` 持久保存时区；`get` 显示当前生效值和来源；`reset` 清除保存项。
- `now`、`from-timestamp`、不带 offset 的 `to-timestamp`/`convert` 源时间均可使用默认配置。
- `convert --to-timezone` 变为可选，省略时使用默认时区。
- 优先级：显式参数 > `KITO_TIMEZONE` > 保存配置 > UTC；已有输入 offset 的源时间含义保持不变。
- 配置使用用户应用目录中的 `kito/time.json`，支持 `KITO_CONFIG_DIR` 绝对目录覆盖；复用共享有界读取及原子发布逻辑。

## 验证环境及结果

本机 macOS arm64，使用仓库独立 Rust 工具链。所有配置测试均设置独立临时 `KITO_CONFIG_DIR`，未改动用户真实设置。未运行其他平台的远程 CI。

| 验证 | 结果 |
| --- | --- |
| `scripts/cargo-local.sh fmt --check` | 通过 |
| `scripts/cargo-local.sh clippy --locked --all-targets -- -D warnings` | 通过 |
| `scripts/cargo-local.sh test --locked` | 91 项通过，0 失败，0 ignored |
| `scripts/cargo-local.sh build --locked --release` | 通过，更新 `target/release/kito` |

新增 `tests/time_configuration.rs` 的 10 项真实 CLI 测试：

1. 跨进程读取保存值，各转换命令自动采用配置。
2. 显式参数和环境变量按优先级覆盖保存值，嵌入 offset 不受默认源时区影响。
3. 未配置时回退 UTC，get/reset 不创建缺失文件。
4. set/reset 保留其他字段，reset 后恢复默认行为。
5. 非法时区与非普通文件目标不破坏已有内容。
6. 坏 JSON、错误字段类型及无效时区明确报错；显式参数、环境变量及独立列表命令可绕过配置。
7. 无效环境覆盖不使已经成功的 set/reset 操作误报失败。
8. 相对 `KITO_CONFIG_DIR` 在写入前拒绝。
9. 嵌套超 u64 大整数及高精度小数原样保留。
10. 精简环境缺少用户目录时仍可只读使用 UTC；需要保存时提示指定 `KITO_CONFIG_DIR`。

## Review 及修复

独立 review 发现将整个配置读为 `serde_json::Value` 会使其他字段中的大整数或高精度小数失真。增加失败回归后，改用 `BTreeMap<String, Box<RawValue>>`，仅解析需要的 timezone 字段，其他值按原始 JSON 保存。只启用现有 serde_json 的 `raw_value` feature，无新增 crate。

另补充缺少 HOME/APPDATA 的回归，先确认失败，再区分只读 UTC 回退和写入时的配置目录要求。上述两个回归修复后，10 项配置测试、原有 15 项时间测试及完整 91 项测试全部通过。符号链接、超限文件拒绝以及原权限保留也经过独立 review 探测，未发现遗留问题。

## 命令示例

```sh
kito time timezone set Asia/Shanghai
kito time timezone get --json
kito time from-timestamp 0 --value-only
# 1970-01-01T08:00:00+08:00
kito time convert '1970-01-01T00:00:00Z' --value-only
# 1970-01-01T08:00:00+08:00
kito time timezone reset
```

reset 只清除文件中的 timezone 字段，不更改环境变量。配置同时包含其他字段时会保留；文件不存在时 reset 不创建文件。无环境覆盖且无保存值时，默认 UTC。
