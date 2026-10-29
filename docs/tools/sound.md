# Echo 艾可 · 声音伙伴

[返回总入口](../../README.md) · [视频工具](video.md)

Echo 艾可 · 声音伙伴（音效师）负责从零生成提示音、游戏音效、短旋律、和弦与鼓点，入口为 `kito sound`，没有角色名别名。用预设即可开始，也可以逐步调整音高、节拍与音色。

## 60 秒入门

在一个新目录中运行以下命令。生成命令会保存文件并报告结果，不会自动播放；使用系统播放器打开 WAV 即可试听。

```sh
# 看看有哪些声音可以直接使用
kito sound presets

# 一个完成提示音，保存到 success.wav
kito sound sfx success

# 用四个音符写一句旋律，保存到 hello.wav
kito sound melody 'C4 E4 G4 C5:2' -o hello.wav

# 四小节鼓点，保存到 drums.wav
kito sound beat basic --bpm 100 -o drums.wav

# 只检查参数和目标路径，暂不生成文件
kito sound melody 'C4+E4+G4:2 R:1 G4:1' -o chord.wav --dry-run
```

同名文件已存在时会报错。再次尝试可以换一个 `-o` 文件名；确定要替换已有结果时加 `--overwrite`。

## 适合做什么

| 想完成的事 | 使用方式 |
| --- | --- |
| 给应用按钮、通知或完成状态配声音 | `sound sfx notify`、`success`、`error` |
| 为游戏原型准备金币、跳跃、激光等音效 | `sound sfx coin`、`jump`、`laser` 等 |
| 生成单音、测试音或练习辨认音高 | `sound tone A4` 或 `sound tone 440Hz` |
| 写短旋律、和弦或音乐草稿 | `sound melody 'C4 E4 G4'` |
| 做节奏练习、鼓点或短循环素材 | `sound beat basic` |
| 把旋律交给 DAW 继续编曲 | `sound melody 'C4 E4 G4' -o sketch.mid` |

声音伙伴只创作新声音，不读取现有音频来剪辑、混音、降噪或提取人声，也不提供录音、实时演奏、自动作曲或真实乐器采样库。现有音频的转码、裁剪、拼接、混音、响度调整与视频音轨处理继续使用 `kito video audio`，详见[视频工具](video.md)。例如先由 `sound` 生成 WAV，再将这份素材交给视频伙伴做后续处理。

## 依赖与平台

