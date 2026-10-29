# Kito 奇托初始化验证

日期：2026-10-28。用户确认采用 Kito 奇托与 Lumi 露米的伙伴设定，并要求用于项目初始化。

## 已落实

- Rust package、library crate、CLI binary 统一为 `kito`，版本仍为 0.1.0；Cargo.lock 已离线更新，依赖版本未升级。
- 总入口帮助展示「各有所长，一起把事情做好」，`image` 展示 Lumi 露米 · 图片伙伴（光影师）；功能子命令保持原样。
- README、AGENTS.md 和 `docs/brand.md` 记录伙伴理念、已有能力与未来概念；CI 产物为 `kito` / `kito.exe`。
- 后端路径按 `KITO_*` → 对应 `CTK_*` → 默认程序的顺序解析。显式空值不会回退，诊断指明实际配置来源。开发测试使用 `KITO_REQUIRE_BACKEND`。
- 新 benchmark 报告使用 schema 2，品牌字段为 `kito` / `kito_*`；3 份归档 JSON 保持 schema 1，历史语料中的文字字节保持不变，旧文档增加当前命名说明。

## 验证结果

本机 macOS arm64，真实 ImageMagick 7 与本地 ZopfliPNG。以下均通过：

- `scripts/cargo-local.sh fmt --check`
- `scripts/cargo-local.sh clippy --locked --offline --all-targets -- -D warnings`
- `KITO_REQUIRE_BACKEND=1 KITO_ZOPFLIPNG=<本地路径> scripts/cargo-local.sh test --locked --offline`
- `scripts/cargo-local.sh build --locked --offline --release`

测试共 66 个通过条目，包含此前 59 项和新增的 7 个配置测试条目（其中 1 个用于隔离环境变量的测试子进程）。新增配置测试先复现 5 项失败，再实现优先级与兼容行为后全部通过。

另用 release 二进制确认 `--version` 为 `kito 0.1.0`，顶层及 image 帮助页正确；真实执行 PNG 压缩、doctor 和 benchmark，验证新报告 schema 2、版本名与统计字段。零预算报告仍如实标记比较未完成。README 与品牌文档的本地链接有效。

未执行远程 CI，不能将本机结果描述为三平台均已验证。没有创建 commit、推送或发布。
