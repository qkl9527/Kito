#!/usr/bin/env python3
"""执行真实 CLI 示例，生成供浏览器截图的终端展示页（仅使用标准库）。"""

import argparse
import html
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/kito"))
    parser.add_argument("--output-dir", type=Path, default=Path("artifacts/readme-capture"))
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    examples = [
        ("Tiko · 同一个时刻，换一个时区", [
            "time", "convert", "2024-01-01T00:00:00Z",
            "--to-timezone", "Asia/Shanghai", "--value-only",
        ]),
        ("Codi · 把文字换一种表示", ["codec", "base64", "encode", "你好，Kito"]),
        ("Sumi · 为内容留下指纹", ["id", "hash", "Kito", "--algorithm", "sha256"]),
    ]
    # 独立配置目录避免读取开发机设置；真实执行失败就中断，不生成虚构结果。
    with tempfile.TemporaryDirectory(prefix="kito-readme-") as config:
        env = {**os.environ, "KITO_CONFIG_DIR": config, "KITO_TIMEZONE": "UTC", "NO_COLOR": "1"}
        version = subprocess.run(
            [str(binary), "--version"], check=True, capture_output=True, text=True, env=env,
        ).stdout.strip()
        transcript = []
        for caption, arguments in examples:
            result = subprocess.run(
                [str(binary), *arguments], check=True, capture_output=True, text=True, env=env,
            )
            transcript.append({
                "caption": caption,
                "command": shlex.join(["kito", *arguments]),
                "stdout": result.stdout,
                "stderr": result.stderr,
            })

    # 保留原始输出作为截图来源；HTML 只加外框、标题和配色，并转义所有输出。
    (args.output_dir / "cli-transcript.json").write_text(
        json.dumps({"version": version, "examples": transcript}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    blocks = "".join(
        '<section><p class="caption">' + html.escape(item["caption"]) + '</p>'
        '<pre class="command"><span class="prompt">❯</span> ' + html.escape(item["command"]) + '</pre>'
        '<pre class="output">' + html.escape(item["stdout"]) + '</pre></section>'
        for item in transcript
    )
    document = """<!doctype html>
<html lang="zh-CN"><meta charset="utf-8"><title>Kito CLI · 真实运行输出</title>
<style>
* { box-sizing: border-box; }
body { margin: 0; color: #292e34; background: #f2f0e9;
  font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif; }
main { width: 1440px; padding: 52px 64px 42px; }
header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 30px; }
h1 { font-size: 38px; font-weight: 650; letter-spacing: -1px; margin: 0; }
header p { font-size: 17px; color: #6a706c; margin: 12px 0 0; }
.brand { font-size: 42px; font-weight: 800; letter-spacing: -2px; }
.brand i { display: inline-block; width: 21px; height: 21px; border-radius: 7px;
  background: #ecd178; margin-right: 12px; transform: rotate(-10deg); border: 2px solid #39413c; }
.terminal { background: #242b2d; color: #f2f1e8; border-radius: 16px; overflow: hidden;
  box-shadow: 0 20px 40px #283a2a18; border: 1px solid #394440; }
.bar { height: 60px; display: flex; align-items: center; padding: 0 26px;
  background: #2e3638; border-bottom: 1px solid #45504c; gap: 9px; }
.dot { width: 11px; height: 11px; border-radius: 50%; background: #ce927e; }
.dot:nth-child(2) { background: #e4c46c; }.dot:nth-child(3) { background: #9aba9e; }
.bar span { color: #b8c3bf; font-size: 15px; margin-left: 18px; }
.bar small { margin-left: auto; color: #b8c3bf; font-size: 14px; }
.body { padding: 0 34px; }
section { padding: 24px 0 26px; border-bottom: 1px solid #3c4743; }
section:last-child { border-bottom: 0; }
.caption { color: #a6b5ad; font-size: 16px; margin: 0 0 12px; }
pre { font-family: "SFMono-Regular", Menlo, "PingFang SC", monospace;
  font-size: 20px; line-height: 1.6; white-space: pre-wrap; overflow-wrap: anywhere; margin: 0; }
.command { color: #f2e5b6; }.prompt { color: #a4c6ac; }
.output { color: #dce6e0; margin-top: 8px; }
footer { display: flex; justify-content: space-between; color: #71766e; font-size: 15px; margin-top: 25px; }
</style>
<main><header><div><h1>小事交给伙伴，专注手头的工作。</h1>
<p>时间换算、编解码、内容指纹，一条命令就出发。</p></div><div class="brand"><i></i>kito</div></header>
<div class="terminal"><div class="bar"><i class="dot"></i><i class="dot"></i><i class="dot"></i>
<span>奇托工具小队 · CLI</span><small>VERSION</small></div><div class="body">BLOCKS</div></div>
<footer><span>各有所长，一起把事情做好。</span><span>实际 CLI 输出 · 仅添加展示外框与配色</span></footer></main>
</html>"""
    (args.output_dir / "cli-demo.html").write_text(
        document.replace("VERSION", html.escape(version)).replace("BLOCKS", blocks), encoding="utf-8",
    )
    print(args.output_dir / "cli-demo.html")


if __name__ == "__main__":
    main()
