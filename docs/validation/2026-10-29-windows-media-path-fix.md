# Windows 媒体候选文件路径校验修复

日期：2026-10-29。未执行 git commit、push、打 tag 或发布。

## 原因与修复

Windows Actions 的 `audio_cli` 中，extract、convert、concat、fade 等操作在输出媒体探测阶段失败。`execute()` 已对候选文件调用 `canonicalize()`，得到 `\\?\C:\...\.kito-video-xxx.wav`；`input_path()` 却把所有以 `\\` 开头的路径都视为不支持的输入，误拒绝本地文件。Rust 的 [canonicalize 文档](https://doc.rust-lang.org/std/fs/fn.canonicalize.html)说明 Windows 会返回这种扩展路径形式。

- Windows 使用 `Path::components()` 区分路径前缀，允许普通盘符路径和绝对 `VerbatimDisk` 路径；保留 `\\?\` 前缀及其长路径语义。
- 继续拒绝 URL、stdin、UNC 网络共享和设备命名空间；在 canonicalize 后再次校验，避免符号链接或相对路径解析到不支持的命名空间。
- 保留普通文件、可读性、媒体探测和完整解码检查，以及验证成功后才原子发布的流程。

## 验证

新增 `video_probe` 回归测试覆盖规范化路径可重复传入、中文与空格文件名、规范化目录仍被拒绝，以及协议、网络共享和设备路径在文件访问前被拒绝。另有 Windows 专属测试覆盖正斜杠、混合分隔符和无效的扩展盘符写法。

本机 macOS arm64、FFmpeg 8.1 执行：

```sh
scripts/cargo-local.sh fmt --check
scripts/cargo-local.sh clippy --locked --all-targets -- -D warnings
KITO_REQUIRE_BACKEND=1 KITO_OPENSSL=/opt/homebrew/opt/openssl@3/bin/openssl scripts/cargo-local.sh test --locked
```

全部通过：280 项测试通过，0 失败、0 ignored。其中音频 CLI 7 项、视频 CLI 17 项、媒体探测 9 项。

本机没有 Windows 执行环境，未执行 Windows 专属测试，也未在本机复现 Windows 修复前后的运行结果。Windows 的实际路径解析与音视频完整流程仍须由包含本次修改的新 tag 在 Actions 上验证；重跑旧 tag 不会包含本次修改。
