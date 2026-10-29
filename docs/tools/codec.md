# Codi 可迪 · 编解码伙伴

可迪是 Kito 的译码师，负责 Base64 编解码、URL 参数解析，以及 HTML/XML 实体转换。入口为 `kito codec`，没有角色名别名。

## 查看命令帮助

在任意层级追加 `-h` 或 `--help`，都可以查看当前命令的帮助。帮助按 `Usage`、使用案例、子命令或参数说明的顺序展示；先选择能力，再深入查看具体操作的输入、输出和选项。

```sh
kito codec -h
kito codec base64 --help
kito codec base64 encode -h
kito codec base64 decode --help
kito codec url --help
kito codec url parse -h
kito codec html --help
kito codec html encode -h
kito codec html decode --help
```

## 输入与保存

所有命令接受位置参数文本、`--file PATH` 或非终端 stdin；位置参数和文件互斥，`--file -` 明确读取 stdin。文件保留原始字节、BOM 和换行，URL/HTML 要求 UTF-8。未提供来源且 stdin 为终端时立即报错。

默认输入上限 64 MiB，可用 `--max-input-mib N` 调整；读取过程中也会检查上限，整数溢出时报错。转换结果在完整验证后才写出。Base64 和 HTML 不自动添加换行。

`-o/--output PATH` 保存结果，默认不覆盖；显式 `--overwrite` 才允许替换已有结果。输入与输出不能指向同一文件（包括符号链接或硬链接）；验证成功后原子发布单个文件。URL 的 `--json` 与 `--output` 互斥。

```sh
kito codec base64 encode --file image.png -o image.b64
kito codec base64 decode --file image.b64 -o restored.png
printf '你好' | kito codec base64 encode
```

Base64 解码可能产生二进制。直接输出到终端时，只允许有效 UTF-8 且不含控制字符（TAB/LF/CR 除外）；其他内容请保存文件或重定向。下游提前关闭 stdout 不会触发 panic。

## Base64

```sh
kito codec base64 encode '你好，Kito'
kito codec base64 encode 'hello' --variant url-safe --padding unpadded
kito codec base64 decode 'aGVsbG8='
kito codec base64 decode --file image.b64 --ignore-whitespace -o image.png
```

`--variant standard|url-safe` 默认 `standard`，遵循 RFC 4648 两种不同字母表。不会自动猜测或混用。编码 `--padding padded|unpadded` 默认 `padded`；解码增加 `auto` 并以此为默认，只接受规范的有填充或无填充形式。错误填充位置、过量/不足填充、非零尾部位均报错，例如 `Zg=`、`Zg===`、`Zh==`。

默认拒绝空白；`--ignore-whitespace` 只删除 ASCII 空格、TAB、CR、LF，其他空白仍报错。编码不折行，不处理 MIME、Data URL 或字符集转换。

## URL 参数

```sh
kito codec url parse 'https://example.com/search?q=hello+world&tag=a&tag=b#top' --json
kito codec url parse 'https://example.com/?tag=a&tag=b&empty=&flag'
kito codec url parse '?q=a+b' --query --mode percent --json
kito codec url parse 'x=&flag&x=2' --query --json
```

默认要求带 scheme 的绝对 URL。`--query` 明确选择原始查询串，允许一个开头 `?`；不会凭内容猜测类型。完整 URL 拒绝未经编码的 ASCII 空格、C0 控制字符、DEL，防止 URL 库静默修复；纯查询串允许普通空格，仍拒绝 C0 和 DEL。

`parse` 完整展示查询串中的参数，不按名称过滤。参数先按 `&` 切分，再按首个 `=` 切分，最后严格解码一次。保留次序、重复名称、空名称、空值；空片段跳过。`has_equals` 区分 `flag` 与 `flag=`。

`--mode form|percent` 默认 `form`：form 把 `+` 变为空格，percent 保留 `+`。两者都严格验证 `%HH` 和解码后 UTF-8，`%`、`%GG`、`%FF` 会报错，不替换为 `�`。percent 模式的 key/value 分隔是 Kito 的约定，不宣称覆盖 RFC 3986 的全部查询语义。

完整 URL 的 fragment 不参与参数。纯查询串中的裸 `#` 是参数内容。`q=%2526%3Dx` 的值是 `%26=x`，不会继续解码为 `&=x`。

默认输出保留 URL 组件、原始 query 和逐项参数行，并在末尾追加标题 `Query JSON:` 与漂亮缩进的 JSON 对象，方便复制。例如上面的 `tag=a&tag=b&empty=&flag` 会追加：

```text
Query JSON:
{
  "empty": "",
  "flag": "",
  "tag": [
    "a",
    "b"
  ]
}
```

