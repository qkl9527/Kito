# Kito 编解码、标识与密钥伙伴设计

日期：2026-10-28。状态：已实现并完成本机验证，包含用户要求的 Ed25519；以下为实施约定。

## 用户需求与设计决定

用户选定 Hash Generator、UUID Generator、Base64 编解码、URL 参数解析与提取、支持多种规范的 HTML 编解码，以及 OpenSSL 非对称密钥对，并授权安排角色及能力归属。

本设计新增三位伙伴。用户已确认角色与命令安排，并要求密钥伙伴在首版支持 Ed25519。

| 伙伴 | 称号 | 功能入口 | 职责 |
| --- | --- | --- | --- |
| Codi 可迪 · 编解码伙伴 | 译码师 | `codec` | Base64、URL 参数、HTML/XML 实体转换 |
| Sumi 苏米 · 标识伙伴 | 印记师 | `id` | 内容 Hash 与 UUID 生成 |
| Kavi 卡维 · 密钥伙伴 | 钥匙匠 | `key` | 通过 OpenSSL 生成、导出和检查 RSA / Ed25519 公私钥 |

可迪负责数据在不同表示方法之间转换，URL 参数提取是该工作流中的读取步骤。苏米处理“如何标识一份内容或一个对象”：Hash 来自内容，UUID 是新生成的标识。两者保持独立子命令和明确解释，不暗示 UUID 可以校验内容。卡维涉及私钥、口令、文件权限和外部密码后端，单独形成领域。

不采用一个宽泛的 `dev` 角色，因为这六类能力的输入、风险和依赖差异较大；也不为每一个编码算法新建角色。现有 `image`、`text`、`time` 保持既有职责。角色名只出现在介绍、帮助与品牌文档，不增加角色名命令别名。

## 角色表达

- **Codi 可迪：** 耐心的译码师，把绕口的数据表示解释清楚。帮助文案：“负责 Base64 编解码、URL 参数解析与提取，以及 HTML/XML 实体转换。按明确规范处理输入，保留重复参数。”
- **Sumi 苏米：** 细致的印记师，给内容算指纹、给对象生成标识。帮助文案：“负责计算字符串和文件的 Hash，以及生成 UUID。说明算法与版本，结果适合脚本使用。”
- **Kavi 卡维：** 稳妥的钥匙匠，把公钥与私钥配好并检查一致性。帮助文案：“通过 OpenSSL 生成和检查 RSA / Ed25519 密钥对，导出公钥。明确保存位置，默认保护私钥。”

不把人格描述当作正确性或安全性保证。首版只建设 CLI 角色和功能，不制作头像、角色别名或额外世界观。

## 拟议命令结构

```text
kito
├── codec                      Codi 可迪
│   ├── base64 encode/decode
│   ├── url parse/get
│   └── html encode/decode
├── id                         Sumi 苏米
│   ├── hash
│   └── uuid
└── key                        Kavi 卡维
    ├── generate
    ├── public
    ├── check
    └── doctor
```

## 通用输入与输出

- Hash 与编解码支持位置参数文本、`--file PATH`、非终端 stdin，三种输入互斥。缺少参数且 stdin 是终端时输出用法错误，不悬空等待读取。位置参数按 UTF-8 编码；Hash 与 Base64 文件输入允许任意字节。
- `--file -` 明确读取 stdin。文件字节不自动去 BOM、去换行或改变行尾。URL/HTML 输入必须是有效 UTF-8。
- 编解码结果默认直接写 stdout，不自动添加字符。Base64 解码输出原始字节，重定向或 `-o` 保存；直接输出到终端前仅允许有效 UTF-8 且无控制字符的内容（TAB/LF/CR 除外），否则提示使用文件或重定向。
- Hash 默认输出一个小写十六进制摘要并换行；UUID 每行一个。URL parse 默认展示组件及有序参数；URL get 默认每行一个匹配值，含换行的值应使用 `--json` 避免边界歧义。
- `id`、URL、`key` 提供 `--json`；JSON 中数组保留顺序，不用对象吞掉重复 key。Base64/HTML 首版直接返回转换结果，不额外提供与原始字节冲突的 JSON 模式。
- 文本/字节结果可用 `-o/--output` 写文件，默认不覆盖。普通结果显式 `--overwrite` 才替换；拒绝同输入文件或符号链接目标，复用验证后单文件原子发布能力。`--json` 与 `--output` 互斥，避免不清楚写的是报告还是处理结果。
- 解析或规范错误在输出前报错；不输出一半结果。Hash 按块读取大文件；编解码首版默认输入上限 64 MiB，提供 `--max-input-mib` 调整，读取时再次检查上限。stdout 下游提前关闭按 CLI 通常行为处理，不 panic。
- 所有错误使用现有中文错误入口；退出码 0 表示成功、1 表示运行失败，clap 用法错误沿用 2。URL key 未找到和密钥不匹配返回 1。

