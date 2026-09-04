#!/usr/bin/env python3
"""mk_ahci.py — W20: AHCI 参考盘
  sdk/ahci.img      4MiB FJFS 卷盘 (2048 簇; m135 文件系统; 前 8 扇区参考模式)
  sdk/ahci-mini.img 4KB 裸盘 (8 扇区仅参考模式; m134 单扇区回读)
"""
import struct
import os

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def pattern(size_bytes, nsecs):
    d = bytearray(size_bytes)
    for sec in range(min(nsecs, size_bytes // 512)):
        for k in range(128):
            off = sec * 512 + k * 4
            d[off:off + 4] = struct.pack("<I", sec)
    return bytes(d)


with open(os.path.join(root, "sdk", "ahci.img"), "wb") as f:
    f.write(pattern(4 * 1024 * 1024, 8))
with open(os.path.join(root, "sdk", "ahci-mini.img"), "wb") as f:
    f.write(pattern(4096, 8))
print("mk_ahci: ahci.img (4MiB) + ahci-mini.img (4KB)")
