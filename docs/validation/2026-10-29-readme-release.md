# README 与 tag 发布流程验证

日期：2026-10-29。执行环境：本机 macOS arm64。未执行 git commit、push、打 tag 或远端 Release 写操作。

## 本次变更

- README 增加 CLI 实际输出展示图、正式 Web 首页、PNG 处理完成页及声音合成完成页；过程图放在可展开区域。
- 增加底层项目、外部后端与设计参考链接，明确 Tone.js 等不属于运行时依赖。
- 增加 tag 触发的五平台 Release 工作流，以及版本验证、归档、校验和和 draft 发布辅助脚本。
- 增加下载、版本与发布操作说明，保留用户确认后才能提交和发布的约定。

## 验证结果

以下检查通过：

```sh
scripts/cargo-local.sh fmt --check
scripts/cargo-local.sh clippy --locked --all-targets -- -D warnings
KITO_OPENSSL=/opt/homebrew/opt/openssl@3/bin/openssl KITO_REQUIRE_BACKEND=1 scripts/cargo-local.sh test --locked
scripts/cargo-local.sh build --locked --release
scripts/cargo-local.sh build --locked --release --target aarch64-apple-darwin
PYTHONDONTWRITEBYTECODE=1 python3.12 -m unittest discover -s scripts -p 'release_test.py' -v
PYTHONDONTWRITEBYTECODE=1 python3.12 scripts/release.py validate --tag v0.1.0
python3 scripts/render-readme-demo.py
git diff --check
```

- Rust 测试：278 项通过，0 失败、0 ignored，强制要求实际媒体与密钥后端。
- 发布脚本：11 项离线测试通过，覆盖版本、原生目标、二进制版本、归档、校验和、资产完整性、已发布版本、分页中的 draft、tag 移动、上传失败和 prerelease。GitHub 写操作全部使用 fake，测试日志中的发布 URL 不是真实发布。
- 使用真实本机 release 二进制生成 ARM64 `.tar.gz`，复核 SHA-256、可执行权限、包内二进制/文档/Cargo 元数据，解压后 `--version` 和 `--help` 正常。
- actionlint 1.7.7 静态检查通过；该旧版不认识 `macos-15-intel`，仅忽略该 runner 标签提示。标签与架构已经按 GitHub 官方 runner 文档核对，未忽略其他规则。
- README、截图来源与发布文档中的 35 个本地 Markdown 链接目标存在；CLI 展示输出用标准库独立核对。

actionlint 命令：

```sh
actionlint -ignore 'label "macos-15-intel" is unknown' .github/workflows/release.yml
```

## 截图与浏览器操作

使用重新构建的 `target/release/kito web` 启动独立本机会话，完成 PNG 上传/压缩和 `success` 音效合成/预览/下载。下载 WAV 已检查为 PCM16 单声道，44100 Hz、39690 帧、0.90 秒。PNG 示例保留原编码，体积不变，README 如实说明。

四张截图均已逐张检查：中文、命令、导航、结果与下载控件可见，没有使用设计稿代替真实页面。CLI 原始输出和素材来源见 [截图说明](../assets/README.md)。截图后已关闭本次浏览器页面和本机服务。

## 边界

- 尚未推送 tag，未运行 GitHub Actions 的五平台 Release 工作流；本机结果不代表其他平台已经通过。
- 发布工作流与强制后端 CI 独立，维护者需在打 tag 前检查原有 CI。发行包不捆绑外部后端，也没有代码签名或公证。
- 发布 API 不是事务；失败可能留下 draft。流程不自动覆盖、删除或复用已有 Release，重试前需要维护者检查。
- 上游项目链接根据本地依赖元数据及既有官方来源核对；本次通用 web 检索受 HTTP 429 限流，未声称逐一在线验证所有链接。