## Codi：Base64

```sh
kito codec base64 encode '你好，Kito'
kito codec base64 encode --file image.png -o image.b64
kito codec base64 encode 'hello' --variant url-safe --padding unpadded
kito codec base64 decode 'aGVsbG8=' -o hello.txt
kito codec base64 decode --file image.b64 --ignore-whitespace -o restored.png
```

- `--variant standard|url-safe` 默认 standard。两种字母表分别遵循 RFC 4648 Base64 与 base64url。
- 编码 `--padding padded|unpadded` 默认 padded；解码同名参数增加 auto，默认 auto，只接受规范的有填充或无填充形式，不接受混合、过量填充、错误位置及非零尾部填充位。
- 默认拒绝输入中的空白；`--ignore-whitespace` 仅忽略 ASCII 空格、TAB、CR、LF。它是明确的兼容选项，不自动切换 MIME 或字母表。
- 编码不自动折行；首版不做 MIME 邮件格式、Data URL 封装或任意字符集转换。
- 帮助以“编码/解码”描述，不使用“加密/解密”。

## Codi：URL 参数

```sh
kito codec url parse 'https://example.com/search?q=hello+world&tag=a&tag=b#top' --json
kito codec url get 'https://example.com/?tag=a&tag=b' --key tag
kito codec url get '?q=a+b' --query --key q --mode percent
kito codec url parse 'x=&flag&x=2' --query --json
```

- 默认输入为带 scheme 的绝对 URL；`--query` 明确选择原始查询串，可含开头的 `?`。不根据字符串内容猜测输入类型。
- `parse` 返回 URL 组件以及 query 参数；`--query` 时只返回查询信息。完整 URL 输入先拒绝未经编码的 ASCII 空格、C0 控制字符与 DEL，防止解析库静默删除或修复输入；纯查询串允许普通空格，但拒绝 C0 控制字符与 DEL。解析库规范化后的 URL 组件与原始 query 分开保存，JSON 必须注明哪些字段是解析后的值。
- 参数列表保留原始次序、重复名称、空名称、空值，并用 `has_equals` 区分 `flag` 与 `flag=`。按 `&` 切分，按每段的首个 `=` 切分名称和值；空片段跳过，片段出现次数与原始 query 可核对。
- `--mode form|percent` 默认 form。form 使用表单查询语义，`+` 转为空格；percent 只作百分号解码，`+` 保持原样。RFC 3986 规定百分号编码，但不规定通用 key/value 格式；percent 模式的分隔规则是 Kito 明确采用的约定，不宣称是完整 RFC 查询标准。
- 两种模式都校验 `%HH` 完整性，并严格校验解码后的 UTF-8；这是 Kito 的严格输入策略，不使用 WHATWG 容错替换非法字节的行为。
- `get --key NAME` 按解码后的名称精确匹配，默认返回所有匹配值，保留顺序；`--first` 显式取首项。JSON 的 `values` 始终为数组，无匹配时输出空数组并返回 1。
- 完整 URL 的 fragment 不参与查询；`--query` 输入按纯查询内容处理，因此裸 `#` 是查询内容，不自动截断。只解码一层，不递归处理嵌套 URL。
- 首版不访问 URL、不请求网络，也不修改参数或生成签名链接。

## Codi：HTML/XML 实体

“多种编码规范”在首版解释为实体标准与实体表示形式；UTF-8、GBK、UTF-16 属于另一类字符集转换需求，本提案未将其混入 HTML 实体转换。

```sh
kito codec html encode '<a title="Kito">你好 & hello</a>'
kito codec html encode '你好 & Kito' --style hex --all
kito codec html decode '&lt;b&gt;你好&#33;&lt;/b&gt;' --standard html5
kito codec html encode '<tag value="1">' --standard xml --style named
```

