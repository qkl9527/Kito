# 文本工具

[返回总入口](../../README.md) · [图片工具](image.md) · [时间工具](time.md)

文本工具提供字符数、UTF-8 字节数统计、按行去重和大小写转换。统计字符串时直接传入位置参数；统计文件时必须通过 `--file PATH` 指定。文件必须是有效的 UTF-8 文本。

`kito text --help` 展示各项能力的入门案例，`kito text count --help`、`kito text dedupe --help` 和 `kito text case --help` 展示对应命令的具体案例与参数说明。各层级也支持 `-h`；案例紧接在 `Usage` 后、参数列表前。

## 命令速查

| 命令 | 用途 |
| --- | --- |
| `kito text count TEXT` | 按 Unicode 字符统计字符串 |
| `kito text count TEXT --bytes` | 按 UTF-8 字节统计字符串 |
| `kito text count --file PATH` | 按 Unicode 字符统计 UTF-8 文本文件 |
| `kito text count --file PATH --bytes` | 按 UTF-8 字节统计文件 |
| `kito text dedupe TEXT` | 对字符串逐行去重并输出到终端 |
| `kito text dedupe --file PATH` | 对 UTF-8 文本文件逐行去重并输出到终端 |
| `kito text dedupe --file PATH -o OUTPUT` | 去重后写入文件 |
| `kito text case upper TEXT` | 全部转为大写 |
| `kito text case lower TEXT` | 全部转为小写 |
| `kito text case capitalize TEXT` | 仅将首字符转为大写 |
| `kito text case capitalize --words TEXT` | 每个单词首字符大写，其余转为小写 |

## 示例

```sh
kito text count '你好，Kito'
# 7

kito text count '你好，Kito' --bytes
# 13

kito text count --file README.md
kito text count --file README.md --bytes
```

默认模式统计 Unicode scalar value（Rust `char`）：ASCII 字母、汉字以及常见 emoji 各计 1，不受 UTF-8 编码占用字节数影响。它不等同于用户感知的字素簇计数；例如由基础字符与组合符号组成的显示字符会按多个 Unicode scalar value 计数。

`--bytes` 统计字符串的 UTF-8 编码长度。文件模式只读取 UTF-8 文本；若文件不是有效 UTF-8，会报错，不会替换或忽略无效字节。命令仅输出计数数字，适合被 shell 脚本接收。

## 按行去重

```sh
# 字符串输入：保留第一次出现的行，结果默认输出到终端
kito text dedupe $'苹果\n香蕉\n苹果'
# 苹果
# 香蕉

# 文件输入：结果输出到终端
kito text dedupe --file input.txt

# 保存到新文件；输出文件已存在时默认报错
kito text dedupe --file input.txt --output unique.txt

# 确认需要替换已有输出文件时显式覆盖
kito text dedupe --file input.txt -o unique.txt --overwrite
```

去重按整行内容比较，保留首次出现的行及其顺序；空行也参与去重。LF 与 CRLF 行尾在比较时视为相同，输出保留首次出现时的行尾形式和原文件末尾是否有换行符。输出到终端时不额外添加换行，输出到文件时默认不覆盖已有文件；输入和输出指向同一个文件时会报错。

## 大小写转换

```sh
kito text case upper 'Hello, 世界!'
# HELLO, 世界!

kito text case lower 'Hello, WORLD!'
# hello, world!

# 只转换首字符，后续字符原样保留
kito text case capitalize 'hello WORLD'
# Hello WORLD

# 每个空白分隔的单词首字符大写，其余转为小写
kito text case capitalize --words 'hello WORLD from KITO'
# Hello World From Kito

# 文件输入并保存到新文件；默认不覆盖已有文件
kito text case upper --file input.txt --output upper.txt
kito text case lower --file input.txt -o lower.txt --overwrite
```

三种模式均支持字符串位置参数或 `--file PATH`，默认直接输出转换后的文本到终端，也可用 `-o/--output PATH` 写文件。`upper` 和 `lower` 使用 Unicode 大小写映射；`capitalize` 默认只处理首个 Unicode 字符，其余内容不变。增加 `--words` 后按空白分隔单词，每个词首字符转大写、其余字符转小写，词间空白保持原样。写入文件时默认不覆盖已有文件，需覆盖时显式指定 `--overwrite`；输入和输出指向同一文件会报错。
