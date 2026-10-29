# 从 ctk 迁移到 Kito

[返回总入口](../README.md) · [图片工具说明](tools/image.md)

- 主命令和构建产物改为 `kito` / `kito.exe`，脚本中的 `ctk image ...` 请改为 `kito image ...`。图片能力继续使用 `image` 功能名；本次不增加 `lumi` 别名。
- 后端路径配置推荐改用 `KITO_MAGICK`、`KITO_ZOPFLIPNG`。对应的 `CTK_MAGICK`、`CTK_ZOPFLIPNG` 仍可作为后备，新变量优先。
- 开发测试及 CI 使用 `KITO_REQUIRE_BACKEND=1`，替换原有的 `CTK_REQUIRE_BACKEND=1`。
- 新生成的 benchmark JSON 使用 `schema_version: 2`：工具名字段 `ctk` 改为 `kito`，所有 `ctk_*` 字段改为对应的 `kito_*`，例如 `versions.kito`、`files[].kito`、`files[].kito_elapsed_ms` 和 `totals.kito_bytes`。消费报告的脚本应先检查 schema 版本，再读取相应字段。
- `docs/validation/` 中已归档的 JSON 和报告文件名保留原样，历史记录中的 `ctk`、`CTK_*` 及 schema 1 字段反映当时的验证环境，不代表当前入口或配置名称。