- `--standard html5|xml` 默认 html5；xml 明确指 XML 1.0。
- 编码支持 `--style named|decimal|hex`，分别为命名实体、十进制数字引用和十六进制数字引用。named 对无命名形式的必需编码字符使用数字引用。
- `--context text|attribute` 默认 attribute。text 编码 `& < >`，attribute 额外编码双引号和单引号；XML attribute 中 TAB/LF/CR 使用数字引用以保留值。`--all` 明确对全部字符编码；切换 style 本身不改变编码范围。
- HTML5 使用完整命名实体集合，包含映射为多个 Unicode 码点的实体。编码选择确定的实体别名，不承诺保留输入实体原有拼写。
- 解码一次识别命名、十进制和十六进制实体；严格要求分号。`&` 后接 ASCII 字母或 `#` 就按实体候选读取，缺分号、未知实体、非法数字、越界码点、surrogate 或该标准不允许的字符报错；其他裸 `&` 保留。`&amp;lt;` 只变成 `&lt;`，不继续变为 `<`。
- 文档称为“基于 HTML5 实体集合的严格转换”，不承诺浏览器 tokenizer 对缺失分号、控制字符和损坏引用的全部容错行为。实现计划需用输入/输出示例固定控制字符、历史 C1 映射和错误位置，不得无说明地套用宽松库函数。
- XML 仅支持五个预定义命名实体和数字引用，按 XML 1.0 字符范围校验，不解析文档、DTD、外部或自定义实体。
- 这项能力不进行 HTML 清洗，也不用于 JavaScript、CSS 或 URL 上下文的安全编码。

## Sumi：Hash

```sh
kito id hash 'hello'
kito id hash --file archive.zip --algorithm sha512
kito id hash --file archive.zip --algorithm sha256 --json
```

- `--algorithm md5|sha1|sha224|sha256|sha384|sha512`，默认 sha256。MD5/SHA-1 用于已有数据兼容，帮助说明其不适用于安全校验用途；不把这些摘要功能当作密码存储方案。
- 位置参数计算 UTF-8 字节摘要，文件/stdin 计算实际字节。默认十六进制，`--format hex|base64` 可选择摘要表示。
- JSON 包含 algorithm、format、digest、input_bytes、source_kind；字节数使用字符串，避免极大输入在消费端损失精度，不复制原始数据到报告。
- 首版只生成 Hash，不加入目录递归、校验清单格式、HMAC 或密码 Hash。

## Sumi：UUID

```sh
kito id uuid
kito id uuid --version v7 --count 10
kito id uuid --version v4 --format simple --json
```

- `--version v4|v7` 默认 v4；v4 为随机 UUID，v7 带 Unix 毫秒时间字段，适合需要按时间组织标识的场景。
- 默认小写带连字符形式，`--format hyphenated|simple|urn` 选择表示。`--count` 默认 1，范围 1–10000。
- 随机数据来自操作系统安全随机源，获取失败要报错，不回退到固定种子。UUID 不宣称绝对不重复，也不作为密钥或授权凭证。
- v7 不承诺多进程、时钟回拨及同毫秒内的全局严格递增；没有验证就不作这类保证。
- JSON 包含 version、format、values 数组。UUID 首版不提供用户指定时间戳、v1/v3/v5 或命名空间生成。

## Kavi：OpenSSL 密钥对

首版生成并检查 RSA 与 Ed25519 公私钥，额外提供从私钥导出公钥。Ed25519 供签名场景使用；本轮交付密钥管理命令，数据加解密、签名操作、证书和 SSH 密钥不纳入该首版。

```sh
# 默认 RSA 3072，保存到必须不存在的新目录
kito key generate --out-dir ./keys/app --passphrase-file ./private-passphrase.txt

# 显式选择不加密私钥，便于需要此格式的本地工具使用
kito key generate --bits 4096 --out-dir ./keys/local --no-passphrase

kito key public ./keys/app/private.pem --passphrase-file ./private-passphrase.txt -o ./public.pem
kito key check ./keys/app/private.pem --passphrase-file ./private-passphrase.txt --public ./keys/app/public.pem --json
kito key doctor --json
```

