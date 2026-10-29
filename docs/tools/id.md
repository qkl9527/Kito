# Sumi 苏米 · 标识伙伴

苏米是细致的印记师：给内容计算 Hash，给对象生成 UUID。入口为 `kito id`，角色名不作为命令别名。

## 查看命令帮助

在任意层级追加 `-h` 或 `--help`，都可以查看当前命令的帮助。帮助按 `Usage`、使用案例、子命令或参数说明的顺序展示；`id` 总览介绍 Hash 与 UUID，具体操作页面说明算法、格式、数量及输出方式。

```sh
kito id -h
kito id hash --help
kito id uuid --help
```

## 内容 Hash

```sh
# 默认 SHA-256，按 UTF-8 计算文本，不额外加入换行
kito id hash 'hello'

# 文件与 stdin 保留原始字节，包括 BOM、换行和任意二进制内容
kito id hash --file archive.zip --algorithm sha512
printf abc | kito id hash
kito id hash --file - --format base64

# 将摘要保存到新文件，或输出可供脚本使用的元数据
kito id hash --file archive.zip -o archive.sha256
kito id hash --file archive.zip --json
```

`--algorithm` 支持 `md5`、`sha1`、`sha224`、`sha256`、`sha384`、`sha512`，默认 `sha256`。MD5 与 SHA-1 仅用于兼容已有数据，不适用于安全校验用途；本功能不是密码存储方案，也不提供 HMAC。

`--format hex|base64` 默认为小写 `hex`；`base64` 使用 RFC 4648 standard 字母表与填充。普通输出为一个摘要并换行。Hash 按 64 KiB 块读取，不设置编解码工具的 64 MiB 输入上限，也不把整个文件载入内存。

位置文本与 `--file` 互斥。未提供两者时读取非终端 stdin；终端中缺少输入会立即报错。`--file -` 明确选择 stdin。位置文本按 UTF-8 编码，文件与 stdin 不去 BOM、不裁剪空白、不更改行尾。

`--json` 返回以下结构，字节数为字符串，报告不复制输入内容：

```json
{
  "algorithm": "sha256",
  "format": "hex",
  "digest": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
  "input_bytes": "3",
  "source_kind": "text"
}
```

`source_kind` 为 `text`、`file` 或 `stdin`。首版不提供目录递归、校验清单或密码 Hash。

## UUID

```sh
# 默认随机 UUID v4，每行一个
kito id uuid

# UUID v7 含 Unix 毫秒时间字段
kito id uuid --version v7 --count 10

# 选择表示形式并生成 JSON 或文件
kito id uuid --version v4 --format simple --json
kito id uuid --format urn --count 20 -o ids.txt
```

`--version v4|v7` 默认 `v4`。`--format hyphenated|simple|urn` 默认为小写、带连字符的 `hyphenated`，例如 `550e8400-e29b-41d4-a716-446655440000`。`simple` 去掉连字符；`urn` 增加 `urn:uuid:` 前缀。

`--count` 默认为 1，允许 1–10000；每行一个 UUID，最后一行也带换行。`--json` 返回 `version`、`format` 与 `values` 数组，即使只生成一个值也保留数组。

随机数据来自操作系统随机源，获取失败时返回错误，不回退到固定种子。v7 的前 48 位为 Unix 毫秒；系统时间早于 Unix epoch 或超出字段范围时返回错误。v4 不依赖系统时间。

UUID 不保证绝对不重复，不能作为密钥或授权凭证。v7 不承诺同毫秒内、多进程间或时钟回拨时全局严格递增。首版不支持自定义时间戳、命名空间或 v1/v3/v5。

## 输出与错误

Hash 与 UUID 均支持 `-o/--output PATH`，默认拒绝覆盖已有文件，显式加 `--overwrite` 才允许替换。Hash 始终拒绝将输出写回输入文件，包括指向同一文件的链接；候选结果成功后才原子发布。`--json` 与 `--output` 互斥。

Hash 读取失败、UUID 随机源失败或系统时间错误都会在输出前终止；UUID 整批生成完毕才写出，不输出半批结果。stdout 下游提前关闭按正常 BrokenPipe 处理。

成功退出码为 0，运行错误为 1，参数用法错误沿用 clap 的 2。实际平台验证结果以本次验证报告为准，不将单机通过描述为三平台已验证。
