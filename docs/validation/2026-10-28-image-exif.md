# EXIF 读取验证

日期：2026-10-28。环境：本地 macOS arm64，仓库独立 Rust 工具链。没有运行 Linux/Windows 远程 CI。

## 已完成

- 新增 `kito image exif INPUT [--json] [--memory-mib N]`，支持 JPEG、TIFF、PNG、WebP；内置 `kamadak-exif 0.6.1`，无需 ImageMagick。
- 普通文本显示标签、IFD、分组、编号与带单位的值；JSON 保留类型、字节与分数精度。
- 无字段正常返回，损坏/不支持/超预算返回错误，成功与失败均不修改原图。
- 格式、字段、展示文字三级资源检查；关联单位重复展开在格式化写入时限额，失败不输出部分报告。

## 检查结果

以下命令均通过：

```sh
scripts/cargo-local.sh fmt --check
scripts/cargo-local.sh clippy --locked --all-targets -- -D warnings
KITO_REQUIRE_BACKEND=1 KITO_ZOPFLIPNG="$PWD/.toolchain/zopfli-src/zopflipng" scripts/cargo-local.sh test --locked
scripts/cargo-local.sh build --locked --release
./target/release/kito image exif --help
```

全量 113 项测试条目通过（包含既有后端测试辅助入口），其中 `tests/exif.rs` 新增 15 项：四种容器、按内容识别、原图不变、无 EXIF、损坏 EXIF、PNG CRC/截断、WebP 长度/padding/重复块、IFD 与 GPS、精确分数、非 UTF-8、零分母、控制字符、文件/字段/展示预算以及重复目录。

先确认新命令未注册时的失败，再完成实现；审查发现关联单位字段可能重复放大展示文字，新增回归测试先复现失败，修复后文本与 JSON 均在超限时返回非零、stdout 为空。

另用 release 可执行文件读取已安装 `kamadak-exif 0.6.1` 包内的测试素材（未复制进仓库，也未访问用户照片）：

| 素材 | 格式 | 字段数 |
| --- | --- | ---: |
| `exif.jpg` | JPEG | 10 |
| `exif.tif` | TIFF | 19 |
| `exif.png` | PNG | 6 |
| `exif.webp` | WebP | 6 |
| `yaminabe.tif` | Big-endian TIFF | 26 |
| `yaminale.tif` | Little-endian TIFF | 26 |

## 范围

本命令不进行图片像素完整性验证；JPEG 读取首个 EXIF APP1。HEIC/HEIF/AVIF、BigTIFF、独立 XMP/IPTC 和厂商 MakerNote 内部结构不在本轮范围。默认及可调预算见 [图片工具说明](../tools/image.md#读取-exif-信息)。

未执行 git commit、推送或发布。
