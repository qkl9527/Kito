# Kito 发布流程

推送 `v<semver>` tag 后，GitHub Actions 的 `Build and release tags` 工作流会校验版本、安装真实后端、运行检查与测试、在五个平台原生构建，并将压缩包和 `SHA256SUMS` 发布到同名 GitHub Release。仓库只保留这一套构建发布流程：普通分支 push、Pull Request 和本地 tag 均不触发构建或发布，也不提供 `workflow_dispatch` 手动触发入口。

## 发布前准备

1. 在 `Cargo.toml` 修改 `[package].version`，例如 `0.2.0`。使用 `cargo check` 同步 `Cargo.lock` 中本项目的版本并审阅 diff；使用仓库本地 Rust 时，改用 `scripts/cargo-local.sh check`。
2. 更新相关文档，执行项目检查：

   ```sh
   scripts/cargo-local.sh fmt --check
   scripts/cargo-local.sh clippy --locked --all-targets -- -D warnings
   scripts/cargo-local.sh test --locked
   python3 scripts/release.py validate --tag v0.2.0
   ```

   发布脚本需要 Python 3.11 或更新版本。GitHub Actions 固定使用 Python 3.12。真实后端集成验证遵循项目约定，在后端齐备时设置 `KITO_REQUIRE_BACKEND=1`。

3. 确认本地检查通过，再由维护者审阅、提交并推送代码。普通代码推送不运行远程构建；五个平台的检查会在推送版本 tag 后执行，全部通过才会发布。必须先经过维护者明确确认，自动化代理不得自行 commit、push、打 tag 或发布。
4. 确认待发布提交包含 `.github/workflows/release.yml`，版本三处一致。以下命令由维护者在决定发布后执行；请将示例版本替换为实际版本：

   ```sh
   git tag -a v0.2.0 -m "Kito v0.2.0"
   git push origin v0.2.0
   ```

`v0.2.0-rc.1` 等带 SemVer prerelease 标识的 tag 会标记为 GitHub 预发布，对应 Cargo 版本必须是 `0.2.0-rc.1`。`v1.2`、`v01.2.3` 等非标准版本会失败；tag、`Cargo.toml`、`Cargo.lock`、二进制 `kito --version` 必须完全一致。不要将已发布 tag 移到新提交。

## 产物与平台边界

以 `v0.1.0` 为例，Release 包含以下文件：

