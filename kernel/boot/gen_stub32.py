#! /usr/bin/env python3
"""gen_stub32.py — 生成 FujoOS 32 位引导桩 + 页表 (kernel/boot_blob.bin)

本脚本把下列汇编手写为机器码并输出 (纯 Python 标准库, 无外部工具):
  .stub32  (blob 偏移 0x0000, 物理 0x101000)  可执行代码
  .tables  (blob 偏移 0x1000.., 物理 0x102000..)  页表数据

流程: 32 位保护模式 (QEMU multiboot v1 入口)
  cli
  mov esp, 0x300000                  ; 内核栈顶 (恒等映射内)
  mov edi, eax                       ; eax = multiboot magic (0x2BADB002)
  mov esi, ebx                       ; ebx = multiboot info 指针
  mov eax, PML4 / mov cr3, eax
  cr4.PAE = 1
  EFER.LME = 1
  lgdt [GDT_PTR]
  cr0.PG|PE = 1
  ljmp 0x08:0x00200000               ; 进入长模式 Rust 入口

地址空间 (M4: 显卡 LFB 在 0xFD000000, 需要 3-4GB 段):
  PML4[0] -> PDPT  (4 KiB 页, 64 MiB 低地址恒等: 内核+模块+用户区+栈)
  PML4[3] -> PDPT3 (2 MiB 大页, 0xFC000000..0xFFFFFFFF 恒等: PCI/显卡 LFB)
  x86 每级 U/S 都检查 -> 全链 U=1 (M1 踩坑实录)。
"""

import os
import struct

# ---- 布局常量（与 kernel/kernel.ld 一致） ----
BLOB_BASE = 0x101000
PML4 = 0x102000        # BLOB + 0x1000
PDPT = 0x103000        # BLOB + 0x2000
PD_BASE = 0x104000     # BLOB + 0x3000
PT_BASE = 0x108000     # BLOB + 0x7000 (32 x 512 x 8B = 128 KiB)
PDPT3 = 0x128000       # BLOB + 0x27000
PD3 = 0x129000         # BLOB + 0x28000 (4 项 PT 指针: 0xFD000000 区, 4KiB 页)
PT3_BASE = 0x12A000    # BLOB + 0x29000 (4 x 512 x 8B = 16KiB: 0xFD000000..0xFD800000)
GDT = 0x12C000         # BLOB + 0x2B000
GDT_PTR = 0x12C018     # BLOB + 0x2B018
STACK_TOP = 0x300000
RUST_ENTRY = 0x200000

MAP_PD_END = 0x4000000           # 64 MiB (低地址 4KiB 映射)
BLOB_SIZE = 0x2D040

blob = bytearray(BLOB_SIZE)


def emit(code: bytes, addr: int) -> None:
    o = addr - BLOB_BASE
    assert o + len(code) <= BLOB_SIZE, "emit overflow"
    blob[o:o + len(code)] = code


def set64(addr: int, val: int) -> None:
    o = addr - BLOB_BASE
    assert o + 8 <= BLOB_SIZE, "table overflow"
    blob[o:o + 8] = struct.pack("<Q", val)


# ================= 引导桩 =================
p = BLOB_BASE
emit(b"\xfa", p); p += 1                                        # cli
emit(b"\xbc" + struct.pack("<I", STACK_TOP), p); p += 5         # mov esp, STACK_TOP
emit(b"\x89\xc7", p); p += 2                                    # mov edi, eax (mb magic)
emit(b"\x89\xde", p); p += 2                                    # mov esi, ebx (mbi ptr)
emit(b"\xb8" + struct.pack("<I", PML4), p); p += 5              # mov eax, PML4
emit(b"\x0f\x22\xd8", p); p += 3                                # mov cr3, eax
emit(b"\x0f\x20\xe0", p); p += 3                                # mov eax, cr4
emit(b"\x0f\xba\xe8\x05", p); p += 4                            # bts eax, 5   (PAE)
emit(b"\x0f\x22\xe0", p); p += 3                                # mov cr4, eax
emit(b"\xb9" + struct.pack("<I", 0xC0000080), p); p += 5        # mov ecx, MSR_EFER
emit(b"\x0f\x32", p); p += 2                                    # rdmsr
emit(b"\x0f\xba\xe8\x08", p); p += 4                            # bts eax, 8   (EFER.LME)
emit(b"\x0f\x30", p); p += 2                                    # wrmsr
emit(b"\x0f\x01\x15" + struct.pack("<I", GDT_PTR), p); p += 7   # lgdt [GDT_PTR]
emit(b"\x0f\x20\xc0", p); p += 3                                # mov eax, cr0
emit(b"\x0f\xba\xe8\x1f", p); p += 4                            # bts eax, 31  (CR0.PG)
emit(b"\x0f\xba\xe8\x00", p); p += 4                            # bts eax, 0   (CR0.PE)
emit(b"\x0f\x22\xc0", p); p += 3                                # mov cr0, eax
emit(b"\xea" + struct.pack("<I", RUST_ENTRY) + struct.pack("<H", 0x08), p); p += 7  # ljmp
assert p <= BLOB_BASE + 0x1000, "stub too long"

# ================= 低 64 MiB: 4 KiB 页恒等映射 =================
set64(PML4 + 0 * 8, PDPT | 0x07)
set64(PDPT, PD_BASE | 0x07)
for i in range(32):
    set64(PD_BASE + 8 * i, (PT_BASE + 0x1000 * i) | 0x07)
for i in range(32):
    for j in range(512):
        vaddr = i * 0x200000 + j * 0x1000
        set64(PT_BASE + 0x1000 * i + 8 * j, vaddr | 0x87)

# ================= 显卡 LFB 区: 4KiB 页映射 0xFD000000..0xFD800000 =================
# (M4: 2MiB 大页在本环境未得验证; 4KiB 页是共同可用的安全路径)
set64(PML4 + 3 * 8, PDPT3 | 0x07)
set64(PDPT3, PD3 | 0x07)
for j in range(4):
    set64(PD3 + 8 * (8 + j), (PT3_BASE + 0x1000 * j) | 0x07)  # PD 索引 8..11 -> PT3[0..3]
for j in range(4):
    for k in range(512):
        vaddr = 0xFD000000 + (j * 0x200000 + k * 0x1000)
        set64(PT3_BASE + 0x1000 * j + 8 * k, vaddr | 0x87)

# ================= GDT =================
set64(GDT + 0x00, 0x0000_0000_0000_0000)
set64(GDT + 0x08, 0x00AF_9B00_0000_FFFF)
set64(GDT + 0x10, 0x00CF_9300_0000_FFFF)
o = GDT_PTR - BLOB_BASE
blob[o:o + 6] = struct.pack("<HI", 0x17, GDT)

# ================= 写盘 =================
out = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", "boot_blob.bin"))
with open(out, "wb") as f:
    f.write(blob)
print(f"wrote {out}: {len(blob)} bytes  (stub {BLOB_BASE:#x}+{p - BLOB_BASE:#x}, "
      f"low {MAP_PD_END:#x} 4KiB + 3-4GiB 2MiB identity)")
