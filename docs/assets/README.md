# README 截图来源与复现

截图日期：2026-10-29。版本：Kito 0.1.0。本机环境：macOS arm64。

| 文件 | 内容与来源 |
| --- | --- |
| `cli-demo.png` | 实际执行时区转换、Base64 编码、SHA-256 命令后，将原始输出放入终端样式外框再截图。只添加标题、说明与配色，没有改写结果。 |
| `web-home.png` | `kito web` 编译内嵌的正式首页，已连接本机服务。 |
| `web-image.png` | 上传 `sample-landscape.png`，选择「压缩 PNG → 快速优化」，实际处理完成后的页面。 |
| `web-sound.png` | 使用 `success` 预设合成音效、打开预览后的真实页面。下载的 WAV 已检查：PCM16、单声道、44100 Hz、39690 帧，即 0.90 秒。 |
| `sample-landscape.png` | 从仓库原创素材 [`web/assets/landscape.svg`](../../web/assets/landscape.svg) 在 800×600 视口下渲染并截图得到的 PNG，供网页上传演示；拍摄时的上传文件名为 `landscape.png`。 |
| `cli-transcript.json` | CLI 截图所用命令及未经改写的 stdout / stderr，便于核对。 |

网页截图直接来自生产页面，没有更换状态、数值或报告；截图不包含启动 URL 或会话令牌。`docs/design/web-ui/` 下的设计稿未用于这些功能截图。

## CLI 展示图

在仓库根目录执行：

```sh
scripts/cargo-local.sh build --locked --release
python3 scripts/render-readme-demo.py
```

脚本仅依赖 Python 3 标准库，在独立临时配置目录中执行真实命令，避免读取或修改个人时区设置。任一命令失败会直接中断。可通过 `--binary` 指定其他 Kito 二进制，通过 `--output-dir` 指定展示页输出目录。

生成的 `artifacts/readme-capture/cli-demo.html` 可用浏览器打开，以 1440 像素宽度截取完整页面；旁边的 `cli-transcript.json` 保存本次原始输出。更新截图时，一并更新本目录的输出记录与截图版本说明。

## 网页截图

```sh
target/release/kito web --no-open
```

在浏览器打开终端给出的本机链接。页面载入后会移除地址栏中的令牌，等待「本机已连接」再开始：

1. 首页：在 1440×1100 视口下截图。
2. 图片处理：选择本目录的 `sample-landscape.png`，选择「快速优化」，点击「压缩 PNG」。等待「处理完成」后，在 1440×1570 视口下截图。
3. 声音创作：选择 `success`，时长留空、音高偏移保持 `0`，点击「合成音效」。等待完成，再点击「预览」，在 1440×1330 视口下截图。

示例 PNG 的实际结果是 18210 → 18210 字节：内置候选未变得更小，因此保留原编码，仍通过像素无损验证。这是该文件本次执行的结果，不是压缩率承诺。网页输出中的耗时和临时路径随运行变化，更新时保留当次真实值。

截图只证明上述本机示例已运行，不代表其他平台、外部编解码器或远程 Release 工作流已完成验证。
