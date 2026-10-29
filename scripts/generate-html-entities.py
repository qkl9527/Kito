#!/usr/bin/env python3
"""从本地 WHATWG entities.json 生成严格带分号的完整 Rust 实体表。

来源：https://html.spec.whatwg.org/entities.json
用法：python3 scripts/generate-html-entities.py PATH_TO_ENTITIES_JSON
不联网；更新数据前由维护者下载官方 JSON，生成注释记录其 SHA-256。
"""
import argparse
import hashlib
import json
from pathlib import Path


def generate(source: Path) -> str:
    raw = source.read_bytes()
    data = json.loads(raw)
    pairs = []
    for name, entry in data.items():
        if not name.endswith(";"):
            continue
        if not name.startswith("&") or not name[1:-1].isascii() or not name[1:-1].isalnum():
            raise ValueError(f"非法实体名：{name!r}")
        value = entry["characters"]
        # 逐 scalar 核对官方两个表示，防止任何多码点被截断或 surrogate 混入。
        if [ord(character) for character in value] != entry["codepoints"]:
            raise ValueError(f"实体的 characters 与 codepoints 不一致：{name}")
        if not value or any(0xD800 <= ord(character) <= 0xDFFF for character in value):
            raise ValueError(f"实体包含无效 Unicode scalar：{name}")
        pairs.append((name[1:-1], value))
    pairs.sort()
    if not pairs or len({name for name, _ in pairs}) != len(pairs):
        raise ValueError("实体集合为空或存在重复名称")
    lines = [
        "//! 完整 HTML5 命名实体标准表；实体名不含 & 和分号。",
        "//! 来源：https://html.spec.whatwg.org/entities.json（WHATWG HTML Living Standard）",
        "//! Copyright © WHATWG (Apple, Google, Mozilla, Microsoft).",
        "//! SPDX-License-Identifier: BSD-3-Clause；许可全文见 WHATWG-LICENSE.txt。",
        "//! 官方许可：https://html.spec.whatwg.org/multipage/acknowledgements.html#ipr",
        f"//! 源文件 SHA-256：{hashlib.sha256(raw).hexdigest()}",
        "//! 生成命令：python3 scripts/generate-html-entities.py PATH_TO_ENTITIES_JSON",
        "//! 仅保留带分号条目；保留 characters 的全部 Unicode scalar。",
        f"pub static NAMED_ENTITIES: [(&[u8], &str); {len(pairs)}] = [",
    ]
    for name, value in pairs:
        escaped = "".join(f"\\u{{{ord(character):X}}}" for character in value)
        lines.append(f'    (b"{name}", "{escaped}"),')
    lines.append("];")
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="已下载的官方 entities.json")
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "src/codec/entities.rs")
    args = parser.parse_args()
    result = generate(args.source)
    args.output.write_text(result, encoding="utf-8")
    print(f"已生成 {args.output}，来源 SHA-256：{hashlib.sha256(args.source.read_bytes()).hexdigest()}")


if __name__ == "__main__":
    main()
