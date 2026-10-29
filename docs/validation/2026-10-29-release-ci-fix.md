# Release CI：FFmpeg 素材与 Node.js 24 修复

日期：2026-10-29。未执行 git commit、push、打 tag 或远端发布。

## 失败原因

`regression_audio_before_video_is_trimmed_to_the_video_origin` 在生成测试素材后，检查视频起点为 `0.4 ± 0.02` 秒时失败，实际为 `0`；此时尚未调用 Kito 的压缩操作。

素材用 `-itsoffset 0.4` 延迟视频，却没有指定输出帧同步方式。FFmpeg 6.1 的 MP4 默认选择 CFR，补齐前 12 帧，视频变成从 0 开始、共 36 帧、长 1.2 秒。FFmpeg 8.1 的默认行为则保留原来的 24 帧、0.4 秒起点和 0.8 秒视频时长。

官方源码中，6.1 的 MP4 muxer 未声明 `AVFMT_VARIABLE_FPS`，8.1 已声明；自动模式据此选择不同策略。参考 [FFmpeg 6.1 MP4 muxer](https://github.com/FFmpeg/FFmpeg/blob/n6.1/libavformat/movenc.c)、[8.1 MP4 muxer](https://github.com/FFmpeg/FFmpeg/blob/n8.1/libavformat/movenc.c) 与 [6.1 同步策略选择](https://github.com/FFmpeg/FFmpeg/blob/n6.1/fftools/ffmpeg_mux_init.c)。

## 修复

- 仅对该测试的素材生成增加 `-fps_mode passthrough`，保留原始时间戳。
- 保持视频起点 `0.4 ± 0.02`、输出时长 `0.8 ± 0.12` 和前段音频 RMS `> 0.03` 的要求；增加音轨起点 `0 ± 0.02` 的素材断言，并在失败时打印探测数据。
- Kito 生产源码和音视频对齐算法未改动。
- 官方 Actions 更新为 Node.js 24 版本：`checkout@v7`、`setup-python@v7`、`upload-artifact@v7`、`download-artifact@v8`。官方 `action.yml` 已核对，上传/下载版本按官方示例配对；tag 触发、平台矩阵和发布依赖保持不变。

## 验证

本机为 macOS arm64，安装的 FFmpeg 为 8.1：

```sh
KITO_REQUIRE_BACKEND=1 scripts/cargo-local.sh test --locked --test video_cli regression_audio_before_video_is_trimmed_to_the_video_origin -- --exact --nocapture
scripts/cargo-local.sh fmt --check
scripts/cargo-local.sh clippy --locked --all-targets -- -D warnings
KITO_OPENSSL=/opt/homebrew/opt/openssl@3/bin/openssl KITO_REQUIRE_BACKEND=1 scripts/cargo-local.sh test --locked
```

以上全部通过。完整 Rust 测试为 278 项通过，0 失败、0 ignored。

另创建 Ubuntu 24.04 ARM64 临时容器，安装发行版 FFmpeg / ffprobe `6.1.1-3ubuntu5`。主机 Rust 测试通过 wrapper 调用容器中的真实后端，仅共享此次创建的临时目录：

1. 执行修改前保留的测试可执行文件，准确复现原错误：`actual=0, expected=0.4 ± 0.02`，退出码 101。
2. 执行修改后的 `video_cli` 中全部 `regression_` 测试，5 项全部通过，覆盖早到/延迟音轨、非零视频起点、Matroska 偏移及旋转元数据。

工作流通过 actionlint 静态检查；使用本机 1.7.7 时，仅忽略其尚未收录的 `macos-15-intel` runner 标签提示。`git diff --check` 通过。

本次验证说明真实 FFmpeg 6.1.1 与 8.1 的相关行为均通过；主机调用容器后端不等同于完整 Linux 原生构建，也不代表 GitHub 五平台工作流已重新运行。远端仍需对包含修复的提交推送匹配版本的 tag；重跑原有 tag 对应的旧提交不会包含这些修改。
