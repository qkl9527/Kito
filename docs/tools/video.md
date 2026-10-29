# 视频工具：Vido 维多

**Vido 维多 · 视频伙伴（剪影师）**，命令入口为 `kito video`。它通过 FFmpeg 与 ffprobe 处理本地视频，也能整理视频音轨和独立音乐文件：转换格式、裁剪片段、拼接、混音、调整速度、淡入淡出与响度。

本文先给出常用命令，再说明每项操作、文件安全、质量边界和报告。所有时间与尺寸以实际探测结果为准；执行 `kito video <命令> --help` 或 `kito video audio <命令> --help` 可查看当前二进制的参数。

## 后端与依赖

需要同时安装 `ffmpeg` 和 `ffprobe`，建议使用 FFmpeg 6 或更新版本的完整构建。Kito 不下载、不捆绑后端；输入支持哪些容器与编码，由本机 FFmpeg 构建的解复用器和解码器决定。安装后先检查：

常见的包管理器安装方式如下，选择自己系统的一条；没有这些包管理器时，可从 [FFmpeg 官方下载入口](https://ffmpeg.org/download.html) 选择对应平台的构建，并把包含两个程序的目录加入 `PATH`。

```sh
# macOS（Homebrew）
brew install ffmpeg

# Ubuntu / Debian
sudo apt update
sudo apt install ffmpeg

# Windows（已安装 Chocolatey，在管理员终端执行）
choco install ffmpeg -y
```

确认两个程序均可运行：

```sh
ffmpeg -version
ffprobe -version
kito video doctor
kito video doctor --json
```

后端默认从 `PATH` 查找，也可单独指定程序路径。环境变量值是可执行文件路径，不是带参数的 shell 命令：

```sh
# macOS / Linux；替换成自己的实际安装路径
export KITO_FFMPEG=/path/to/ffmpeg
export KITO_FFPROBE=/path/to/ffprobe
kito video doctor
```

```powershell
# Windows PowerShell
$env:KITO_FFMPEG = 'C:\Tools\ffmpeg\bin\ffmpeg.exe'
$env:KITO_FFPROBE = 'C:\Tools\ffmpeg\bin\ffprobe.exe'
kito video doctor
```

若一个程序可用、另一个不可用，仍需要补齐后端。指定的路径无效时应修正变量；不要把重编码失败误认为输入本身损坏。具体操作需要相应编码器，例如 H.264 的 `libx264`、VP9 的 `libvpx-vp9`、Opus 的 `libopus`、MP3 的 `libmp3lame`、AAC 的 `aac`、截图的 `png` 和音频的 `flac` / `pcm_s16le`。

`doctor` 检查路径与版本可用性，`healthy: true` 不保证全部编码器、过滤器和容器都可用；缺少某项能力时，实际处理会报错，候选不会发布为目标文件。

FFmpeg 后端为可选依赖：安装 Kito 本身不会要求所有伙伴的外部程序齐备。视频相关的真实后端测试使用临时生成的素材；开发中设置 `KITO_REQUIRE_BACKEND=1` 会要求后端存在。本机通过只说明本机环境的结果，不能据此宣称 macOS、Linux、Windows 均已验证。

## 先试这几条

```sh
# 查看信息
kito video info clip.mov

# 转为 MP4，默认生成 clip.convert.mp4
kito video convert clip.mov --to mp4

# 以体积优先的质量压缩；并不保证结果更小
kito video compress clip.mp4

# 从 10 秒开始截取 5 秒
kito video trim clip.mp4 --start 10 --duration 5 -o highlight.mp4

# 等比缩放至 1280×720 范围内
kito video resize clip.mp4 --size 1280x720

# 顺序拼接，统一尺寸、帧率和音频规格
kito video concat opening.mp4 main.mp4 ending.mp4 -o joined.mp4

# 截图和 GIF
kito video snapshot clip.mp4 --at 00:00:03.5 -o cover.png
kito video gif clip.mp4 --start 2 --duration 3 --width 480 --fps 12

# 提取音轨与转换音乐文件
kito video audio extract clip.mp4 --to mp3
kito video audio convert song.wav --to flac
```

## 能力索引

| 命令 | 用途 | 必须指定的主要参数 |
| --- | --- | --- |
| `doctor` | 检查后端路径与版本 | 无 |
| `info INPUT` | 查看容器、轨道和时长 | 本地文件 |
| `convert INPUT` | 转换视频容器并重编码 | `--to mp4\|mkv\|webm\|mov` |
| `compress INPUT` | 以体积优先档位重编码 | 本地视频 |
| `trim INPUT` | 截取时间片段 | `--end` 或 `--duration` |
| `resize INPUT` | 等比缩放画面 | `--size` |
| `crop INPUT` | 裁切画面矩形 | `--width`、`--height` |
| `rotate INPUT` | 顺时针旋转画面 | `--angle 90\|180\|270` |
| `speed INPUT` | 音画同步变速 | `--factor` |
| `concat INPUTS...` | 顺序拼接视频 | 至少两个输入 |
| `mute INPUT` | 去除音轨 | 本地视频 |
| `audio extract INPUT` | 提取第一条音轨 | 包含音轨的文件 |
| `audio replace INPUT` | 替换视频配乐 | `--audio PATH` |
| `audio volume INPUT` | 调整线性音量 | `--factor` |
| `audio convert INPUT` | 转换音频格式 | 包含音轨的文件 |
| `audio trim INPUT` | 截取音频片段 | `--end` 或 `--duration` |
| `audio concat INPUTS...` | 顺序拼接音频 | 至少两个输入 |
| `audio mix INPUTS...` | 多段音频同时混合 | 至少两个输入 |
| `audio speed INPUT` | 保持音调地改变音乐速度 | `--factor` |
| `audio fade INPUT` | 添加淡入和淡出 | `--fade-in`、`--fade-out` |
| `audio normalize INPUT` | 按目标 LUFS 处理响度 | 可选 `--loudness`，默认 `-16` |
| `snapshot INPUT` | 截取一帧 PNG | 可选 `--at`，默认 `0` |
| `gif INPUT` | 制作 GIF 动图 | 可选 `--start`、`--duration`、`--width`、`--fps` |

`video` 是功能入口，`Vido` 只用于角色介绍，没有 `vido` 命令别名。

## 共用参数与文件规则

除 `info`、`doctor` 外，所有输出操作都支持以下参数：

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `-o, --output PATH` | 输入旁的新文件 | 指定目标路径；视频一般默认 MP4，音乐一般默认 MP3 |
| `--overwrite` | 关闭 | 允许替换已有目标；仍禁止覆盖任何输入文件 |
| `--json` | 关闭 | 以 JSON 输出报告，便于脚本读取 |
| `--time-limit SECONDS` | `3600` | 每次 FFmpeg / ffprobe 子进程的最大运行秒数，必须大于 `0` |
| `--dry-run` | 关闭 | 探测输入并展示结构化 FFmpeg 参数，不编码、不创建目标 |

视频需要随机访问素材，只接收本地普通文件，不支持 `-` 代表的 stdin、URL、远程媒体或播放列表协议。多输入命令按参数的书写顺序处理，带空格的路径应加引号。媒体内容不会作为 shell 命令执行；后端通过独立参数数组启动。

探测、处理与输出验证均限制为 `file` 协议及直接媒体容器白名单。HLS、DASH、concat 播放列表和 image2 文件序列不能间接读取其他文件，伪装成 `.mp4` 也会拒绝。具体输入仍须满足后端解码能力。

默认文件名使用 `<原文件名去扩展名>.<操作>.<扩展名>`，如 `clip.compress.mp4`、`clip.trim.mp4`、`clip.snapshot.png` 和 `clip.gif.gif`。多输入操作使用第一份输入的文件名。提取音轨的后缀是 `audio`，替换音轨是 `replace-audio`。指定 `-o` 后使用该路径。

音乐转换、裁剪、拼接、混音、变速、淡化和响度处理的操作后缀分别为 `audio-convert`、`audio-trim`、`audio-concat`、`audio-mix`、`audio-speed`、`audio-fade`、`audio-normalize`；音量调整统一使用 `volume`。例如 `song.wav` 的 FLAC 转换默认输出 `song.audio-convert.flac`。

`convert --to` 明确指定视频格式；其他常规视频操作可通过输出扩展名选择 `mp4`、`mkv`、`webm` 或 `mov`，省略输出路径时默认 MP4。音频输出使用 `--to` 指定，默认 MP3。输出扩展名应与选择的格式保持一致，不支持的格式或冲突应先改正。

写入流程先保护输入和检查目标，再将编码结果保存到目标目录内的临时候选。候选通过探测与解码验证后才原子发布；错误或超时不能损坏原输入或已存在的输出。`--overwrite` 表示允许替换目标，不表示允许就地改写输入。符号链接或指向同一文件的路径不能用来绕开保护。

`--time-limit` 是每次子进程的独立上限，不是整个命令的总耗时上限：探测、编码和验证各自可能启动子进程。处理长视频时可适当调大；磁盘空间也需要容纳完整临时候选。

需要计算时长的操作优先使用所选轨道的时长。MKV 等容器可能只有容器级时长，而不同音视频轨的长度和起点并不相同；这时 Kito 会额外解码所选轨道以测量时长，再进行裁剪、淡出或配乐等操作。该测量也遵守 `--time-limit`，因此这些文件的 `--dry-run` 也可能需要较长时间。`info` 仍是只读元数据探测，缺失字段保留为空。

## 质量、格式与保留范围

### 视频格式

| 目标容器 | 视频编码 | 音频编码 | 常见用途 |
| --- | --- | --- | --- |
| MP4 | H.264 | AAC | 常规视频分享与播放 |
| MKV | H.264 | AAC | Matroska 容器输出 |
| MOV | H.264 | AAC | QuickTime 容器输出 |
| WebM | VP9 | Opus | WebM 输出 |

输入容器与输出编码是两件事：将 MOV 转成 MP4 不代表仅更换容器。这里的操作都会重新编码，不提供 stream copy，也不提供无损视频剪辑模式。

常规视频操作支持 `--quality high|balanced|small` 与 `--preset fast|medium|slow`。默认 `balanced` 与 `medium`，只有 `compress` 默认使用 `small`：

| 质量档位 | H.264 CRF | VP9 CRF | 取舍 |
| --- | --- | --- | --- |
| `high` | 18 | 28 | 更注重画面质量，通常占用更多空间 |
| `balanced` | 23 | 33 | 质量与体积折中 |
| `small` | 28 | 38 | 更注重体积，画面损失可能更明显 |

不同编码器的 CRF 数字不能直接横向比较。`preset` 控制编码速度与压缩效率，不是画面分辨率；较慢档位通常花费更多 CPU 时间。没有固定目标码率或指定最终文件大小的参数。

重复有损编码会累积损失；即使选择 `high`，也不会恢复素材原先丢掉的细节。`compress` 只是采用更偏向体积的编码档位，已经充分压缩、很短或很简单的视频仍可能变大。应结合输出报告和肉眼观看判断结果。

### 轨道与元数据

普通视频操作取第一条真实视频轨和第一条音轨；封面图片不当作真实视频轨。音频命令取第一条音轨，其他音轨不混入，除非显式使用 `audio mix` 并把每个素材列为输入。没有音轨的素材可以执行不需要音频的操作；提取或处理音频时则需要实际存在音轨。

输出不保留原有字幕、附件、章节、封面和自定义元数据，不提供语言轨道选择、字幕烧录或原始时间码保留。编码器和容器可能生成自身的描述字段，不能把它理解为完全没有任何元数据。

视频会规范化显示方向与像素宽高比，尺寸处理针对显示画面。常规视频输出使用 **8 bit `yuv420p`**，不保留 HDR、原始高位深或透明通道，也不承诺保留广色域。当前没有自动色调映射，HDR 素材不能理解为原样转换到 SDR；这类素材应先确认转换后的观感与用途。

视频处理以所选画面的起点作为时间零点，保留音轨相对画面的延迟：比画面更早开始的音频部分裁除，晚于画面开始的音频前补静音。纯音频处理则以所选音轨自身的起点归零，不把原容器的空白前导时间带入音乐文件。

## 信息与后端检查

```sh
kito video doctor --json
kito video info clip.mp4 --json
kito video info soundtrack.flac --json
```

`info` 使用 ffprobe 查看实际内容，而不是单靠文件扩展名猜测。报告包括容器名称、字节数、时长和轨道列表，以及存在时的视频编码、宽高、帧率、显示旋转、像素宽高比或音频采样率和声道数。缺少的元数据不会被伪造为 `0`；需要可靠时长的剪辑命令会拒绝无法确定时长的输入。

`info --json` 的顶层字段为 `path`、`duration`、`size_bytes`、`format`、`streams`。每条轨道含 `index`、`codec_type`、`codec`，以及适用时的 `width`、`height`、`duration`、`start_time`、`fps`、`sample_rate`、`channels`、`attached_pic`、`rotation`、`sample_aspect_ratio`。`start_time` 记录原轨道时间起点，用于理解音视频偏移；它不是处理后片段中的裁剪起点。未知可选值以 `null` 表示。

`doctor --json` 包括 `kito` 版本、`healthy` 和两个后端对象 `ffmpeg` / `ffprobe`；每个后端对象提供 `path`、`version`、`error`。任一后端检查失败时 `healthy` 为 `false`，命令返回失败退出码，JSON 保留相应诊断。

## 视频操作详解

### 转换与压缩

```sh
kito video convert clip.mov --to mp4 --quality high -o share.mp4
kito video convert clip.mp4 --to webm --preset slow -o web.webm
kito video compress clip.mp4 --quality small --preset slow -o smaller.mp4
```

`convert` 的 `--to` 必填。`compress` 可以覆盖默认质量，例如选择 `balanced`。MP4、MKV 和 MOV 均采用 H.264/AAC；容器名本身不会让它自动使用 ProRes、HEVC 或其他编码器。

### 时间片段裁剪

```sh
kito video trim clip.mp4 --start 12.5 --duration 7.5
kito video trim clip.mp4 --start 00:01:10 --end 00:01:20 -o ten-seconds.mp4
```

`--start` 默认 `0`。`--end` 和 `--duration` 必须且只能选一个；前者是原素材中的绝对终点，后者是从起点计算的片段长度。起点、终点与时长必须落在素材范围内，终点必须晚于起点，时长必须大于 `0`。

所有时间位置和时长统一支持：非负秒数（`12`、`12.5`）、`MM:SS`（`01:30.25`）或 `HH:MM:SS`（`00:01:30.25`）。小数最多六位，冒号后的分、秒必须小于 `60`，不支持负数、科学计数法或 FFmpeg 表达式。时间参数以秒为基准，最终切点受音视频帧与编码采样边界影响，不承诺任意微秒都能成为独立视频帧。

### 缩放与裁切画面

```sh
kito video resize clip.mp4 --size 1280x720
kito video resize clip.mp4 --size 1280x
kito video resize clip.mp4 --size x720
kito video resize clip.mp4 --size 50%
kito video crop clip.mp4 --width 640 --height 360 --x 100 --y 50
```

`resize` 沿用图片工具的等比尺寸语法：`1280x720` 表示放进这个范围，不强行拉伸为该比例；`1280x` 固定宽度、`x720` 固定高度、`50%` 按比例缩放。允许放大，放大不会创造原素材没有的细节。

视频宽高需在 `2` 到 `16384` 像素之间，并向下对齐到偶数以满足编码要求。例如 `160×90` 缩放到 `80x60` 的范围内，输出为 `80×44`。这属于重采样，不是原像素无损操作。

`crop` 在显示方向规范化后的画面中取矩形，`--x`、`--y` 从左上角起算且默认 `0`。宽高和坐标都必须为偶数，宽高至少为 `2`，区域不得超出画面；奇数值直接报错，以免 YUV420 色度网格悄悄改变用户要求的区域。裁切与缩放作用不同：裁切丢掉边缘内容，缩放改变整幅画面的尺寸。

### 旋转与变速

```sh
kito video rotate clip.mp4 --angle 90
kito video rotate clip.mp4 --angle 180
kito video speed clip.mp4 --factor 2
kito video speed clip.mp4 --factor 0.5
```

旋转只接受顺时针 `90`、`180`、`270` 度，90/270 度会交换宽高。速度倍数支持 `0.125` 到 `8`，必须为有限数值；`2` 表示两倍速，时长约为原来的二分之一，`0.5` 表示半速，时长约为原来的两倍。存在音轨时同步变速并保持音调；极端倍数仍可能改变听感。

### 拼接

```sh
kito video concat part1.mp4 part2.mov part3.webm -o joined.mp4
kito video concat portrait.mp4 landscape.mp4 --size 1280x720 --fps 30 -o joined.mp4
```

提供 `2` 到 `64` 段，按参数顺序播放。画布默认采用第一段的显示尺寸，也可用 `--size WIDTHxHEIGHT` 指定两个偶数宽高；这里的画布尺寸不接受单边或百分比写法。每段等比放入画布，多余位置以黑边补齐；默认统一到 `30` fps，可通过 `--fps` 在 `1` 到 `120` 之间调整。不同原始帧率会进行帧率转换，不保留可变帧率时间布局。

拼接音频统一为 48 kHz 立体声。没有音轨的片段补静音，避免音画错位；不提供转场、片段重叠或交叉淡化。拼接操作会重新编码所有片段。

### 去音轨与替换配乐

```sh
kito video mute clip.mp4
kito video audio replace clip.mp4 --audio music.mp3 -o scored.mp4
```

`mute` 删除音轨；`audio volume --factor 0` 则保留一条静音轨，两者输出结构不同。替换配乐使用指定文件的第一条音轨，原视频音轨被移除。新配乐较短时尾部补静音，较长时截到视频结束，视频时长保持原素材长度。

## 音乐与音频处理

音频命令可读取纯音乐文件，也可读取视频中的第一条音轨。除 `audio replace` 和默认保留视频画面的 `audio volume` 外，音乐命令仅输出音频，不保留画面、封面或原有标签。

`--to` 支持 `mp3`、`m4a`、`wav`、`flac`，默认 `mp3`。所有音乐输出统一为 48 kHz 立体声，按下列参数编码：

| 格式 | 编码器与参数 | 说明 |
| --- | --- | --- |
| MP3 | `libmp3lame`，192 kbit/s | 有损编码 |
| M4A | `aac`，192 kbit/s | 有损编码 |
| WAV | `pcm_s16le`，16 bit PCM | 未压缩音频，体积通常较大 |
| FLAC | `flac`，16 bit | 对送入编码器的 PCM 无损压缩 |

WAV/PCM 与 FLAC 可避免目标音频编码本身再使用有损压缩，但采样率、声道和 16 bit 规格转换可能引入重采样、混合或量化；它们不承诺与源文件逐采样一致。把 MP3 转为 FLAC 只会把已经解码的声音保存下来，不会恢复原先损失的频率或细节。报告中的 `lossy: true` 表示没有源音频逐样本无损承诺，即使目标为 WAV 或 FLAC 也是如此。

音频当前使用预设的编码参数，不提供音频 `--quality`、`--bitrate` 或采样格式选择。视频的 `--quality` / `--preset` 只控制视频编码；在音频输出模式中不能用它们推导音频质量。

### 提取与转换

```sh
kito video audio extract clip.mp4 --to mp3 -o soundtrack.mp3
kito video audio extract clip.mp4 --to wav -o soundtrack.wav
kito video audio convert recording.wav --to flac -o archive.flac
kito video audio convert song.flac --to m4a -o portable.m4a
```

`extract` 用来表达“从素材中取出音轨”，`convert` 用来表达“把音频转换成目标格式”；二者都重新编码第一条音轨，不执行 stream copy。文件没有音轨时会报错，不生成空音频。

### 音频裁剪与拼接

```sh
kito video audio trim recording.wav --start 00:01:00 --duration 30 --to wav
kito video audio trim song.flac --start 5 --end 25 --to flac
kito video audio concat intro.wav speech.wav outro.wav --to mp3 -o program.mp3
```

音频裁剪与视频使用相同的时间语法和范围检查，`--end` / `--duration` 二选一。音频拼接接受 `2` 到 `64` 个输入，按顺序连接并统一为 48 kHz 立体声；不会自动消除两段边界处的停顿，也没有交叉淡化参数。

### 混音

```sh
kito video audio mix voice.wav music.wav --to wav -o mixed.wav
```

`mix` 接受 `2` 到 `64` 个输入，从同一时刻开始播放，以相同权重混合，以最长一条音轨结束时间为准。它使用 `amix normalize=1` 的混音归一化减少直接相加造成的峰值溢出；等权不代表每段在人耳听来一样响。当前不提供每轨权重、延迟、声像或按说话声自动降低背景音乐的参数。

需要控制背景音乐音量时，先生成调低音量的音轨，再将它与人声混合：

```sh
kito video audio volume music.wav --factor 0.25 --to wav -o quiet-music.wav
kito video audio mix voice.wav quiet-music.wav --to wav -o mixed.wav
```

### 音量与速度

```sh
# 调整纯音频音量，指定 WAV 避免额外有损编码
kito video audio volume song.wav --factor 0.5 --to wav

# 保留视频画面并调低其音轨
kito video audio volume clip.mp4 --factor 0.5 -o quieter.mp4

# 显式 --to 只输出音频
kito video audio volume clip.mp4 --factor 0.8 --to mp3 -o quieter.mp3

# 变速并保持音调
kito video audio speed speech.wav --factor 1.25 --to wav
```

音量倍数是线性振幅，不是 dB，支持有限值 `0` 到 `16`：`1` 保持原值，`0.5` 减半，`2` 加倍，`0` 静音。提高音量可能削波失真，不能等同于响度规范化。纯音频输入默认输出 MP3；视频输入省略 `--to` 时输出含视频的结果，指定 `--to` 则仅输出音频。

音频变速支持 `0.125` 到 `8` 倍，保持音调但改变时长，不提供单独的移调功能。处理后仍应试听人声、打击乐和极端变速处的效果。

### 淡入淡出

```sh
kito video audio fade song.wav --fade-in 2 --fade-out 3 --to wav -o faded.wav
```

`--fade-in`、`--fade-out` 默认 `0`，至少一个必须大于 `0`，接受统一时间语法。淡入从开头持续指定时长，淡出在结束前相同时长处开始。两个值分别不得超过音轨时长，但允许淡入与淡出区间交叠。淡化不延长原音频，也不是两段音乐之间的交叉淡化。

### 响度处理

```sh
kito video audio normalize podcast.wav --loudness -16 --to wav -o normalized.wav
```

`--loudness` 的单位为 LUFS，默认 `-16`，允许 `-70` 到 `-5` 的有限值。它衡量整体主观响度，不是线性音量倍数。当前使用 FFmpeg `loudnorm` 单遍动态处理，固定真峰值参数 `TP=-1.5` dBTP、响度范围参数 `LRA=11` LU；这些是处理参数，不是输出测量值。

单遍处理不能承诺输出精确达到目标 LUFS，也不提供双遍测量报告或广播交付认证。静音、很短的片段和动态变化大的音频尤其需要试听，并在有严格交付规范时另行测量输出。

## 截图与 GIF

```sh
kito video snapshot clip.mp4 --at 3.5 -o cover.png
kito video gif clip.mp4 --start 00:00:02 --duration 3 --width 480 --fps 12 -o preview.gif
```

`snapshot` 默认截取 `0` 秒处的一帧，固定输出 PNG。截图的颜色来自视频解码结果，不代表保留视频原始压缩编码或原图像素。

`gif` 默认起点 `0`、时长 `5` 秒、宽度 `480`、帧率 `12`。片段最多 `60` 秒，宽度范围 `2` 到 `4096`，帧率范围 `1` 到 `50`；起点加时长不能超出素材。高度按画面比例计算，使用调色板生成和映射改善 GIF 颜色表现；GIF 的颜色数仍有限，不等于源视频无损。GIF 不含音频并无限循环，长片段、高分辨率或高帧率可能生成很大文件。

## 预览参数与 JSON 报告

```sh
kito video convert clip.mov --to mp4 --dry-run --json
kito video audio trim song.wav --start 10 --duration 5 --to flac --dry-run --json
kito video trim clip.mp4 --start 10 --duration 5 --json
```

`--dry-run` 会读取输入、调用 ffprobe 并执行参数及路径校验，因此仍需要合法媒体文件与可用的 ffprobe；需要测量缺失的轨道时长时也会调用 FFmpeg 解码。它展示将采用的操作和 FFmpeg 参数数组，不启动编码，也不创建目标。报告中的参数字符串仅用于检查；真实执行把每个值作为独立 argv 传递。不要把展示值直接用空格拼接后交给 shell 执行，否则带空格或 shell 特殊字符的路径可能改变含义。

正常执行的 JSON 报告包含以下字段：

| 字段 | 含义 |
| --- | --- |
| `operation` | 处理操作名 |
| `inputs` / `output` | 输入列表与最终目标路径 |
| `input_bytes` / `output_bytes` | 输入总字节数与输出字节数 |
| `output_info` | 发布结果的容器、轨道、尺寸、时长等信息 |
| `lossy` | 是否不承诺保留源信息；音乐重采样与量化也包含在内 |
| `verified_decodable` | 输出已通过完整音视频解码检查，且每条预期输出轨实际包含帧或采样 |
| `notes` | 当前操作的质量、保留范围或规格说明 |

`--dry-run` 报告使用 `dry_run: true`，并提供 `program`、`args`、`operation`、`output`、`lossy` 和 `notes`；此时尚未生成结果，不会给出输出验证成功的声明。

输入输出字节数适合观察体积变化，不能作为画面或音频质量的证明。可自行用 `(input_bytes - output_bytes) / input_bytes` 计算比例；输出变大时差值为负，不把“压缩”名字当作大小承诺。

`--json` 用于脚本解析结构化 stdout；失败时返回非零退出码并在 stderr 说明原因，不应把失败当作成功结果继续处理。无论是否选择 JSON，发布目标都遵守同一保护规则。

## 常见问题与边界

- **找不到后端：** 检查 `PATH` 或两个 `KITO_*` 路径变量，运行 `video doctor`。一个可执行文件可启动不代表其构建包含所需编码器。
- **输出已经存在：** 使用新的 `-o` 路径，或确认后加 `--overwrite`；同输入路径仍会拒绝。
- **裁剪范围无效：** 用 `video info` 查看轨道元数据，并检查终点与时长是否混淆。容器时长可能包含其他轨道或非零起点，不能代替每条轨的持续时间；轨道时长缺失时会解码测量，仍无法确定时长或没有实际帧/采样则报错。
- **画面少一行或一列：** 常规视频输出需要偶数宽高，会向下对齐；以报告中的实际尺寸为准。
- **压缩后更大：** 更改质量或尺寸，比较画面后再决定是否采用结果。Kito 不会为了保证数字更小而悄悄丢弃用户的处理要求。
- **音频转换后没有封面或标签：** 当前不保留原有封面与 metadata，也没有标签编辑功能。
- **格式名相同但无法处理：** 容器和实际编码不同，输入扩展名不构成支持保证。后端报错应结合轨道编码、构建能力与文件完整性判断。
- **需要批量、字幕或时间线编辑：** 当前命令一次产生一个结果，不提供目录批量、字幕处理、镜头转场、多轨时间线、硬件编码或任意 FFmpeg 参数透传。

视频与音乐输出经过重新编码和必要的规格转换。最终是否满足播放平台、音质或交付标准，应根据实际输出检查；这里的候选验证用于发现不可读、缺失轨道或解码失败，不代替主观质量评价。
