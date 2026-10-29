# Kavi 卡维 · 密钥伙伴

Kavi 是 Kito 的钥匙匠，通过可选 OpenSSL 3.x 后端生成、导出和检查 RSA / Ed25519 密钥对。命令入口为 `kito key`。Ed25519 用于签名，不是数据加密算法；此工具暂不执行数据加解密、签名、证书或 SSH 密钥操作。

## 查看逐层帮助

`key` 及其每个子命令都支持 `-h` 和 `--help`。帮助先展示简介与 Usage，再展示“使用案例”，最后列出子命令或参数说明；案例中的命令均可单行复制。

```sh
kito key -h
kito key --help
kito key generate --help
kito key public --help
kito key check --help
kito key doctor --help
```

加密密钥案例中的 `passphrase.txt` 需要预先准备；口令不直接写在命令行参数中。生成案例使用的目标目录必须不存在，导出案例也不会覆盖已有文件。

## 准备后端

```sh
kito key doctor
kito key doctor --json
```

默认从 PATH 查找 `openssl`；`KITO_OPENSSL` 可指定可执行文件路径，显式配置无效时不会静默回退。仅 `key` 命令需要该后端。要求 OpenSSL 3.x，并检查 RSA、Ed25519 与 AES-256-CBC 的可用性；LibreSSL 和旧主版本会明确报错。macOS 系统自带的 `openssl` 可能是 LibreSSL，需要另外安装 OpenSSL 3 并配置其实际位置。

```sh
KITO_OPENSSL=/your/path/to/openssl kito key doctor --json
```

后端使用参数数组运行，每次操作最多等待 120 秒，stdout / stderr 各限制 1 MiB。失败不打印后端原始诊断，以免密钥或口令意外进入日志。

## 生成密钥对

```sh
# RSA 默认 3072 位；缺少的父目录自动创建，app 目录必须不存在
kito key generate --out-dir ./keys/app --passphrase-file ./passphrase.txt

# RSA 支持 2048 / 3072 / 4096 位
kito key generate --bits 4096 --out-dir ./keys/rsa --passphrase-stdin

# Ed25519 参数固定，不接受 --bits
kito key generate --algorithm ed25519 --out-dir ./keys/signing --passphrase-file ./passphrase.txt

# 显式允许未加密私钥
kito key generate --algorithm ed25519 --out-dir ./keys/local --no-passphrase --json
```

必须且只能选择一种保护方式：`--passphrase-file PATH`、`--passphrase-stdin`、`--no-passphrase`。不会自动提示输入，也不会默认生成未加密私钥。RSA 使用两素数、公钥指数 65537；默认 3072 位是 Kito 的选择。

成功后目录中只有：

| 文件 | 格式 |
| --- | --- |
| `private.pem` | PKCS#8 PEM，按所选保护方式加密或不加密 |
| `public.pem` | SPKI PEM（`PUBLIC KEY`） |

加密私钥明确使用 **PBES2 / PBKDF2-HMAC-SHA256、600000 次迭代、AES-256-CBC**。不依赖 OpenSSL `genpkey` 的隐含加密默认参数。报告的 `encryption` 列出这些参数；未加密时为 `null`。

口令接受 1–1023 字节 UTF-8 单行文本，去掉至多一个末尾 LF 或 CRLF；拒绝空值、内嵌 CR/LF、NUL、无效 UTF-8 和超长输入。该上限按 UTF-8 字节数计算，避免 OpenSSL stdin 口令读取截断；1024 字节及以上会在生成文件前拒绝。口令只通过 OpenSSL 的 stdin 管道传递，不加入命令行参数或环境变量，不新建口令明文临时文件。口令文件由用户自行保护。

## 导出公钥

```sh
kito key public ./keys/app/private.pem \
  --passphrase-file ./passphrase.txt -o ./exported-public.pem

kito key public ./keys/local/private.pem -o ./exported-local.pem --json
```

`-o/--output` 必填；输出文件必须不存在，包括已有符号链接。先检查私钥内部一致性，再生成候选公钥，最后通过同文件系统 hard link 排他发布。输出不能指向已有私钥或其他文件，没有覆盖选项。stdout 只输出结果报告，不输出私钥或口令。

## 检查密钥与配对

```sh
kito key check ./keys/app/private.pem --passphrase-file ./passphrase.txt

kito key check ./keys/app/private.pem \
  --public ./keys/app/public.pem --passphrase-file ./passphrase.txt --json
```

输入私钥须为 PKCS#8 PEM；PKCS#1 `RSA PRIVATE KEY`、证书和 SSH 格式不在此入口支持范围内。私钥检查支持 RSA 与 Ed25519。已加密私钥未提供口令来源时立即报错，不进入交互等待。

`--public` 须为 SPKI PEM `PUBLIC KEY`。工具将指定公钥和从私钥导出的公钥分别规范化为 DER 后比较，不能仅凭私钥自检判断配对。JSON 包含 `validity` 和 `match`；未提供公钥时 `match` 为 `null`，匹配为 `true`，不匹配为 `false` 并退出 1。格式错误、错误口令或内部一致性检查失败也退出 1，不会报告有效。

所有成功报告包含 `algorithm`、`bits`、格式、`encrypted`、`openssl_version`、相关输出路径和 `fingerprint`。fingerprint 为 `SHA256:` 后接 SPKI DER 摘要的十六进制表示。Ed25519 的 `bits` 表示 256 位编码密钥长度，不表示 256 位安全强度。`check` 不猜测现有加密私钥的 KDF 参数；固定加密参数仅是本工具 `generate` 的契约。

## 文件与权限边界

密钥结果报告中的路径必须可表示为 UTF-8：`generate --out-dir` 及派生输出路径、`public --output`、`check` 的私钥路径会在文件写入或发布前校验。不合法时返回中文错误并退出 1，不创建输出或生成目录。此限制仅针对报告路径，不是其他工具或所有输入路径的统一限制。

缺少的父目录会自动创建，失败时保留这些共享父路径。生成目标目录排他创建；拒绝已有目录、文件和符号链接。Unix 目录为 0700，私钥及所有候选文件为 0600；公钥继承同样的保守权限，可按分发需要自行复制。

Windows 实现通过 Win32 API 获取当前用户 SID，在创建目录时设置仅允许该用户的受保护 DACL，并让文件继承该权限；创建后再次检查 ACL。不使用 shell、readonly 或 Unix mode 冒充访问控制。Windows 运行验证范围应以实际 CI 结果为准，不能把 macOS 本地通过解释为三平台均已验证。

临时未加密私钥仅存在于上述受保护目录内，通过 OpenSSL `-out` 写入，不经 stdout。生成前后检查私钥，并确认候选公私钥配对成功，才分别发布 `private.pem` 和 `public.pem`。这是逐文件发布，不是跨文件原子事务。常规失败尽力清理本次创建且身份仍一致的文件和目录；进程中断可能留下不完整目录。未清理完的目录会在错误中列出，应由用户检查后处理；已有文件不会被覆盖。

## 开发验证

```sh
KITO_OPENSSL=/your/path/to/openssl KITO_REQUIRE_BACKEND=1 \
  scripts/cargo-local.sh test --test key_cli
```

测试仅在临时目录中生成测试密钥。未设置 `KITO_REQUIRE_BACKEND=1` 且 OpenSSL 3 不可用时，真实后端测试明确输出跳过原因；设置该变量后缺后端会使测试失败。跳过不代表后端或平台验证通过。

返回 [工具总入口](../../README.md)。
