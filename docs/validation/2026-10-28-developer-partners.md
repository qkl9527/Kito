# 编解码、标识与密钥伙伴验证记录

日期：2026-10-28。

本次实现 Codi 可迪（`codec`）、Sumi 苏米（`id`）、Kavi 卡维（`key`），密钥包括用户要求的 Ed25519。详细接口见[编解码工具](../tools/codec.md)、[标识工具](../tools/id.md)、[密钥工具](../tools/key.md)。

## 环境

- 本机 macOS 26.4.1，arm64。
- 项目本地 Cargo 1.99.1。
- OpenSSL 3.6.3，通过测试命令的 `KITO_OPENSSL=/opt/homebrew/opt/openssl@3/bin/openssl` 明确选择。
- ImageMagick 7.1.2-18 Q16-HDRI，用于现有图片功能回归。
- 系统默认 `/usr/bin/openssl` 为 LibreSSL；本轮未修改 PATH 或用户级配置。

## 检查结果

| 检查 | 实际结果 |
| --- | --- |
| `scripts/cargo-local.sh fmt --check` | 通过 |
| `scripts/cargo-local.sh clippy --locked --offline --all-targets -- -D warnings` | 通过 |
| `KITO_OPENSSL=… KITO_REQUIRE_BACKEND=1 scripts/cargo-local.sh test --locked --offline` | 163 项通过，0 失败；真实图片和密钥后端强制启用 |
| `scripts/cargo-local.sh build --locked --offline --release` | 通过，生成 `target/release/kito` |
| 新增 key 模块的 Windows GNU `cargo check` | 通过；独立检查工程直接引用实际 `src/key/mod.rs`，包含 Windows ACL 分支 |
| 完整项目 Windows GNU `cargo check --all-targets` | 未完成：既有图片 C 依赖需要本机未安装的 `x86_64-w64-mingw32-gcc` |

Windows 编译检查不能证明 ACL 的运行效果。Linux/Windows 原生运行测试尚未在本次本机工作中执行；已补充 CI 的 OpenSSL 3 配置，不宣称远程 CI 已通过。

## 关键验证内容

- Base64 标准与 URL-safe 向量、二进制 stdin/文件回环、不额外加换行、规范 padding、非零尾部位、混用字母表和非法空白拒绝。
- URL 重复参数顺序、空键/空值、`flag` 与 `flag=`、form/percent 的 `+` 区别、先切分后解码一次、非法 `%HH`/UTF-8 拒绝、未找到 key 不发布空文件。
- HTML5 完整 2125 个命名实体，其中 93 个包含多个码点；全部实体解码/回环。XML 1.0 字符范围、严格分号、未知实体、十六进制引用大小写及属性空白规则。
- Hash 六算法已知摘要、64 KiB 分块读取、二进制与读取失败；UUID 版本/variant、v7 毫秒位布局、表示形式、数量上限、随机失败与异常时钟处理。
- OpenSSL 真实 RSA/Ed25519 生成、加密和未加密 PKCS#8、公钥导出、SPKI DER 配对、不匹配/错误口令拒绝、文件权限、已有文件/目录/符号链接保护、嵌套目录、子进程超时。
- 解析实际加密密钥的 ASN.1，确认 PBES2、PBKDF2-HMAC-SHA256、600000 次迭代与 AES-256-CBC。
- 共享输入按原始字节读取，不裁剪 BOM/换行，限制输入尺寸；输出拒绝覆盖输入与符号链接。

## 审阅中发现并修复的问题

1. 初选的实体库截断多码点实体。已替换为 WHATWG 官方 JSON 生成的完整表，保留生成器、输入 SHA-256 和许可。官方表 SHA-256：`d741d877ac77c4194c4ad526b5b4a19aef8dfe411ab840a466891cdbb9f362e6`。
2. OpenSSL 的 stdin 口令读取会截断 1024 字节输入。现在明确最多 1023 个 UTF-8 字节，超长即报错；真实回归确认最大长度的最后一个字节仍影响解密，覆盖 ASCII 与多字节 UTF-8。
3. 原先导出不支持的 EC 私钥会在报错前发布公钥。现于发布前检查算法并准备报告；回归确认 EC 输入被拒绝且无输出文件。
4. 密钥报告曾直接通过 `json!(PathBuf)` 序列化路径，遇到非 UTF-8 路径会 panic。已改为在写入前验证报告路径，失败明确返回中文错误与退出码 1。新增 generate/public/check 三个回归；本机明确复现了 public 的 panic，另两项验证前置错误和无输出边界。

## 保留的边界

生成密钥逐文件发布，不宣称跨文件原子事务；中断可能留下受保护但不完整的目录。RSA 与 Ed25519 是本轮支持算法；Ed25519 用于签名，本工具交付密钥管理，不执行签名或数据加解密。所有测试密钥均在临时目录中创建，未操作用户现有密钥。

密钥命令中需要写入报告的路径须可表示为 UTF-8；非 UTF-8 路径在写入前拒绝。该限制不扩展到 Hash/Base64 的原始字节输入或其他工具的路径处理。

HTML5 采用文档明确的严格实体策略，不复制浏览器 tokenizer 的全部容错行为；字符集转码不在本次范围。URL parse 的 JSON 按接口约定包含原始 query，Hash/UUID 报告不复制输入内容，密钥报告不包含私钥或口令。

本轮未执行 git commit、推送或发布。
