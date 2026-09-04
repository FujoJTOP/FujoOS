#!/usr/bin/env python3
"""mk_ahci.py — W20: AHCI 参考盘 (8 扇区; 扇区 i 每 u32 = i)"""
import struct
import os

out = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "sdk", "ahci.img")
d = bytearray(4096)
for sec in range(8):
    for k in range(128):
        off = sec * 512 + k * 4
        d[off:off + 4] = struct.pack("<I", sec)
with open(out, "wb") as f:
    f.write(d)
print(f"mk_ahci: {os.path.getsize(out)} bytes -> {out}")
