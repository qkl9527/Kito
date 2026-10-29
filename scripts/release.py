#!/usr/bin/env python3
"""验证版本、打包和发布 Kito；仅使用 Python 3.11+ 标准库。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import zipfile


ROOT = Path(__file__).resolve().parent.parent
TARGETS = {
    "aarch64-apple-darwin": ".tar.gz",
    "x86_64-apple-darwin": ".tar.gz",
    "aarch64-unknown-linux-gnu": ".tar.gz",
    "x86_64-unknown-linux-gnu": ".tar.gz",
    "x86_64-pc-windows-msvc": ".zip",
}
# SemVer 的数字标识符不能有前导零；build metadata 可包含数字前导零。
NUMBER = r"(?:0|[1-9][0-9]*)"
IDENTIFIER = rf"(?:{NUMBER}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
TAG_PATTERN = re.compile(
    rf"v(?P<version>{NUMBER}\.{NUMBER}\.{NUMBER}"
    rf"(?:-(?P<pre>{IDENTIFIER}(?:\.{IDENTIFIER})*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?)"
)


def tag_version(tag):
    match = TAG_PATTERN.fullmatch(tag)
    if match is None:
        raise ValueError("tag 必须为 v<semver>，例如 v0.1.0 或 v0.2.0-rc.1")
    return match["version"], match["pre"] is not None


def validate_version(tag, root=ROOT):
    version, prerelease = tag_version(tag)
    with (root / "Cargo.toml").open("rb") as source:
        package = tomllib.load(source)["package"]
    with (root / "Cargo.lock").open("rb") as source:
        locked = tomllib.load(source)["package"]
    # 忽略 registry 中可能出现的同名包，只接受本项目的唯一锁定版本。
    versions = [p["version"] for p in locked if p["name"] == "kito" and "source" not in p]
    if package["name"] != "kito" or package["version"] != version or versions != [version]:
        raise ValueError("tag、Cargo.toml 与 Cargo.lock 的 kito 版本必须完全一致")
    return version, prerelease


def checksum(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def archive_name(tag, target):
    return f"kito-{tag}-{target}{TARGETS[target]}"


def package(tag, target, output, root=ROOT):
    version, _ = validate_version(tag, root)
    # 显式核对 host，避免未来矩阵修改后误把交叉构建当作 native 验证。
    rustc = subprocess.check_output(["rustc", "-vV"], encoding="utf-8")
    if f"host: {target}" not in rustc.splitlines():
        raise ValueError(f"runner 的 Rust host 与目标 {target} 不一致")
    binary_name = "kito.exe" if target.endswith("windows-msvc") else "kito"
    binary = root / "target" / target / "release" / binary_name
    actual = subprocess.check_output([str(binary), "--version"], encoding="utf-8").strip()
    if actual != f"kito {version}":
        raise ValueError(f"二进制版本不一致：{actual!r}，期望 kito {version}")
    subprocess.run([str(binary), "--help"], check=True, stdout=subprocess.DEVNULL)
    output.mkdir(parents=True, exist_ok=True)
    archive = output / archive_name(tag, target)
    digest_file = output / f"{archive.name}.sha256"
    if archive.exists() or digest_file.exists():
        raise ValueError(f"打包目标已存在，拒绝覆盖：{archive}")
    # 归档内只有一个顶层目录；先在临时目录完成，再将完整文件移至输出目录。
    with tempfile.TemporaryDirectory(prefix="kito-release-", dir=output) as temporary:
        temporary = Path(temporary)
        staged = temporary / f"kito-{tag}-{target}"
        staged.mkdir()
        shutil.copy2(binary, staged / binary_name)
        # 许可证随所有平台的二进制归档一同分发，保留版权与授权声明。
        for name in ("README.md", "LICENSE", "Cargo.toml", "Cargo.lock"):
            shutil.copy2(root / name, staged / name)
        shutil.copytree(root / "docs", staged / "docs")
        candidate = temporary / archive.name
        if TARGETS[target] == ".zip":
            with zipfile.ZipFile(candidate, "x", compression=zipfile.ZIP_DEFLATED) as bundle:
                for path in sorted(staged.rglob("*")):
                    if path.is_file():
                        bundle.write(path, path.relative_to(temporary))
        else:
            with tarfile.open(candidate, "w:gz") as bundle:
                bundle.add(staged, arcname=staged.name)
        digest = checksum(candidate)
        candidate.rename(archive)
        digest_file.write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
    print(f"已验证并打包：{archive}")


def assemble(tag, artifacts, output):
    """只接受完整五平台资产，并逐个复核构建作业产生的摘要。"""
    tag_version(tag)
    expected = {archive_name(tag, target) for target in TARGETS}
    expected_files = expected | {f"{name}.sha256" for name in expected}
    files = [path for path in artifacts.rglob("*") if path.is_file()]
    if len(files) != len(expected_files) or {p.name for p in files} != expected_files:
        raise ValueError("构建资产缺失、重复或包含未知文件，拒绝发布")
    by_name = {path.name: path for path in files}
    lines = []
    for name in sorted(expected):
        digest = checksum(by_name[name])
        line = f"{digest}  {name}\n"
        if by_name[f"{name}.sha256"].read_text(encoding="utf-8") != line:
            raise ValueError(f"SHA256 校验失败：{name}")
        lines.append(line)
    # 不合并到非空目录，防止陈旧文件被意外上传。
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        raise ValueError(f"发布目录必须为空：{output}")
    for name in sorted(expected):
        shutil.copy2(by_name[name], output / name)
    (output / "SHA256SUMS").write_text("".join(lines), encoding="utf-8")
    return sorted(output.iterdir())


class GitHub:
    """显式区分 404 与认证/网络失败；绝不把失败查询当作 Release 不存在。"""

    def __init__(self, repository, token):
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
            raise ValueError("GITHUB_REPOSITORY 格式无效")
        if not token:
            raise ValueError("发布需要 GH_TOKEN")
        self.base = f"https://api.github.com/repos/{repository}"
        self.token = token

    def request(self, method, path, payload=None, asset=None, missing_ok=False):
        url = path if path.startswith("https://") else self.base + path
        # GitHub 返回的 upload_url 只能携带令牌发送到官方上传端点。
        if urllib.parse.urlparse(url).netloc not in {"api.github.com", "uploads.github.com"}:
            raise ValueError("拒绝将发布令牌发送到未知主机")
        data = asset.read_bytes() if asset else None
        if payload is not None:
            data = json.dumps(payload).encode("utf-8")
        request = urllib.request.Request(url, data=data, method=method, headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {self.token}",
            "X-GitHub-Api-Version": "2022-11-28",
            "Content-Type": "application/octet-stream" if asset else "application/json",
            "User-Agent": "kito-release",
        })
        try:
            with urllib.request.urlopen(request, timeout=120) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            code = error.code
            error.close()
            if missing_ok and code == 404:
                return None
            raise RuntimeError(f"GitHub {method} 请求失败：HTTP {code}") from error


def publish_release(api, tag, commit, assets):
    """只有完整上传后才公开；任何失败均保留 draft，且重跑不自动修改已有 Release。"""
    _, prerelease = tag_version(tag)
    encoded_tag = urllib.parse.quote(tag, safe="")
    if api.request("GET", f"/releases/tags/{encoded_tag}", missing_ok=True) is not None:
        raise ValueError("该 tag 已有 Release（包括 draft）；拒绝覆盖，请先人工检查")
    # 按 tag 查询只保证返回已发布版本。用有写权限的令牌遍历完整列表，
    # 将较旧页面上的 draft 也纳入查重，避免重跑时创建第二个同 tag draft。
    page = 1
    while True:
        releases = api.request("GET", f"/releases?per_page=100&page={page}")
        if any(item["tag_name"] == tag for item in releases):
            raise ValueError("该 tag 已有 Release（包括 draft）；拒绝覆盖，请先人工检查")
        if len(releases) < 100:
            break
        page += 1
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("GITHUB_SHA 必须是完整 commit SHA")
    remote = api.request("GET", f"/commits/{encoded_tag}")
    if remote["sha"] != commit:
        raise ValueError("远端 tag 已移动或不再指向本次构建提交，拒绝发布")
    body = (
        "五个平台的原生构建压缩包及 SHA256SUMS 见附件。\n\n"
        "解压后运行 `kito --version`。ImageMagick、zopflipng、OpenSSL、"
        "FFmpeg / ffprobe 等外部后端未打包，按所用功能单独安装。"
        "Linux 包基于 Ubuntu 24.04 / glibc，不能视为通用静态 Linux 二进制。\n\n"
        "发布流程与平台边界见包内 `docs/releasing.md`。"
    )
    release = api.request("POST", "/releases", {
        "tag_name": tag,
        "target_commitish": commit,
        "name": f"Kito {tag}",
        "body": body,
        "draft": True,
        "prerelease": prerelease,
        "generate_release_notes": True,
    })
    release_path = f"/releases/{release['id']}"
    upload_url = release["upload_url"].split("{", 1)[0]
    for asset in assets:
        url = upload_url + "?" + urllib.parse.urlencode({"name": asset.name})
        uploaded = api.request("POST", url, asset=asset)
        if (uploaded["state"] != "uploaded" or uploaded["size"] != asset.stat().st_size
                or uploaded["name"] != asset.name):
            raise ValueError(f"资产上传未完整完成：{asset.name}；Release 保留 draft")
    # 上传和公开是不同请求；再次确认远端 tag 和 draft 内容，缩小误发布窗口。
    if api.request("GET", f"/commits/{encoded_tag}")["sha"] != commit:
        raise ValueError("上传期间远端 tag 已移动；Release 保留 draft")
    current = api.request("GET", release_path)
    actual = {(asset["name"], asset["size"]) for asset in current["assets"]}
    expected = {(asset.name, asset.stat().st_size) for asset in assets}
    if (not current["draft"] or current["tag_name"] != tag or actual != expected
            or len(current["assets"]) != len(assets)
            or any(asset["state"] != "uploaded" for asset in current["assets"])):
        raise ValueError("draft 状态或资产清单发生变化，拒绝继续发布")
    published = api.request("PATCH", release_path, {"draft": False, "prerelease": prerelease})
    if published["draft"]:
        raise ValueError("GitHub 未确认公开 Release，请人工检查远端状态")
    print(f"已发布：{published['html_url']}")


def main():
    # Windows 重定向到 Actions 日志时可能使用 cp1252，中文完成日志也会使成功打包报错。
    # 在 argparse 前统一标准流，覆盖帮助、成功日志和异常诊断；异常路径字节保留转义形式。
    sys.stdout.reconfigure(encoding="utf-8", errors="backslashreplace")
    sys.stderr.reconfigure(encoding="utf-8", errors="backslashreplace")
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    for command in ("validate", "package", "publish"):
        child = subcommands.add_parser(command)
        child.add_argument("--tag", required=True)
        if command != "validate":
            child.add_argument("--output", type=Path, required=True)
        if command == "package":
            child.add_argument("--target", choices=TARGETS, required=True)
        if command == "publish":
            child.add_argument("--artifacts", type=Path, required=True)
    args = parser.parse_args()
    validate_version(args.tag)
    if args.command == "validate":
        print(f"版本一致：{args.tag}")
    elif args.command == "package":
        package(args.tag, args.target, args.output)
    else:
        assets = assemble(args.tag, args.artifacts, args.output)
        api = GitHub(os.environ.get("GITHUB_REPOSITORY", ""), os.environ.get("GH_TOKEN", ""))
        publish_release(api, args.tag, os.environ.get("GITHUB_SHA", ""), assets)


if __name__ == "__main__":
    main()
