# PNG 快照输出的单文件探测修复

日期：2026-10-29。未执行 git commit、push、打 tag 或发布。

## 根因

Windows Actions 的三个视频测试均在 PNG 快照输出探测时失败：路径已通过 Kito 的本地文件校验，但 ffprobe 选择 `image2`，随后被格式白名单拒绝。

FFmpeg 会在读取文件内容前按文件名预探测。PNG 路径中的 `?`、`*`、`{` 或 `%d` / `%03d` 等模式会提高 `image2` 的探测分数。Windows 的 `\\?\C:\...` 扩展路径自带 `?`，因此触发图片序列探测。白名单是在格式选定后检查，不会让自动探测跳过 `image2` 再选择 `png_pipe`。参考官方 [img2dec.c](https://github.com/FFmpeg/FFmpeg/blob/n8.1/libavformat/img2dec.c) 和 [demux.c](https://github.com/FFmpeg/FFmpeg/blob/n7.1/libavformat/demux.c)。

## 修复与回归

- `snapshot` 固定编码 PNG，因此仅对该操作的候选输出指定 `-f png_pipe`；ffprobe 探测与 FFmpeg 完整解码共用此设置，均放在对应 `-i` 之前。
- 保留格式、协议白名单及完整解码后才发布的流程；没有放行 `image2`，也不按用户输入的扩展名强制格式。
- 普通媒体输入、GIF 和 APNG 保持原探测策略，避免把共享 PNG 签名的 APNG 当成静态 PNG。
- 新增端到端回归：输出目录包含 `%03d`，Unix 额外包含 `?`。验证输出格式、解码完成标志、尺寸、逐像素一致性，以及仅发布一个结果文件。

在 macOS arm64、FFmpeg 8.1 上，新回归测试在修改生产代码前真实失败：退出码 101，错误同为 `[image2] Format not on whitelist`；修改后通过。另用独立 FFmpeg / ffprobe 命令确认两处自动探测都受模式字符影响，显式 `png_pipe` 后均成功。

## 完整检查

```sh
scripts/cargo-local.sh fmt --check
scripts/cargo-local.sh clippy --locked --all-targets -- -D warnings
KITO_REQUIRE_BACKEND=1 KITO_OPENSSL=/opt/homebrew/opt/openssl@3/bin/openssl scripts/cargo-local.sh test --locked
```

以上全部通过：281 项测试通过，0 失败、0 ignored，包含原先报错的视频测试、音频、GIF 与播放列表拒绝测试。`git diff --check` 通过。

本次在 macOS 复现并修复了同一个 FFmpeg 格式选择错误，未运行 Windows 原生测试。Windows Actions 仍需使用包含本次修复的 tag 验证；旧 tag 的重跑不会包含此次修改。
