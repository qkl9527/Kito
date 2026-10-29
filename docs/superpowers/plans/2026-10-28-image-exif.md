# 图片 EXIF 读取

## 范围

新增 `kito image exif INPUT [--json] [--memory-mib N]`，默认中文可读输出，JSON 保留标签名称、编号、上下文、IFD、类型、解析值与带单位的展示值。独立模块负责读取，不修改文件，不依赖 ImageMagick。

首版读取 JPEG APP1、TIFF、PNG eXIf、WebP EXIF，按内容识别而非扩展名。只提取和解析元数据，不宣称验证图片像素完整性。HEIC/AVIF、BigTIFF、独立 XMP/IPTC 和厂商 MakerNote 深度解码暂不支持。未发现字段返回成功、空数组；容器/元数据解析错误返回非零，不输出半截报告。

## 实现与验证

1. 用可复现的小 TIFF 及容器夹具编写 CLI 测试，先确认新增命令不存在时失败。
2. 接入 `kamadak-exif`，在解析 TIFF 前检查目录、字段数量和累计数据量，防止重复指针或共享大字段造成资源放大；所有读取有文件预算。
3. JSON 保留有理数分子/分母与 ASCII 字节；普通文本转义控制字符。保留重复标签与不同 IFD 的相同标签。
4. 更新图片工具文档、README 索引与简短用法，运行格式、lint、全量测试和 release 构建。本地验证不能代替 Linux/Windows CI。

不执行 commit、推送或发布。
