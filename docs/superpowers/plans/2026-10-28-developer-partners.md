# Kito 三位开发伙伴 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. 用户已要求直接实施，不再次请求设计或计划确认。使用独立文件所有权并行实施；不提交、推送或发布。

**Goal:** 实现 Codi 编解码、Sumi Hash/UUID、Kavi RSA/Ed25519 密钥能力。

**Architecture:** 每位伙伴拥有独立目录和 `cli::run(Args) -> anyhow::Result<bool>`，顶层负责分派。共享字节输入/安全输出集中于 `src/io_args.rs`。密钥专用进程与权限处理放在 key 内，避免改变图片后端行为。

**Tech Stack:** Rust、clap、serde、base64、sha2/sha1/md-5、uuid/getrandom、url、HTML5 实体表、OpenSSL 3.x。

**Spec:** `docs/superpowers/specs/2026-10-28-developer-partners-design.md`（用户批准并增加 Ed25519）。

## Global Constraints

- 中文文档、关键流程注释；不 commit/push，不改变现有功能输出。
- `codec` / `id` / `key` 分别为 Codi 可迪、Sumi 苏米、Kavi 卡维。
- 文件默认不覆盖；输入输出同文件拒绝；原始字节不自动增加换行。
- Hash/UUID 报告不复述输入内容；URL parse 按规格保留原始 query；私钥与口令不进 stdout、日志或报告。
- Hash 默认 SHA-256；UUID 默认 v4；Base64 默认 standard；HTML 默认 html5/attribute/named。
- RSA 默认 3072；Ed25519 是签名算法，禁止 bits 参数；私钥 PKCS#8，公钥 SPKI。
- 通过 `scripts/cargo-local.sh` 验证；仅报告实际验证平台。

## Review Focus

1. 编码源输出同文件、符号链接及输出冲突必须保留原文件：Task 1/2。
2. Unicode、多码点实体、非法百分号/UTF-8、嵌套转义不能静默改写：Task 2。
3. 随机源错误、UUID v7 位布局和系统时间异常不可 panic：Task 3。
4. OpenSSL stdin 口令、错误口令、进程超时和密钥配对失败不泄露秘密：Task 4。
5. Ed25519 私钥检查与公钥算法识别、Windows ACL 及未覆盖平台声明：Task 4/5。

### Task 1: 共享 IO 与入口（主协调者）

**Files:** `src/io_args.rs`, `src/lib.rs`, `src/cli.rs`, `src/main.rs`, `Cargo.toml`, `Cargo.lock`, `tests/partners_cli.rs`。

**Interfaces:**
- `InputArgs { pub text: Option<String>, pub file: Option<PathBuf> }`（clap Args/Default/Clone）。
- `InputArgs::open(&self) -> Result<(Box<dyn Read>, &'static str)>`：source_kind 为 text/file/stdin；没有源且 stdin 为 TTY 拒绝；`--file -` 为 stdin。
- `InputArgs::read_bytes(&self, max_bytes: usize) -> Result<Vec<u8>>`、`read_utf8(&self, max_bytes: usize) -> Result<String>`。
- `OutputArgs { pub output: Option<PathBuf>, pub overwrite: bool }`（clap Args/Default/Clone）。
- `OutputArgs::write(&self, input: &InputArgs, bytes: &[u8], terminal_guard: bool) -> Result<()>`：按需终端二进制保护、验证路径、文件原子发布。
- `write_stdout(bytes: &[u8]) -> Result<()>`：BrokenPipe 作为成功处理。
- 各模块提供 `pub mod cli;`、`cli::CodecArgs/IdArgs/KeyArgs` 与 `cli::run(args) -> Result<bool>`。

- [x] 添加三伙伴 CLI 行为测试，运行并观察现有命令拒绝新入口。
- [x] 实现共享输入输出，补充二进制保持、同文件拒绝与 stdin 互斥用例。
- [x] 统一管理依赖和模块入口；各实现者不得写 Cargo/顶层共享文件。
- [x] 运行目标测试与格式检查。

### Task 2: Codi 编解码（独立实现者）

**Files:** `src/codec/{mod,cli,base64,url,html}.rs`、`tests/codec_cli.rs`、`docs/tools/codec.md`。