声音合成和 MIDI 编码内置于 Rust 程序，可离线使用，不需要 Node.js、浏览器、Tone.js、FFmpeg 或音频设备，也没有需要配置的后端路径变量。安装与构建方式见[总入口](../../README.md#构建与运行)。

实现面向 macOS、Linux、Windows；本地验证通过不代表三个平台均已验证。本文的单行 `kito` 示例适用于常见 shell；带 `printf`、循环或 here-document 的脚本标明为 macOS/Linux 用法，并另给出 PowerShell 示例。

## 命令和共同参数

每一级都可追加 `-h` 或 `--help`。例如先运行 `kito sound --help`，再运行 `kito sound melody --help`。

| 命令 | 必要输入 | 默认输出 |
| --- | --- | --- |
| `sound presets` | 无 | 终端中的预设清单 |
| `sound sfx PRESET` | 音效名称 | `<preset>.wav` |
| `sound tone [PITCH]` | 可省略，默认 `A4` | `tone.wav` |
| `sound melody [SCORE]` | 乐谱文本、文件或 stdin | `melody.wav` |
| `sound beat [PRESET]` | 可省略，默认 `basic` | `beat.wav` |

四个生成命令都有以下参数；`presets` 仅支持 `--json` 和帮助选项。

| 参数 | 默认值 | 含义与边界 |
| --- | --- | --- |
| `-o, --output PATH` | 见上表，位于当前目录 | 保存到有路径的文件；不支持二进制 stdout。`melody` 可选 `.wav`、`.mid`、`.midi`，其他命令只接受 `.wav` |
| `--overwrite` | 关闭 | 允许替换已有普通结果文件，仍不能覆盖乐谱输入 |
| `--dry-run` | 关闭 | 完成输入、资源限制与目标路径校验，输出计划报告；不合成，不创建目录或文件 |
| `--json` | 关闭 | stdout 只输出一个 JSON 对象，便于脚本读取 |
| `--sample-rate RATE` | `44100` | WAV 采样率，单位 Hz，只接受 `22050`、`44100`、`48000` |
| `--volume VALUE` | `0.7` | WAV 总振幅，范围 `0..1`，含端点；`0` 生成静音。它不是 dB，也不代表声压级 |
| `--seed NUMBER` | `0` | WAV 合成中噪声的随机种子，无符号 64 位整数，范围 `0..18446744073709551615` |

输出 MIDI 时不得显式传入 `--sample-rate`、`--volume`、`--seed` 或 `--instrument`，即使提供的是默认值也会报错。详见 [WAV 与 MIDI](#wav-与-midi)。所有数值须有限；`NaN`、`Infinity`、超范围值均不作为有效参数。

固定种子让含噪声的音效和鼓声在同一构建中便于复现。只有使用噪声的声音才会受 `--seed` 影响；改变种子不等于随机创作旋律。结果不承诺跨 CPU、架构或程序版本位级完全相同。

## presets：先选声音

```sh
kito sound presets
kito sound presets --json
```

清单包含 8 个音效、5 个乐器和 3 个鼓型。名称使用英文，说明使用中文，便于复制到命令中。

### 8 个音效

| 名称 | 适用场景 / 听感方向 | 默认时长（秒） |
| --- | --- | ---: |
| `notify` | 简短提醒、普通通知 | 0.45 |
| `success` | 完成、确认或成功状态 | 0.9 |
| `error` | 失败或错误状态 | 0.65 |
| `coin` | 金币、拾取或积分反馈 | 0.4 |
| `jump` | 跳跃、弹起等动作反馈 | 0.45 |
| `laser` | 激光、发射等科幻反馈 | 0.35 |
| `explosion` | 爆炸或低沉冲击，包含噪声 | 1.2 |
| `whoosh` | 掠过、滑动或转场，包含噪声 | 0.8 |

这些名称描述合成音效的用途，不表示真实环境录音。

### 5 个乐器

| 名称 | 特点与适用场景 |
| --- | --- |
| `sine` | 简单、干净的正弦音色，适合单音和音高练习；`tone` 默认值 |
| `soft` | 柔和的合成音色，适合初次尝试旋律与和弦；`melody` 默认值 |
| `bell` | 铃音风格的合成音色，适合短提示、点缀或清亮旋律 |
| `pluck` | 拨弦风格的合成音色，适合短促、有颗粒感的音符 |
| `chip` | 复古电子游戏风格的合成音色，适合像素风旋律 |

乐器是合成音色预设，不是钢琴、钟或吉他的实录采样。可以用同一段乐谱分别生成文件，比较自己喜欢的音色：

```sh
kito sound melody 'C4 E4 G4 C5:2' --instrument bell -o bell.wav
kito sound melody 'C4 E4 G4 C5:2' --instrument pluck -o pluck.wav
kito sound melody 'C4 E4 G4 C5:2' --instrument chip -o chip.wav
```

### 3 个鼓型

每个模式都是一小节 16 格：`x` 表示击打，`-` 表示休止。三条轨道同时播放。

| 鼓型 | `kick` 底鼓 | `snare` 军鼓 | `hat` 踩镲 |
| --- | --- | --- | --- |
| `basic` | `x---x---x---x---` | `----x-------x---` | `x-x-x-x-x-x-x-x-` |
| `house` | `x---x---x---x---` | `----x-------x---` | `--x---x---x---x-` |
| `hiphop` | `x-----x-x-------` | `----x-------x---` | `x-x-x-x-x-x-x-x-` |

`basic` 提供均匀的基础节奏；`house` 在反拍加入踩镲；`hiphop` 的底鼓分布更有错落感。它们是便于起步的节奏模板，不自动生成完整伴奏。

## sfx：生成场景音效

```sh
kito sound sfx notify
kito sound sfx coin --duration 0.6 --pitch 5 -o coin-high.wav
kito sound sfx laser --pitch=-12 -o laser-low.wav
kito sound sfx explosion --seed 42 --volume 0.5 -o boom.wav
```

| 专用参数 | 默认值 | 含义与范围 |
| --- | --- | --- |
| `PRESET` | 必填 | 上表中的 8 个音效之一 |
| `--duration SECONDS` | 各预设默认时长 | 总时长，单位秒，范围 `0.05..10`，含端点 |
| `--pitch SEMITONES` | `0` | 音高偏移，单位半音，范围 `-24..24`，含端点；`12` 为升高一个八度，`-12` 为降低一个八度 |

用 `--duration` 调整音效时长，用 `--pitch` 调整音高；音高偏移不改变噪声型声部。为避免 shell 将负数误认为选项，可以写成 `--pitch=-12`。`sfx` 不接受 `--instrument`；音色由场景预设决定。采样率和音高共同影响可合成频率，超出当前采样率可表达范围时会报错。

## tone：生成单音

```sh
# 默认 A4，440 Hz，1 秒，sine 音色
kito sound tone

kito sound tone Bb3 --duration 2 --instrument soft -o low-note.wav
kito sound tone 440Hz --duration 0.5 -o tuning.wav
kito sound tone 660 --duration 1 --instrument bell -o high-note.wav
```

| 专用参数 | 默认值 | 含义与范围 |
| --- | --- | --- |
| `PITCH` | `A4` | 音名，如 `C4`、`F#4`、`Bb3`；或正数频率，如 `440`、`440Hz` |
| `--duration SECONDS` | `1` | 总时长，单位秒，范围 `0.01..120`，含端点 |
| `--instrument NAME` | `sine` | `sine`、`soft`、`bell`、`pluck`、`chip` |

音名使用 12 平均律，`A4 = 440 Hz`。频率必须为正数，且不超过当前采样率的 `0.45` 倍，为 Nyquist 临界值留出余量。例如 `44100 Hz` 采样率的上限为 `19845 Hz`；不能用 `0Hz` 表示休止。生成静音可用 `--volume 0`。

## melody：写旋律与和弦

```sh
kito sound melody 'C4 D4 E4 G4:2' --bpm 120 -o tune.wav
kito sound melody 'C4+E4+G4:2 R:1 G4:1' --instrument soft -o chords.wav
kito sound melody 'C4:0.5 E4:0.5 G4:1 | D4:0.5 F4:0.5 A4:1' \
  --bpm 100 --repeat 2 -o phrase.wav
kito sound melody 'C4 E4 G4 C5:2' -o sketch.mid
```

| 专用参数 | 默认值 | 含义与范围 |
| --- | --- | --- |
| `SCORE` | 无 | 用引号包裹的乐谱，与 `--file` 互斥；省略时尝试从非终端 stdin 读取 |
| `--file PATH` | 无 | 读取 UTF-8 乐谱文件；`--file -` 明确读取 stdin |
| `--bpm NUMBER` | `120` | 每分钟拍数，整数，范围 `20..400`，含端点 |
| `--instrument NAME` | `soft` | 5 个乐器之一，只用于 WAV |
| `--repeat NUMBER` | `1` | 整段乐谱重复次数，范围 `1..64`，含端点 |

### 看懂音名、拍、和弦与休止

音名由字母、可选升降号和八度数组成。`C D E F G A B` 对应 do、re、mi、fa、sol、la、si；八度数越大，音越高。例如 `C4` 是中央 C，`C5` 比它高一个八度。`#` 升高半音，`b` 降低半音，因此 `F#4` 与 `Gb4` 对应相同音高。使用 ASCII 的 `#` 和小写 `b`，不使用 `♯`、`♭` 等排版符号；不支持重升号或重降号。

乐谱接受 MIDI 音高 `0..127` 对应的音名范围（从 `C-1` 到 `G9`）。这里的 MIDI 数字用于说明边界，乐谱本身写 `C4` 等音名；`440Hz` 这样的频率写法只用于 `tone`。

| 乐谱片段 | 含义 |
| --- | --- |
| `C4` | 播放 C4，一拍 |
| `C4:2` | 播放 C4，两拍 |
| `C4:0.5` | 播放 C4，半拍 |
| `C4 E4 G4` | 按顺序播放三个音，每个一拍 |
| `C4+E4+G4:2` | 同时播放三个音组成和弦，持续两拍 |
| `R` 或 `R:1` | 休止一拍，保留这段静音时间 |
| `R:0.5` | 休止半拍 |
| `C4 E4 | G4 C5` | `|` 仅帮助阅读，不额外添加拍数或强制对齐小节 |

事件之间用空白分隔，支持跨行；`|` 必须独立成项，即两侧留空白。不要在 `C4+E4+G4` 的加号前后插入空格，也不要使用逗号分隔。拍数使用十进制数字，例如半拍写 `0.5`，不写 `1/2`。文件不支持注释或完整五线谱语法。

`BPM` 决定一拍多长：一拍秒数为 `60 / BPM`。在 `120 BPM` 下，一拍是 `0.5` 秒，两拍是 `1` 秒。`C4 E4 G4 C5:2` 共 5 拍，因此长 `2.5` 秒；`--repeat 2` 得到 10 拍、`5` 秒。和弦中的多个音同时发声，不把拍数相加。

### 乐谱限制

- 单个事件拍数范围为 `0.0625..64`（即 `1/16..64` 拍），含端点；每拍固定 `480 ticks`，拍数乘以 480 必须能表示为整 tick，不会静默四舍五入。例如 `0.5` 是 240 ticks，`0.1` 是 48 ticks，而 `0.333` 不是整 tick，会报错。
- 一个和弦最多 8 个音，不能包含相同音高的重复音，包括 `F#4+Gb4` 这样的等音写法；计入重复后，最多 4096 个发声音符。一个三音和弦计作 3 个音符，休止符不计作发声音符。
- 计入休止和重复后，总时长最多 120 秒。较低 BPM、较多拍数或较高重复次数可能触及此上限。
- WAV 的所有声部累计渲染帧数最多为 `64,000,000`。长和弦中每个同时发声的音都需要独立计算，因此即使最终文件不足 120 秒，也可能触及这项资源限制；可以缩短乐谱、减少和弦音数或降低采样率。
- 允许只有休止符的乐谱，例如 `R:4`，用于生成有明确时长的静音。空文件或空白乐谱不是有效乐谱。
- 音名范围和 WAV 可渲染范围分别检查；合法 MIDI 音高在低采样率 WAV 下仍可能超出 `0.45 × 采样率` 的频率上限。

### 从文件和 stdin 读取

乐谱输入上限为 `64 KiB`（65,536 字节），位置参数、文件和 stdin 都受此限制。文件与 stdin 按原始字节读取，再严格验证 UTF-8；不会隐式去掉 BOM 或更改换行。空格、TAB、换行可用作事件分隔符；BOM 不是音符的一部分，请保存为不带 BOM 的 UTF-8 文本。

macOS/Linux：

```sh
# 跨行乐谱与写在同一行效果相同
cat > tune.txt <<'SCORE'
C4:0.5 E4:0.5 G4:1
C4+E4+G4:2 R:1
SCORE

kito sound melody --file tune.txt --bpm 110 -o from-file.wav
printf '%s\n' 'C4 E4 G4 C5:2' | kito sound melody --file - -o from-stdin.wav
printf '%s\n' 'C4 E4 G4 C5:2' | kito sound melody -o from-pipe.mid
```

PowerShell：

```powershell
[System.IO.File]::WriteAllText('tune.txt', 'C4 E4 G4 C5:2', [System.Text.UTF8Encoding]::new($false))
kito sound melody --file tune.txt -o from-file.wav
'C4 E4 G4 C5:2' | kito sound melody --file - -o from-stdin.wav
```

没有位置参数、没有文件且 stdin 是交互终端时，会立即说明缺少输入，不等待用户逐行录入。位置参数和 `--file` 不能同时使用。

## beat：生成鼓点

```sh
kito sound beat
kito sound beat house --bpm 124 --bars 2 -o house.wav
kito sound beat hiphop --bpm 90 --bars 4 -o hiphop.wav

# 保留 basic 的底鼓与军鼓，只替换踩镲轨
kito sound beat basic --hat 'xxxxxxxxxxxxxxxx' --bars 2 -o busy-hat.wav

# 使用自定义底鼓，关闭军鼓与踩镲
kito sound beat --kick 'x---x-x---x-x---' \
  --snare '----------------' --hat '----------------' --bars 1 -o kick.wav
```

| 专用参数 | 默认值 | 含义与范围 |
| --- | --- | --- |
| `PRESET` | `basic` | `basic`、`house`、`hiphop` |
| `--kick PATTERN` | 所选鼓型的底鼓轨 | 替换底鼓轨，必须正好 16 个字符 |
| `--snare PATTERN` | 所选鼓型的军鼓轨 | 替换军鼓轨，必须正好 16 个字符 |
| `--hat PATTERN` | 所选鼓型的踩镲轨 | 替换踩镲轨，必须正好 16 个字符 |
| `--bpm NUMBER` | `120` | 每分钟拍数，整数，范围 `20..400`，含端点 |
| `--bars NUMBER` | `4` | 小节数，范围 `1..64`，含端点，同时受 120 秒总时长上限约束 |

每小节固定 4/4，16 格各占四分之一拍，即一个十六分音符。字符 `x` 或 `X` 都表示一次击打，`-` 或 `.` 表示休止；大小写不会改变力度。模式内部不接受空格、斜杠或其他符号，不足或超过 16 格都会报错。

指定某条轨时只替换这一条，未指定的轨道继续使用所选鼓型。关闭一条轨可传入 16 个 `-`，例如 `--hat '----------------'`。`--bars` 将同一个小节重复指定次数，不会自动加入过门。鼓点时长为 `小节数 × 4 × 60 / BPM`；`120 BPM` 的四小节为 `8` 秒。末尾不追加混响或鼓声尾音，方便将片段按明确长度使用。

## WAV 与 MIDI

只有 `melody` 支持 MIDI。输出扩展名决定格式，不需要额外的 `--format`：

```sh
# 渲染为可以直接播放的声音
kito sound melody 'C4 E4 G4 C5:2' --instrument bell -o song.wav

# 保存音符和节拍，供 DAW 或 MIDI 播放器继续使用
kito sound melody 'C4 E4 G4 C5:2' -o song.mid
kito sound melody 'C4+E4+G4:4 R:1' -o chord.midi
```

| 比较项 | WAV | MIDI |
| --- | --- | --- |
| 保存内容 | 已合成的音频采样 | 音符、起止位置、速度与时长等演奏信息 |
| 编码规格 | 单声道、16-bit PCM，支持三种采样率 | Standard MIDI File type 0，单轨，480 ticks/beat |
| 声音来源 | Kito 的合成乐器或音效预设 | 播放器或 DAW 使用自己的音源 |
| 音色控制 | `--instrument` 选择合成音色 | 固定 program 0、velocity 100，不保存 Kito 合成音色 |
| 适合场景 | 应用、游戏和视频中的实际声音素材 | 修改音符、替换音源、继续编曲 |

MIDI 文件不是 WAV 的另一种压缩方式，也不包含渲染后的声音。即使乐谱相同，MIDI 在不同播放器中的听感也可能不同；program 0 在常见 General MIDI 音源中通常对应钢琴，但最终取决于播放器。为避免参数被静默忽略，输出 `.mid` / `.midi` 时显式提供 `--instrument`、`--sample-rate`、`--volume`、`--seed` 都会失败。`--bpm`、`--repeat`、文件/stdin 输入与安全输出参数仍可使用。

MIDI 的速度以每拍整数微秒存储，不能整除时取最近微秒。报告中的 MIDI 时长按请求的 BPM 计算，因此文件的实际时间安排可能有极小的速度量化差异；音符位置与休止长度仍保留整数 tick。

`sfx`、`tone`、`beat` 只生成 WAV；不会把音效或鼓点转换成 MIDI。导出 MP3、FLAC 等格式可把生成的 WAV 交给现有音频转换能力。

## 安全输出与 dry-run

生成命令先校验完整输入及参数，在内存中完成合成与编码，最后原子发布。默认不覆盖已有文件；即使传入 `--overwrite`，也拒绝符号链接、目录等非普通目标，且不能覆盖乐谱输入本身或它的硬链接。

输出必须是文件路径，不支持 `-o -` 或把 WAV/MIDI 二进制直接写入 stdout。终端显示的是执行报告；重定向 stdout 得到的是报告文本或 JSON，不是音频。

```sh
# 检查文件乐谱、音色、采样率和输出路径，但不保存音频
kito sound melody --file tune.txt --instrument bell \
  --sample-rate 48000 -o exports/tune.wav --dry-run --json

# 确认计划后运行同一命令，去掉 --dry-run
kito sound melody --file tune.txt --instrument bell \
  --sample-rate 48000 -o exports/tune.wav --json
```

`--dry-run` 仍会读取并解析乐谱，检查资源上限、输出扩展名和已有目标；因此也可能报错。它不合成采样、不创建缺失目录或文件，不代表已验证真实合成后的峰值。普通生成成功时可以创建输出所需的父目录。无效输入或合成失败不会发布部分结果，也不会损坏已有目标。

WAV 声部包含起落包络，混音过高时进行整体峰值衰减，避免通过硬削波截断波形。包络在请求时长内结束，不在输出末尾额外添加尾音；乐谱末尾的休止符则保留。合成音色可能包含谐波，不能把 `chip`、`bell` 等音色当作无谐波的测量信号。

## JSON 与退出状态

`presets --json` 输出预设清单，包含 `partner`、`effects`、`instruments`、`beats`；三个数组中的条目有 `name` 和 `description`，`effects` 还包含默认 `duration_seconds`。生成命令的 `--json` 输出本次执行报告。stdout 始终只有一个 JSON 对象，不混入进度或说明文字。参数、输入或输出失败时返回非零状态，错误写入 stderr，不向 stdout 输出半份 JSON。

生成报告的字段如下，不适用的字段为 `null`：

| 字段 | 含义 |
| --- | --- |
| `schema_version` | 报告结构版本，当前为 `1` |
| `partner` | 伙伴名称，`Echo` |
| `operation` | 操作名称，如 `sfx`、`tone`、`melody`、`beat` |
| `format` | 输出格式，`wav` 或 `midi` |
| `output` | 输出路径 |
| `dry_run` | 是否只检查与规划 |
| `event_count` | 发声音符或声音事件数；和弦内的音分别计数，休止不计数 |
| `sample_rate` | WAV 采样率；MIDI 为 `null` |
| `channels` | WAV 声道数，固定为 `1`；MIDI 为 `null` |
| `frames` | 输出的采样帧数；单声道下等于采样点数 |
| `duration_seconds` | 时长，单位秒；WAV 以帧数为准，MIDI 为按请求 BPM 计算的乐谱时长 |
| `instrument` | `tone` / WAV `melody` 使用的合成音色；不适用时为 `null` |
| `seed` | WAV 噪声种子；MIDI 为 `null` |
| `volume` | WAV 总振幅参数；MIDI 为 `null` |
| `peak` | 渲染结果的峰值振幅，绝对值范围 `0..1` |
| `normalization_gain` | 为控制混音峰值采用的整体增益；`1` 表示无需额外衰减，小于 `1` 表示衰减 |
| `parameters` | 当前命令实际生效的场景、音高或节奏参数，结构见下表 |

`parameters` 按命令提供以下字段：

| 命令 | 字段 |
| --- | --- |
| `sfx` | `preset`、`pitch`（半音偏移）、`requested_duration_seconds` |
| `tone` | `pitch`（输入音高文本）、`frequency_hz`、`requested_duration_seconds` |
| `melody` | `bpm`、`repeat`、`total_ticks`（含休止与重复）、`ticks_per_beat`（固定 `480`） |
| `beat` | `preset`、`bpm`、`bars`、`kick`、`snare`、`hat`（自定义替换后实际采用的模式） |

以下命令生成一秒静音，便于理解帧数和峰值：

```sh
kito sound tone A4 --duration 1 --volume 0 -o silence.wav --json
```

```json
{
  "schema_version": 1,
  "partner": "Echo",
  "operation": "tone",
  "format": "wav",
  "output": "silence.wav",
  "dry_run": false,
  "duration_seconds": 1.0,
  "event_count": 1,
  "sample_rate": 44100,
  "channels": 1,
  "frames": 44100,
  "instrument": "sine",
  "seed": 0,
  "volume": 0.0,
  "peak": 0.0,
  "normalization_gain": 1.0,
  "parameters": {
    "pitch": "A4",
    "frequency_hz": 440.0,
    "requested_duration_seconds": 1.0
  }
}
```

读取 JSON 时应按字段取值，不依赖对象字段顺序或默认文本报告的排版。dry-run 的 `peak` 与 `normalization_gain` 为 `null`；MIDI 的 `sample_rate`、`channels`、`frames`、`instrument`、`seed`、`volume`、`peak` 与 `normalization_gain` 为 `null`。dry-run 和 MIDI 没有真实 WAV 采样数据，不能把它们的报告解释为已经测得音频峰值。

成功退出状态为 `0`，运行时输入/合成/保存错误为 `1`，命令行解析错误为 `2`。脚本应先检查退出状态，再使用输出文件或报告。

## 脚本教程

### 批量生成一组应用提示音

macOS/Linux 的 sh、bash 或 zsh：

```sh
set -e
mkdir -p app-sounds
for preset in notify success error; do
  kito sound sfx "$preset" --volume 0.5 -o "app-sounds/$preset.wav" --json
done
```

每次执行发布一个文件，不提供跨多个命令的事务。如果第二个文件已存在，脚本停止，之前成功生成的文件会保留；需要重新整组生成时请明确处理已有文件。

PowerShell：

```powershell
New-Item -ItemType Directory -Force app-sounds | Out-Null
foreach ($preset in 'notify', 'success', 'error') {
    kito sound sfx $preset --volume 0.5 -o "app-sounds/$preset.wav" --json
    if ($LASTEXITCODE -ne 0) { throw "生成 $preset 失败" }
}
```

### 同一乐谱导出试听版和编曲版

macOS/Linux：

```sh
set -e
printf '%s\n' 'C4 E4 G4:2 | F4 A4 C5:2 | G4:2 R:2' > theme.txt
kito sound melody --file theme.txt --bpm 108 --instrument soft -o theme.wav
kito sound melody --file theme.txt --bpm 108 -o theme.mid
```

两份文件包含相同的音符时间安排；WAV 保存 Kito 的合成声音，MIDI 交给后续音源解释。保留 `theme.txt` 便于版本管理和再次修改。

### 给同一噪声音效生成可追溯变体

macOS/Linux：

```sh
set -e
for seed in 1 2 3; do
  kito sound sfx whoosh --seed "$seed" -o "whoosh-$seed.wav" --json > "whoosh-$seed.json"
done
```

记录命令参数、种子和 Kito 版本，可在同一环境中重新生成；种子不会替你挑选“最好听”的结果，生成后仍应试听。

## 常见问题与错误

| 现象 | 处理方式 |
| --- | --- |
| 输出已存在 | 换一个 `-o` 路径，或确认后加 `--overwrite` |
| 乐谱中的 `C4:1/2` 无法解析 | 用十进制拍数 `C4:0.5` |
| 乐谱中的 `C4,E4,G4` 无法解析 | 顺序音符用空白：`C4 E4 G4`；和弦用加号：`C4+E4+G4` |
| `C4:0.333` 被拒绝 | 拍数须对应整 tick；例如改用精确可表示的 `0.5`、`0.25` 等，而非期待自动修正 |
| `C4+E4+G4` 变成了多个 shell 参数 | 用单引号或双引号包住整段乐谱 |
| 第一枚音符看起来正确但报错 | 检查 UTF-8 BOM 或复制时带入的隐藏字符；文件须为有效 UTF-8 |
| beat 模式长度错误 | 每条模式必须 16 格，不能包含空白；关闭轨道使用 16 个 `-` |
| 时长超限 | 减少 `--repeat`、`--bars` 或拍数，或适当提高 BPM；总长最多 120 秒 |
| 累计渲染帧数超限 | 缩短长和弦、减少同时发声的音，或降低采样率；所有声部合计最多 64,000,000 帧 |
| 乐谱输入过大 | 将输入缩小到 64 KiB 以内；用 `--repeat` 表达整段重复，同时注意音符数和时长限制 |
| 频率超出采样率范围 | 降低音高，或在允许的范围内提高 `--sample-rate` |
| 导出 MIDI 时 `--instrument bell` 报错 | MIDI 不保存该音色，删除所有 WAV 专用选项，导入 DAW 后再选音源 |
| 想直接听却只看到报告 | 命令生成文件，不自动播放；用播放器打开输出 WAV |
| 输出为 WAV，但感觉音量变小 | 和弦或鼓组叠加可能触发峰值衰减，可查看 `normalization_gain`；`--volume` 不是响度标准化 |

## 设计参考

声音伙伴借鉴以下项目的表达方式，合成与文件输出由 Kito 的 Rust 实现完成：

- [Tone.js 官方说明](https://tonejs.github.io/)：使用音名、节拍、乐器与包络表达声音。Kito 借鉴“写出音符就能开始”的方式，提供有限、可验证的离线乐谱语法。
- [jsfxr 官方源码与 API](https://github.com/chr15m/jsfxr)：通过 coin、laser、jump 等场景预设快速生成游戏音效。Kito 用用途名称作为音效入口，让用户无需先学习大量合成器参数。
- [Bfxr 官方说明](https://www.bfxr.net/v1/)：预设、随机化和声音微调适合短音效创作。Kito 以明确参数与 seed 保留可复现的操作方式，不提供完整 Bfxr 参数兼容。
- [Sonic Pi 官方合成器教程](https://sonic-pi.net/tutorial-02.html)：以音名、顺序、和弦、休止和少量音色选项逐步学习。Kito 采用类似的教学顺序，但不执行 Ruby、JavaScript 或其他乐谱脚本。

Tone.js 面向浏览器 Web Audio，围绕 `AudioContext`、交互播放与时间调度组织 API。当前 Kito 的工作是把短片段离线生成文件，直接使用 Rust 可以减少额外运行环境并便于验证时长、采样和输出安全，因此没有将 Tone.js 嵌入 CLI。这是当前实现的工程取舍，不表示 Tone.js 只能用于在线场景，也不声称与其合成结果或 API 兼容。