| 系统与架构 | Native runner | 压缩包 |
| --- | --- | --- |
| macOS Apple Silicon | `macos-15` | `kito-v0.1.0-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `macos-15-intel` | `kito-v0.1.0-x86_64-apple-darwin.tar.gz` |
| Linux ARM64 | `ubuntu-24.04-arm` | `kito-v0.1.0-aarch64-unknown-linux-gnu.tar.gz` |
| Linux x86_64 | `ubuntu-24.04` | `kito-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` |
| Windows x86_64 | `windows-2025` | `kito-v0.1.0-x86_64-pc-windows-msvc.zip` |

每个压缩包内有一个 `kito-<tag>-<target>/` 顶层目录，包含可执行文件 `kito`（Windows 为 `kito.exe`）、`README.md`、`LICENSE`、`docs/`，以及供依赖溯源的 `Cargo.toml`、`Cargo.lock`。网页静态资源已经编译进可执行文件，无需另装 Node.js 或前端服务器。

- **外部后端未打包**：ImageMagick、zopflipng、OpenSSL 3.x、FFmpeg / ffprobe 仍按所用能力单独安装。参考各工具文档中的环境变量与 doctor 检查。声音合成不依赖外部媒体后端。
- **Linux**：使用 Ubuntu 24.04 的 GNU/glibc 工具链，不是 musl 静态包；不承诺兼容更旧的 glibc、Alpine 或所有 Linux 发行版。
- **macOS 与 Windows**：本流程不提供 Developer ID 签名、公证或 Authenticode 签名；下载后仍可能受到操作系统安全检查。Windows ARM64、Windows x86 和其他 CPU 架构未提供独立产物。
- **验证边界**：每个平台的构建作业安装 ImageMagick 7、OpenSSL 3.x、FFmpeg / ffprobe，并设置 `KITO_REQUIRE_BACKEND=1`，运行 fmt、clippy、测试、`cargo build --locked --release`、二进制版本与帮助检查。任何平台检查或构建失败都会阻止发布，不再有另一套独立 CI 重复构建。这里只列出配置目标，只有对应 GitHub run 实际成功才构成该平台的运行证据，也不代表所有操作系统版本兼容。

## 下载后校验

所有压缩包共用一个 `SHA256SUMS`。将它与下载的压缩包放在同一个目录，在 Linux 上校验已下载的包：

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

macOS 可运行 `shasum -a 256 kito-v0.1.0-aarch64-apple-darwin.tar.gz`，Windows PowerShell 可运行 `Get-FileHash .\kito-v0.1.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256`，将结果与 `SHA256SUMS` 中对应文件的摘要对照。摘要校验用于发现下载损坏，不替代代码签名。

## 失败、重跑与权限

工作流只由 `push.tags: v*` 触发。所有 build 成功后，publish 作业才下载五套资产，核对文件集合与每个 SHA256；缺失、重复、额外文件或摘要不一致均阻止发布。构建使用 `Cargo.lock` 锁定依赖，Rust stable 和 runner 镜像仍会更新，因此不承诺不同时间构建的二进制逐字节相同。

发布过程先检查远端 tag 指向本次提交，再创建 draft，上传五个压缩包与摘要清单，复核远端 tag、draft 和资产后才公开。GitHub 的这些 API 请求不是事务：失败或被取消时可能留下 draft，但流程不会主动公开只上传了一部分的 Release。

- 构建失败且还未创建 Release：修复环境后可在 Actions 重跑。重跑只会替换该 workflow run 的临时 artifact。
- 上传或公开失败且留有 draft：先人工确认 draft 与资产，决定删除该 draft 后重跑，或人工完成发布。本流程不自动删除或复用它。
- 已有同名 Release，无论正式、预发布还是 draft：重跑会明确失败，不覆盖资产、不修改说明、不自动删除 Release。已公开版本需要修复时，发布新的版本 tag。
- 同 tag 的发布工作流串行运行，`cancel-in-progress: false`；这是工作流级保护，不能阻止维护者手工或其他工作流修改 Release。建议为版本 tag 配置仓库 ruleset，禁止移动或删除。
- 默认 `contents: read`，仅 publish 作业使用 `contents: write` 和内置 `GITHUB_TOKEN`，无需个人访问令牌。checkout 不保留 Git 凭据。仓库须启用 Actions，并允许该工作流申请所需写权限；组织策略可能限制可用 runner 或权限。

本实现面向 GitHub.com 的 GitHub-hosted runners；没有验证 GitHub Enterprise Server、自托管 runner 或自定义 API 主机。配置文件落地本身不会创建远端 Release；实际推送版本 tag 才会启动发布。

## 离线验证发布辅助脚本

```sh
python3 -m unittest discover -s scripts -p 'release_test.py'
python3 scripts/release.py validate --tag v0.1.0
```

测试覆盖版本拒绝、归档布局、执行权限、完整资产集、摘要损坏、已存在 Release、tag 移动、上传失败不公开，以及 prerelease 标记。测试中的 GitHub 操作全部由 fake 替代，不会发出真实发布请求。

工作流使用 Node.js 24 版本的官方 Actions：`checkout@v7`、`setup-python@v7`、`upload-artifact@v7` 与 `download-artifact@v8`。Runner 与 API 行为参考 [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)、[upload-artifact v7](https://github.com/actions/upload-artifact/tree/v7)、[download-artifact v8](https://github.com/actions/download-artifact/tree/v8) 和 [GitHub Release REST API](https://docs.github.com/en/rest/releases/releases)。