- 首版 `--algorithm rsa|ed25519`，默认 rsa。RSA 位数 2048/3072/4096，默认 3072、公钥指数 65537、两素数；Ed25519 使用固定算法参数，拒绝 `--bits`。3072 是 Kito 选择，不声称是 OpenSSL 默认。Ed25519 明确用于签名，不宣传为数据加密算法；EC/X25519 待以后加入。`public/check` 同时支持上述两种算法。
- 私钥 PKCS#8 PEM，公钥 SPKI PEM。加密私钥使用明确选定的 OpenSSL PKCS#8 加密选项，计划需固定算法和 KDF 参数，并在报告中列出；优先 PBES2/PBKDF2-HMAC-SHA256 与 AES-256-CBC。
- 首版选择显式口令来源：`generate` 必须在 `--passphrase-file PATH`、`--passphrase-stdin`、`--no-passphrase` 中选一个；未选择即给出用法提示，不静默生成未加密私钥。自动化与终端遵循相同行为，首版不做交互式密码提示。
- 口令接受 UTF-8 单行输入，可去掉一个末尾 LF 或 CRLF；口令最多 1023 个 UTF-8 字节，避免 OpenSSL stdin 的行缓冲截断；拒绝空口令、嵌入 CR/LF、NUL 和超长输入。口令不得作为明文参数传给子进程，不使用环境变量口令，不写入日志、报告或新建明文临时文件。传递给 OpenSSL 时使用专门的 stdin 管道。
- `public/check` 可接受口令文件或 stdin；没有来源时按未加密私钥处理，若实际已加密则给出明确错误，不让 OpenSSL 阻塞交互。
- `generate --out-dir` 必须不存在，拒绝符号链接和覆盖。目录以排他创建保留本次操作的所有权；在受控临时位置生成并验证两份候选，再分别发布 `private.pem` 和 `public.pem`。成功只在两者全部就绪后报告。
- 首版不宣称跨文件原子事务。常规错误尽力移除本次创建且仍可确认归属的文件/目录；中断可能留下不完整目录，错误需列出保留路径。已有文件不得被修改；没有密钥覆盖选项。
- Unix 受控目录设为 0700，私钥及其临时文件设为 0600。Windows 需要明确实现当前用户访问控制并检查 ACL，不能把 Unix mode 或 readonly 标志当作 Windows 权限保证；未完成 Windows 验证前不得宣传该路径已在 Windows 验证。
- 公钥导出 `public` 为单文件原子发布且不覆盖已有输出。生成前后调用 OpenSSL 检查私钥内部一致性；给定公钥需确认为 SPKI PUBLIC KEY，规范化为 DER 后与从私钥导出的公钥比较。不能仅用 `pkey -check` 代替另一文件的匹配检查。
- 报告包括算法、位数、格式、私钥加密状态、输出路径、OpenSSL 版本以及 SPKI DER 的 SHA-256 fingerprint；不包含私钥内容或口令。`check` 报告 validity 与 match；不匹配为 1。
- 密钥报告中的路径要求 UTF-8；generate/public 在任何写入前验证输出路径，check 在读取前验证需要报告的私钥路径。不可表示的路径明确报错，不能因 JSON 序列化 panic 或在失败前发布输出。
- 使用可选外部 OpenSSL，`KITO_OPENSSL` 明确覆盖 PATH；配置错误不静默回退。仅 key 命令探测该后端，不影响其他伙伴。初始开发参考 OpenSSL 3.5 文档；兼容 OpenSSL 3.x 需结合实际能力验证，明确拒绝 LibreSSL/旧主版本，报告实际版本。
- 后端用参数数组执行，设置超时，私钥始终用 `-out` 写受控路径；不通过通用 stdout 临时文件回收私钥，不输出私钥文本详情。

## 工程划分与验证

按现有 `src/time/` 模式创建 `src/codec/`、`src/id/`、`src/key/`，各自维护 CLI 适配。入口只分派，共享原始字节输入、结果写出、安全子进程能力独立维护；避免直接复用含图片专用限制的路径规划函数。

实现计划可以决定具体 Rust crates 和函数拆分；使用维护中的库处理标准算法及实体表，不自创密码算法，不把外部后端缺失伪装成成功。现有 `sha2` 可用于默认摘要；新依赖锁入 Cargo.lock。

验证重点为标准向量和错误边界：Hash 已知摘要、UUID 版本/variant、Base64 二进制回环及坏填充、URL 重复 key/空值/加号/非法 UTF-8、HTML 多码点实体/XML 非法字符/只解码一层、OpenSSL 真实生成与匹配检查/错误口令/输出冲突/超时/权限。密钥测试只在临时目录生成测试密钥，不读写用户现有密钥。后端缺失用明确跳过记录，不能把跳过写成验证通过。

使用项目本地 Rust 工具链运行 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`，记录本机与 CI 的实际平台范围。更新 README 工具索引、`docs/brand.md`、`docs/tools/codec.md`、`docs/tools/id.md`、`docs/tools/key.md` 与必要的项目约定。所有新能力在真正实现后才进入可用功能列表；不提交或推送。

## 主要参考

- [RFC 4648：Base64 与 base64url](https://www.rfc-editor.org/rfc/rfc4648)
- [WHATWG URL：表单查询解析](https://url.spec.whatwg.org/#application/x-www-form-urlencoded-parsing)
- [RFC 3986：百分号编码与 URI 语法](https://www.rfc-editor.org/rfc/rfc3986)
- [HTML 命名字符引用](https://html.spec.whatwg.org/multipage/named-characters.html)
- [XML 1.0](https://www.w3.org/TR/xml/)
- [RFC 9562：UUID v4/v7](https://www.rfc-editor.org/rfc/rfc9562)
- [OpenSSL genpkey](https://docs.openssl.org/3.5/man1/openssl-genpkey/)
- [OpenSSL pkey](https://docs.openssl.org/3.5/man1/openssl-pkey/)
- [OpenSSL passphrase options](https://docs.openssl.org/3.5/man1/openssl-passphrase-options/)

