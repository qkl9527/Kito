# 时间工具 · Tiko 提可

[返回总入口](../../README.md) · [图片工具](image.md)

Tiko 提可 · 时间伙伴（时刻向导）负责时间戳与日期时间互转、跨时区转换和全球时区查询，入口为 `kito time`。安装与构建方式见[总入口](../../README.md#构建与运行)。

## 命令速查

| 命令 | 用途 |
| --- | --- |
| `kito time now` | 查看当前日期时间，或输出指定单位的时间戳 |
| `kito time from-timestamp VALUE` | 秒、毫秒、微秒、纳秒时间戳转日期时间 |
| `kito time to-timestamp DATETIME` | 日期时间转指定单位的时间戳 |
| `kito time convert DATETIME` | 转换到目标时区，省略目标时使用默认时区 |
| `kito time zones [FILTER]` | 查看或筛选全球时区名称、偏移和缩写 |
| `kito time timezone set/get/reset` | 保存、查看和清除默认时区 |

每一级都支持 `-h` / `--help`：先用 `kito time -h` 查看入口，再用 `kito time convert --help` 查看具体能力；配置命令可继续进入 `kito time timezone -h` 和 `kito time timezone set --help`。帮助中的 `Usage` 后紧跟可复制的使用案例，再列出子命令、位置参数和选项说明；案例会展示实际参数值、时区方向和输出方式。

## 本文导航

- [基本使用](#基本使用)
- [输出方式](#输出方式)
- [自定义日期输出格式](#自定义日期输出格式)
- [全球时区查询](#全球时区查询)
- [默认时区配置](#默认时区配置)
- [日期输入与夏令时](#日期输入与夏令时)
- [精度与边界](#精度与边界)
- [JSON 输出示例](#json-输出示例)

## 基本使用

时间工具内置 IANA 时区数据库，可离线使用，无需 ImageMagick。通过 `kito time --help` 查看命令。

```sh
# 当前时间；指定单位后，主要结果变为对应时间戳
kito time now --timezone Asia/Shanghai
kito time now --unit ms --value-only

# 时间戳转日期时间：指定毫秒，也可用 auto 识别常见位数
kito time from-timestamp 1704067200123 --unit ms --timezone Asia/Shanghai
kito time from-timestamp 1704067200123456 --timezone Asia/Shanghai --value-only
# 2024-01-01T08:00:00.123456+08:00

# 日期时间转时间戳：不带 offset 的输入由 --timezone 解释
kito time to-timestamp '2024-01-01 08:00:00.123456' --timezone Asia/Shanghai --unit us --value-only
# 1704067200123456

# 直接跨时区转换；也可输入自带 offset 的 RFC 3339 时间
kito time convert '2024-01-01 08:00:00' --from-timezone Asia/Shanghai --to-timezone America/New_York --value-only
# 2023-12-31T19:00:00-05:00
kito time convert '2024-01-01T00:00:00Z' --to-timezone +05:45 --json
```

## 输出方式

默认输出适合阅读的时间、时区和四种精度时间戳。`--value-only` 只输出一个转换结果，不附加标签；`--json` 输出结构化信息，两者互斥。`now --value-only` 返回日期时间，加 `--unit` 后返回时间戳；`to-timestamp` 默认输出秒，其他转换命令的主要结果为日期时间。

## 自定义日期输出格式

`now`、`from-timestamp`、`to-timestamp`、`convert` 均支持 `--format PATTERN`（简写 `-f`），采用 `strftime` 占位符；包含空格的模板需用引号包裹。

```sh
# 空格分隔，显示 6 位小数和时区
kito time from-timestamp 1704067200123456 --timezone Asia/Shanghai \
  --format '%Y-%m-%d %H:%M:%S%.6f%:z' --value-only
# 2024-01-01 08:00:00.123456+08:00

# 省略时区显示
kito time from-timestamp 1704067200123456 --timezone Asia/Shanghai \
  --format '%Y-%m-%d %H:%M:%S%.6f' --value-only
# 2024-01-01 08:00:00.123456

# 调整分隔符，只显示到秒；也可搭配 --json
kito time convert '2024-01-01T00:00:00.123456Z' --to-timezone Asia/Shanghai \
  -f '%Y/%m/%d %H:%M:%S' --json
kito time now --format '%Y-%m-%d %H:%M:%S' --value-only
```

| 模板 | 含义 / 示例 |
| --- | --- |
| `%Y-%m-%d` 或 `%F` | 年月日，如 `2024-01-01` |
| `%H:%M:%S` 或 `%T` | 24 小时制时分秒，如 `08:00:00` |
| `%.f` | 自动显示 0、3、6 或 9 位小数，保留非零子秒精度 |
| `%.3f` / `%.6f` / `%.9f` | 固定 3 / 6 / 9 位小数，包含小数点；不足补零，多余截去 |
| `%:z` | 分钟精度的时区偏移，如 `+08:00`；UTC 为 `+00:00` |
| `%::z` | 包含秒的时区偏移，如 `+08:05:43`，适合历史时区 |
| `%Z` | 当前时刻的时区缩写，如 `CST`、`EST`、`EDT`；固定偏移时显示偏移 |
| `%%` | 字面量 `%` |

未指定 `--format` 时沿用原有带 offset 的默认格式。自定义格式作用于 `datetime` 和日期类型的 `value`；JSON 额外包含 `format` 模板，`utc`、`timestamps` 和时区元数据仍保留完整值。`to-timestamp` 以及指定 `--unit` 的 `now` 仍以时间戳为主要结果，模板只影响报告中的日期显示。

省略时区或降低小数位数只改变展示；完整时刻可从 `utc` / `timestamps.ns` 获取。`precision_lost` 仍仅表示时间戳单位换算的取整，格式选择不修改该字段。`--format` 不改变输入解析规则，也不保存为默认配置。空白模板、非法格式和仅用于解析的 `%#z` 会报错。完整语法见 [Chrono strftime 文档](https://docs.rs/chrono/0.4.45/chrono/format/strftime/index.html)，本机验证见 [自定义格式验证记录](../validation/2026-10-28-time-format.md)。

## 全球时区查询

不知道时区名时，先查询再将名称复制到 `--timezone` 或 `--to-timezone`。

```sh
kito time zones                              # 完整列表：IANA 名称、UTC 偏移、缩写
kito time zones Shanghai                     # 按名称搜索，忽略大小写
kito time zones America --value-only         # 只输出名称，每行一个
kito time zones +05:45 --json                 # 也可搜索偏移或缩写
kito time zones --value-only -- -05:00        # 负偏移放在 -- 后面
kito time zones New_York --at '2024-07-01T00:00:00Z' --json
```

列表包含全球 IANA 区域与数据库别名，按名称排序；本次锁定的数据库为 **2025b**、共 **597** 个名称。输出标明数据库版本和统一的参考 UTC 时刻，偏移按该时刻的夏令时及历史规则计算。默认参考现在；`--at` 必须自带 offset。名称、偏移、缩写均可搜索；`CST` 等缩写可能匹配多个地区，应从结果中选择完整 IANA 名称。无匹配项时 JSON 的 `zones` 为 `[]`，单值模式不输出内容。时区规则来自构建时内置的数据，更新 chrono-tz 并重新构建后才会更新数据库。

## 默认时区配置

一次配置后，各次命令和新开的终端都可直接使用。

```sh
kito time timezone set Asia/Shanghai
kito time timezone get                       # 查看当前生效的默认值和来源
kito time timezone get --value-only          # 只输出时区名
kito time timezone get --json

# 不传时区时，使用保存的默认值
kito time from-timestamp 1704067200 --value-only
# 2024-01-01T08:00:00+08:00
kito time to-timestamp '2024-01-01 08:00:00' --value-only
# 1704067200
kito time convert '2024-01-01T00:00:00Z' --value-only
# 2024-01-01T08:00:00+08:00

# 单次覆盖默认值
kito time from-timestamp 1704067200 --timezone UTC --value-only

# 清除保存项；回退到 KITO_TIMEZONE 或 UTC
kito time timezone reset
```

优先级为 **显式参数 > `KITO_TIMEZONE` > 保存的配置 > UTC**，不读取机器本地时区，也不修改系统设置。`convert` 的源时区参数为 `--from-timezone`，目标参数为 `--to-timezone`；两者省略时都可使用默认值，但输入自带 offset 时优先用它解释源时间。

仍可用 `export KITO_TIMEZONE=Europe/London`（macOS/Linux）或 `$env:KITO_TIMEZONE = 'Europe/London'`（PowerShell）临时覆盖保存值；要恢复使用保存值，分别用 `unset KITO_TIMEZONE` 或 `Remove-Item Env:KITO_TIMEZONE`。IANA 名称保留夏令时规则，固定偏移如 `+08:00`、`-03:30` 全年不变。

### 配置文件

配置文件采用用户目录，所有工作目录共用，不必改 shell 配置：

| 平台 | 默认配置文件 |
| --- | --- |
| macOS | `~/Library/Application Support/kito/time.json` |
| Linux | `$XDG_CONFIG_HOME/kito/time.json`，未设置时为 `~/.config/kito/time.json` |
| Windows | `%APPDATA%\kito\time.json` |

也可用 `KITO_CONFIG_DIR` 指定独立的**绝对目录**，文件名仍为 `time.json`。保存内容如 `{"timezone":"Asia/Shanghai"}`；`set/reset` 仅更改 `timezone` 字段，保留其他配置。非法时区不会落盘，写入采用同目录临时文件原子替换。只读查询不会创建缺失的文件；损坏的 JSON、无效保存值或不可读文件会明确报错，显式参数和 `KITO_TIMEZONE` 仍可覆盖。

`timezone` 的三个命令均支持 `--value-only` 和 `--json`。`set` 报告保存值；`get` 报告当前生效值，JSON 的 `source` 为 `environment`、`config` 或 `fallback`；`reset` 只清除保存项，不修改环境变量，单值输出 `reset`。JSON 同时包含 `action`、`timezone`、`source`、`config_path`；reset 的 `timezone/source` 为 `null`，环境覆盖生效时不读取文件，`config_path` 为 `null`。

精简环境中缺少用户目录变量时，普通转换仍可使用 UTC，`get` 的 `config_path` 为 `null`；保存或清除配置需要先设置 `KITO_CONFIG_DIR`。其他配置字段按原始 JSON 值保留，包括超大整数及高精度小数。默认时区配置的验证结果见 [持久化配置验证记录](../validation/2026-10-28-timezone-config.md)。

## 日期输入与夏令时

输入日期自带 offset 时优先使用该 offset，不受环境默认影响；如果同时提供显式源时区，则两者必须一致。无 offset 的输入支持 `YYYY-MM-DD HH:MM:SS[.小数秒]` 或 T 分隔形式。夏令时跳变中不存在的本地时间、回拨时有两个解释的本地时间均报错；回拨时可附上明确 offset 消除歧义，例如 `2024-11-03T01:30:00-04:00` 或 `2024-11-03T01:30:00-05:00`。

## 精度与边界

- `--unit s/ms/us/ns` 分别表示秒、毫秒、微秒、纳秒，也接受 `seconds/milliseconds/microseconds/nanoseconds`。时间戳接受带正负号的整数。
- `from-timestamp` 默认 `auto`：忽略符号和前导零，1–10 位按秒、13 位按毫秒、16 位按微秒、19 位按纳秒；其他位数要求显式单位。位数无法证明真实单位，历史日期或接近 Unix 纪元的小值应明确指定，例如 `-1 --unit ms`。
- 日期小数最多 9 位；全流程使用整数计算，支持超出 i64 范围的纳秒时间戳。JSON 的 `value` 和 `timestamps.s/ms/us/ns` 都是字符串，避免 JavaScript 大整数精度损失。
- `to-timestamp` 和指定 `--unit` 的 `now` 在降低精度时向负无穷取整，报告 `precision_lost: true`。例如纪元前 1 纳秒转毫秒为 `-1`；JSON 仍保留完整 UTC 日期与纳秒。单值模式仅保留结果，不附带精度说明。
- 日期年份范围为 0000–9999，输入、UTC 以及显示的目标日期均须在范围内；拒绝无法表示的值和闰秒 `:60`。Unix 时间戳不包含闰秒计数。
- 默认日期输出包含明确 offset，UTC 使用 `Z`；指定 `--format` 时按模板显示。默认格式完整保留历史规则中的秒级 offset（例如 `+08:05:43`），作为 RFC 3339 之外的扩展也可重新输入；自定义模板需要保留秒级 offset 时使用 `%::z`。

## JSON 输出示例

例如 `kito time from-timestamp 1704067200123 --timezone Asia/Shanghai --json` 输出：

```json
{"operation":"from-timestamp","value":"2024-01-01T08:00:00.123+08:00","datetime":"2024-01-01T08:00:00.123+08:00","timezone":"Asia/Shanghai","utc":"2024-01-01T00:00:00.123Z","utc_offset":"+08:00","abbreviation":"CST","timestamps":{"s":"1704067200","ms":"1704067200123","us":"1704067200123000","ns":"1704067200123000000"},"unit":"ms","precision_lost":false}
```

`zones --json` 的字段为 `reference_utc`、`tzdb_version`、`zones`；每个条目包含 `name`、`utc_offset`、`offset_seconds` 和 `abbreviation`。具体验证环境及结果见 [时间工具验证报告](../validation/2026-10-28-time-tool.md)。
