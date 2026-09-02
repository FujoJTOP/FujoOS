#!/usr/bin/env python3
"""mk_vblk.py — W13: 生成 virtio-blk 参考盘 (sdk/vblk.img, 1MiB raw)。
模式: 每字节 = (offset % 256) —— demo 逐字节比对 (扇区级判别, 无周期误判)。
"""
import sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "sdk/vblk.img"
size = 1 << 20
with open(OUT, "wb") as f:
    f.write(bytes(range(256)) * (size // 256))
print(f"mk_vblk: wrote {OUT} ({size} bytes, i%256 pattern)")
