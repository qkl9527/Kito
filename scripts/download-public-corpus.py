#!/usr/bin/env python3
"""下载固定版本的 libpng/PngSuite 小型公开验证集合，同时保存来源和许可。"""
import argparse
import hashlib
import json
import urllib.request
from pathlib import Path

COMMIT = 'ac0c3532476fa4876a83a279127840741cf216f2'
BASE = f'https://raw.githubusercontent.com/pnggroup/libpng/{COMMIT}/'
FILES = {
    'pngtest.png': 'pngtest.png',
    'basn6a08.png': 'contrib/pngsuite/basn6a08.png',
    'basn2c16.png': 'contrib/pngsuite/basn2c16.png',
    'basn0g01.png': 'contrib/pngsuite/basn0g01.png',
    'LICENSE.libpng': 'LICENSE',
    'README.pngsuite': 'contrib/pngsuite/README',
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    args.directory.mkdir(parents=True, exist_ok=True)
    records = []
    for name, relative in FILES.items():
        url = BASE + relative
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read()
        destination = args.directory / name
        if destination.exists() and destination.read_bytes() != data:
            raise SystemExit(f'目标已有不同内容，拒绝覆盖：{destination}')
        destination.write_bytes(data)
        records.append({'file': name, 'url': url, 'sha256': hashlib.sha256(data).hexdigest()})
    manifest = {
        'upstream': 'pnggroup/libpng', 'commit': COMMIT,
        'attribution': 'PNG Reference Library Authors；PngSuite 图片作者 Willem van Schaik。',
        'license_files': ['LICENSE.libpng', 'README.pngsuite'],
        'purpose': '小型兼容性与压缩验证集合，不代表真实照片分布。', 'files': records,
    }
    (args.directory / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    print(f'公开验证素材和许可已保存：{args.directory}')


if __name__ == '__main__':
    main()