对象中的单个参数值是字符串；重复同名参数合并为按出现顺序排列的字符串数组。空值和没有 `=` 的参数均表示为 `""`，不推断数字、布尔值或嵌套结构；没有 query 或没有参数时输出 `{}`。对象字段次序不作为参数原始次序的依据，完整次序见逐项参数行或 `--json` 的 `parameters` 数组。

`--json` 继续输出完整解析报告，不追加 `Query JSON:` 文本。报告中的 `parameters[].has_equals` 仍记录是否存在 `=`。其 `components` 来自 URL 库解析后的规范化结果，`components_are_normalized` 明确标记此语义；`raw_query` 保留原始 query，`parameters` 是有序数组。`--query` 时 `components` 为 null，规范化标记为 false。示例片段：

```json
{"parameters":[{"name":"tag","value":"a","has_equals":true},{"name":"tag","value":"b","has_equals":true}]}
```

无参数时，JSON 的 `parameters` 为空数组。值含换行时可使用 `--json` 保留明确的结构。工具不访问网络。

## HTML/XML 实体

```sh
kito codec html encode '<a title="Kito">你好 & hello</a>'
kito codec html encode '你好 & Kito' --style hex --all
kito codec html decode '&lt;b&gt;你好&#33;&lt;/b&gt;'
kito codec html encode '<tag value="1">' --standard xml --style named
```

`--standard html5|xml` 默认 `html5`。这是**基于 HTML5 实体集合的严格转换**，不复制浏览器 tokenizer 对损坏实体的全部容错行为。HTML5 使用完整命名实体标准表（2125 项，由官方 JSON 通过 `scripts/generate-html-entities.py` 可复现生成，源文件 SHA-256 记录在生成文件中，来源 [WHATWG entities.json](https://html.spec.whatwg.org/entities.json)），支持多个 Unicode 码点，例如 `&NotEqualTilde;` → `≂̸`、`&fjlig;` → `fj`。

编码选项：

- `--style named|decimal|hex` 默认 named，仅改变表示形式。named 优先命名实体，没有名称时使用十进制数字引用；hex 使用大写十六进制数字，例如 `&#x4F60;`。
- `--context text|attribute` 默认 attribute。text 编码 `& < >`；attribute 额外编码双引号、单引号。XML attribute 的 TAB/LF/CR 总是编码为数字引用，保留属性值。
- `--all` 编码全部字符。named 按最长匹配支持多码点实体，同值别名选最短名称、再按字典序；基础五个实体固定小写 `amp/lt/gt/quot/apos`。不承诺保留原拼写。

解码严格要求分号。`&` 后接 ASCII 字母或 `#` 就按实体候选读取，未知实体、缺分号、非法数字、越界、surrogate 或不允许的字符会报错，错误说明零起始 UTF-8 字节位置。其他裸 `&` 保留：`a & b` 保持原样。只解码一层：`&amp;lt;` → `&lt;`。

字符策略同时用于原始输入与解码后的 scalar：

| 范围 | HTML5 严格模式 | XML 1.0 |
| --- | --- | --- |
| NUL、surrogate、超出 U+10FFFF | 拒绝 | 拒绝 |
| C0（TAB/LF/CR 除外）、DEL | 拒绝 | 按 XML Char 范围校验，DEL 允许 |
| TAB/LF/CR | 保留；按编码上下文转换 | 允许；attribute 编码数字引用 |
| C1 U+0080–U+009F | 保留 Unicode scalar；不作历史 Windows-1252 映射 | 允许 |
| Unicode noncharacter | 全部拒绝 | 按 XML Char 范围；U+FFFE/U+FFFF 拒绝，U+FDD0 等允许 |

因此 HTML5 下 `&#128;` 输出 U+0080，**不会**变成 `€`；`&#0;`、`&#xFDD0;`、`&#xFFFF;` 均报错；`ok &unknown;` 报错前不会输出 `ok `。XML 十六进制引用按 CharRef 语法必须使用小写 `x`（`&#X41;` 会报错，HTML5 则允许大写 X）。XML 只允许五个预定义命名实体和数字引用，`&nbsp;` 被拒绝。XML Char 为 TAB/LF/CR、U+0020–U+D7FF、U+E000–U+FFFD、U+10000–U+10FFFF，不解析文档、DTD、外部实体或自定义实体。

这项能力不执行 HTML 清洗，不适用于 JavaScript、CSS 或 URL 上下文的安全编码，也不转换 GBK/UTF-16 等字符集。

## 退出状态

成功为 0，转换/解析/保存失败为 1，clap 参数错误为 2。实现按跨平台 Rust 接口编写；实际验证平台以本次测试记录为准，不以本地通过推断三平台已验证。
