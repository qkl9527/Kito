#!/usr/bin/env python3
"""生成确定性的 PNG 合成语料；不使用第三方素材，不将它伪称真实照片集合。"""
import argparse
import hashlib
import json
import random
import struct
import zlib
from pathlib import Path


def chunk(kind, payload):
    return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))


def write_png(path, width, height, color, depth, rows, extra=b''):
    header = struct.pack('>IIBBBBB', width, height, depth, color, 0, 0, 0)
    data = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', header) + extra
    data += chunk(b'IDAT', zlib.compress(b''.join(b'\0' + row for row in rows), 6)) + chunk(b'IEND', b'')
    path.write_bytes(data)
    return {'file': path.name, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    args.directory.mkdir(parents=True, exist_ok=True)
    rng = random.Random(20261028)
    records = []
    cases = {
        'ui-blocks': lambda x, y: (30 + (x // 8) * 25, 40 + (y // 8) * 25, 120, 255),
        'gradient': lambda x, y: (x * 4, y * 4, (x + y) * 2, 255),
        'transparent-hidden-rgb': lambda x, y: ((x * 17) % 256, (y * 13) % 256, 80, 0 if x < 32 else 255),
        'noise': lambda x, y: tuple(rng.randrange(256) for _ in range(3)) + (255,),
        'checker': lambda x, y: (255, 255, 255, 255) if (x // 4 + y // 4) % 2 else (0, 0, 0, 255),
        'solid': lambda x, y: (42, 100, 150, 255),
    }
    for name, pixel in cases.items():
        rows = [bytes(c for x in range(64) for c in pixel(x, y)) for y in range(64)]
        records.append(write_png(args.directory / (name + '.png'), 64, 64, 6, 8, rows))
    rows16 = [b''.join(struct.pack('>HHH', x * 997, y * 991, (x * 1001 + y * 67) % 65536) for x in range(32)) for y in range(32)]
    records.append(write_png(args.directory / 'precision-16.png', 32, 32, 2, 16, rows16))
    palette = bytes([30, 60, 90, 200, 10, 80, 80, 200, 40, 230, 230, 230])
    rows = [bytes(((x // 3) + (y // 4)) % 4 for x in range(64)) for y in range(64)]
    records.append(write_png(args.directory / 'indexed.png', 64, 64, 3, 8, rows, chunk(b'PLTE', palette)))
    records.append(write_png(args.directory / 'gray-1bit.png', 32, 32, 0, 1, [bytes([0xaa if y % 2 else 0x55]) * 4 for y in range(32)]))
    # 保留首版语料中的 ctk 文本，保证历史基准的字节数与 SHA-256 可复现。
    records.append(write_png(args.directory / 'gamma-text.png', 32, 32, 6, 8, [bytes([20, 40, 60, 255]) * 32 for _ in range(32)], chunk(b'gAMA', struct.pack('>I', 45455)) + chunk(b'tEXt', b'Comment\0ctk synthetic corpus, preserve metadata')))
    manifest = {'kind': 'synthetic', 'seed': 20261028, 'generator': 'scripts/generate-corpus.py', 'note': '仅用于可复现测试，不代表真实素材的总体压缩表现。', 'files': records}
    (args.directory / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    print(f'生成 {len(records)} 张合成图片：{args.directory}')


if __name__ == '__main__':
    main()
