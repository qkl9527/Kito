# 本机网页 · 工具小队

`kito web` 把常用 CLI 能力放进本机网页：选择一位伙伴，填写参数或选择文件，运行后查看结果并下载。网页资源内嵌在 Kito 二进制中，不需要安装 Node.js 或前端依赖。

## 启动与停止

```sh
# 自动分配本机端口，并尝试打开默认浏览器
kito web

# 手动打开终端打印的完整链接
kito web --no-open

# 使用指定端口；端口被占用时会报错
kito web --port 8765 --no-open

# 查看参数和使用案例
kito web --help
```

默认 `--port 0` 由操作系统分配端口；可指定 0–65535。服务仅监听 `127.0.0.1`，不能作为局域网共享服务。启动后终端打印 `http://127.0.0.1:<端口>/#token=<会话令牌>`，请用完整链接进入；每次启动的令牌不同。关闭浏览器不会停止服务，在启动服务的终端按 `Ctrl+C` 结束。

从源码运行可使用 `cargo run -- web`；使用仓库本地 Rust 时运行 `scripts/cargo-local.sh run -- web`。已构建的开发版本可直接执行 `./target/debug/kito web`。修改 `web/` 资源后须重新构建并重启，因为页面随二进制内嵌。

## 使用流程

1. 在首页「工具小队」选择领域和操作。
2. 输入文本或使用文件选择器上传文件，再设置参数。需要外部后端的功能先查看页面的能力提示。
3. 运行后等待任务完成；结果区提供命令输出、报告或下载按钮。二进制结果通过下载保存，避免把任意字节当作 UTF-8 文本。
4. 把需要的结果下载保存后，可清空会话。运行中的任务结束后才能清空；清空后已有上传、任务和结果下载链接失效。

上传使用文件内容副本，处理结果放在会话临时目录中，不修改用户选择的原始文件。浏览器下载时的最终保存位置由浏览器决定。

## 首版支持范围

网页提供以下常用操作，并不覆盖各领域 CLI 的全部高级选项；详细语义以对应工具文档和 `kito <领域> --help` 为准。

| 领域 | 网页操作 | 说明 |
| --- | --- | --- |
| 图片 · Lumi 露米 | compress、convert、resize、info、exif | PNG 无损验证包含透明像素 RGB；缩放涉及重采样，不属于原图像素无损 |
| 时间 · Tiko 提可 | now、from-timestamp、to-timestamp、convert、zones | 时间戳精度、时区与 DST 校验沿用 CLI |
| 编解码 · Codi 可迪 | Base64、URL、HTML 实体 | Base64 解码可下载任意二进制；URL parse 保留重复项与空值 |
| 标识 · Sumi 苏米 | hash、uuid | hash 按真实字节计算，UUID 支持 v4/v7 |
| 文本 | count、dedupe、case | 文本统计、去重与大小写转换 |
| 密钥 · Kavi 卡维 | generate、public、check | 需要 OpenSSL 3.x；私钥通过文件下载，口令通过 stdin 传给后端 |
| 视频 · Vido 维多 | convert、compress、trim、resize、mute、snapshot、gif、audio-extract、audio-convert | 需要 FFmpeg / ffprobe；重编码有损，不保证体积变小 |
| 声音 · Echo 艾可 | sfx、tone、melody、beat、presets | 内置合成，不需要声卡或外部媒体后端 |

网页图片输入仅接收通过内容签名识别的 PNG、JPEG、WebP、GIF、BMP、TIFF、ICO、AVIF、HEIC 像素容器；具体能否解码仍由实际编解码器决定。网页不接收 SVG、PDF、MVG、MSL 等矢量、文档或脚本格式，把扩展名改成 `.png` 也会拒绝；这与 CLI 的高级格式范围不同。图片部分格式需要 ImageMagick 或 zopflipng。后端路径配置沿用 CLI：`KITO_MAGICK`、`KITO_ZOPFLIPNG`、`KITO_OPENSSL`、`KITO_FFMPEG`、`KITO_FFPROBE`。缺少后端时，安装对应程序或设置路径后重新启动 Kito。

## 资源上限与保护

| 项目 | 上限 |
| --- | --- |
| 单个上传文件 | 64 MiB |
| 当前会话累计上传 | 256 MiB |
| 当前会话上传文件数量 | 64 个 |
| 文本参数 | 64 KiB |
| 同时运行任务 | 2 个 |
| 当前会话保留任务 | 64 个 |
| 单任务运行时间 | 180 秒 |
| 单任务输出文件总量 | 256 MiB |
| 当前会话输出文件总量 | 512 MiB |

各领域还会执行自身的参数和资源校验，例如声音最多 120 秒。达到会话数量或上传限额时，先保存结果，再清空会话。

输出预算由运行中的定期检查执行，超限时终止整个任务进程组并拒绝发布；这不等同于操作系统级的内存或磁盘硬配额。正常停止服务会取消任务并清理会话临时目录；强制结束进程或系统异常退出不保证完成清理。

服务拒绝任意本机路径、执行程序和自由 argv；只能处理本次上传和白名单操作。API 需要会话令牌，并校验 Host 与 Origin。页面与 API 使用 `no-store`，输入和结果不保存到浏览器持久本地存储。持有完整启动链接的人可访问当前会话，请勿分享包含令牌的链接；处理密钥时尤其需要及时下载并清理。

## 本机 API

页面使用以下接口；调用时使用终端输出的端口与令牌，并添加 `X-Kito-Token` 请求头。若添加 `Origin`，其值必须严格等于 `http://127.0.0.1:<端口>`，`Host` 也必须对应该地址和端口。

| 请求 | 用途 |
| --- | --- |
| `GET /api/info` | 版本、资源限额、后端能力 |
| `POST /api/uploads?name=<URL编码文件名>` | 请求体为文件原始字节，返回上传 ID |
| `POST /api/jobs` | 提交 `{tool, action, params, files}`，`params` 为字符串值对象，`files` 为上传 ID 数组 |
| `GET /api/jobs/<任务ID>` | 查询 running、completed 或 failed 状态、报告和结果文件 |
| `GET /api/files/<文件ID>` | 鉴权下载结果字节 |
| `DELETE /api/session` | 无运行任务时清空当前会话 |

错误返回 JSON 的 `error` 字段。结果文件 URL 不能直接作为无需认证的公共下载链接；网页使用鉴权 fetch 获取下载内容。上传 ID、任务 ID 和文件 ID 均为当前会话中的标识，不是本机路径。

## 验证范围

工程包含本机 HTTP 集成测试，覆盖鉴权、Host/Origin 拒绝、路径穿越、真实 PNG 处理、二进制下载及会话清理。测试通过仅代表执行测试的本机环境，不表示 macOS、Linux、Windows 三个平台或所有外部编解码器均已验证。