**Interfaces:** 消费 Task 1 IO；提供 `CodecArgs` 与 `run`。Base64 使用 base64 crate；HTML 命名实体使用成熟库公开表或带来源的标准表，不只替换五个实体。

- [x] 先编写标准向量、二进制回环、重复 URL 参数及 HTML 多码点实体测试，观察缺失命令失败。
- [x] 实现 Base64 两字母表、padding 与 ASCII 空白策略；编解码默认 max 64 MiB。
- [x] 实现 URL parse/get、form/percent、显式 query 输入及 JSON 数组，先切分后严格解码一次。
- [x] 实现 HTML5/XML strict decode，named/decimal/hex、text/attribute、all；明确 C1 和非字符行为，拒绝非法 scalar；XML 完整 Char 校验。
- [x] 运行 `cargo test --test codec_cli` 与模块测试，记录命令、结果及文档。

### Task 3: Sumi 标识（独立实现者）

**Files:** `src/id/{mod,cli,hash,uuid}.rs`、`tests/id_cli.rs`、`docs/tools/id.md`。

**Interfaces:** 消费 Task 1 IO；提供 `IdArgs` 与 `run`。Hash 使用 Digest 流式块读取；UUID 使用 fallible OS 随机字节与标准位布局，不走可能 panic 的 RNG 包装。

- [x] 先编写 `abc` 各算法摘要、stdin 二进制、UUID version/variant/数量测试并观察缺失能力失败。
- [x] 实现 md5/sha1/sha224/sha256/sha384/sha512，默认 sha256，hex/base64 与 JSON。
- [x] 实现 v4/v7、hyphenated/simple/urn、count 1–10000、JSON；v7 前48位 Unix ms，保留 version/variant。
- [x] 运行 `cargo test --test id_cli` 与模块测试，补全工具文档。

### Task 4: Kavi 密钥（独立实现者）

**Files:** `src/key/{mod,cli,process,storage}.rs`（可合理拆分）、`tests/key_cli.rs`、`docs/tools/key.md`。

**Interfaces:** 提供 `KeyArgs` 与 `run`；复用 files 单文件思路。专用 child stdin 管道传口令，超时、stdout/stderr 有限且不含私钥，所有私钥用 -out。

- [x] 先编写缺少后端、参数互斥、已有输出保护及 generate/public/check 合同测试。
- [x] 实现 KITO_OPENSSL 路径、版本/能力探测与 doctor。真实测试后端可用 `/opt/homebrew/opt/openssl@3/bin/openssl`，该路径只在测试环境设定，不硬编码产品默认。
- [x] RSA 2048/3072/4096 默认3072；Ed25519 无 bits。口令 file/stdin 或显式 no-passphrase。
- [x] PKCS#8 保护采用 PBES2/PBKDF2-HMAC-SHA256 600000 次与 AES-256-CBC；报告具体参数。不把默认 genpkey 的隐含参数作为约定。
- [x] 新目录独占保存，候选验证后逐文件发布；Unix0700/0600；Windows 使用用户 SID 的受保护 ACL，支持 cfg 下实现与编译检查。
- [x] public/check 识别 RSA/Ed25519、验证内部一致性及给定 SPKI DER 相等，报告 SHA256 fingerprint；不匹配退出1。
- [x] 临时目录真实测试 RSA/Ed25519、加密私钥、错误口令、错误公钥、不覆盖和缺失后端；补全工具文档。

### Task 5: 整合、审阅和全量验证（主协调者）

**Files:** `README.md`, `docs/brand.md`, `AGENTS.md`, `.github/workflows/ci.yml`（如需要）, `docs/validation/2026-10-28-developer-partners.md`。

- [x] 整合已实现入口、伙伴介绍、工具链接和常见示例；避免把未实现能力写成可用。
- [x] 独立审阅各模块的规范符合度与实现质量，修复重要问题。
- [x] 在本机实际 OpenSSL 3.x 环境运行 fmt、clippy、cargo test；按已有 CI 风格补充后端支持但不宣称远程 CI 已运行。
- [x] 记录验证数目、后端版本、已知跨平台限制；输出可直接运行的示例。

## 执行记录

用户明确要求直接实施。仓库无 HEAD，所有现有文件未跟踪，不能从提交建立包含当前代码的 worktree；使用当前目录，文件归属隔离，不创建提交。并行仅写独立新模块，共享文件由协调者统一修改。
