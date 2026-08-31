#! /usr/bin/env python3
"""gen_stub32.py — 生成 FujoOS 32 位引导桩 + 恒等页表 (kernel/boot_blob.bin)

本脚本把下列汇编手写为机器码并输出 (纯 Python 标准库, 无外部工具):
  .stub32  (blob 偏移 0x0000, 物理 0x101000)  可执行代码
  .tables  (blob 偏移 0x1000.., 物理 0x102000..)  页表数据

流程: 32 位保护模式 (QEMU multiboot v1 入口)
  cli
  mov esp, 0x300000                  ; 内核栈顶 (恒等映射内)
  mov edi, eax                       ; eax = multiboot magic (0x2BADB002)
  mov esi, ebx                       ; ebx = multiboot info 指针
  mov eax, PML4 / mov cr3, eax
  cr4.PAE = 1                        ; bts eax,5
  EFER.LME = 1                       ; rdmsr 0xC0000080 / bts eax,8 / wrmsr
  lgdt [GDT_PTR]                     ; 自建 GDT: 0x08=64位码段, 0x10=64位数据段
  cr0.PG|PE = 1                      ; 最后开启分页+长模式
  ljmp 0x08:0x00200000               ; 进入长模式 Rust 入口 (rust64_entry)

页表 (4 KiB 页, 16 MiB 恒等映射 — M1 用 4KB 页, 与主流内核一致):
  PML4[0] -> PDPT
  PDPT[0] -> PD
  PD[0]   -> PT0..PT7  (16 MiB / 2 MiB = 8 个页表)
  PTi[j]  = (i*2MiB + j*4KiB) | flags
  用户区 0x400000..0x900000: flags=0x87 (P|RW|U|...)
  其余:                      flags=0x83 (P|RW)

说明: 不再使用 2MiB 大页 (PS=1) —— 在 QEMU TCG 上出现不可解释的
     注入模式保护故障; 4KB 页是最稳的公共路径。常量与 kernel.ld 一致。
"""

import os
import struct

# ---- 布局常量（与 kernel/kernel.ld 一致） ----
BLOB_BASE = 0x101000   # .boot_blob 段基址
PML4 = 0x102000        # BLOB + 0x1000
PDPT = 0x103000        # BLOB + 0x2000
PD_BASE = 0x104000     # BLOB + 0x3000
PT_BASE = 0x108000     # BLOB + 0x7000 (8 x 512 x 8B = 32 KiB)
GDT = 0x110000         # BLOB + 0xF000 (位于 8 个页表之后)
GDT_PTR = 0x110018     # BLOB + 0xF018
STACK_TOP = 0x300000
RUST_ENTRY = 0x200000

# 16 MiB 恒等映射, 用户区 4..9 MiB
MAP_PD_END = 0x1000000          # 16 MiB
USER_LO = 0x400000
USER_HI = 0x900000
BLOB_SIZE = 0xF030             # 覆盖 stub+所有表+GDT

blob = bytearray(BLOB_SIZE)


def emit(code: bytes, addr: int) -> None:
    o = addr - BLOB_BASE
    assert o + len(code) <= BLOB_SIZE, "emit overflow"
    blob[o:o + len(code)] = code


def set64(addr: int, val: int) -> None:
    o = addr - BLOB_BASE
    assert o + 8 <= BLOB_SIZE, "table overflow"
    blob[o:o + 8] = struct.pack("<Q", val)


# ================= 引导桩（顺序发射, blob 偏移 0） =================
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
emit(b"\xea" + struct.pack("<I", RUST_ENTRY) + struct.pack("<H", 0x08), p); p += 7  # ljmp 0x08:0x200000
assert p <= BLOB_BASE + 0x1000, "stub too long"

# ================= 页表数据（4 KiB 页, 16 MiB 恒等映射） =================
# 注意: x86 页表遍历在**每一级**都检查 U/S 位 —— 用户访问要求
# PML4/PDPT/PD/PTE 全链 U=1, 只设叶级会 #PF (M1 踩坑实录; e=5 U 违规)。
set64(PML4, PDPT | 0x07)          # PML4[0]: P|RW|U
set64(PDPT, PD_BASE | 0x07)       # PDPT[0]: P|RW|U
# PD[0..7] = PT 指针, 任意用途均为用户可见 (16MiB 恒等区)
for i in range(8):
    set64(PD_BASE + 8 * i, (PT_BASE + 0x1000 * i) | 0x07)
# 页表内容
for i in range(8):
    for j in range(512):
        vaddr = i * 0x200000 + j * 0x1000
        flags = 0x83                               # P|RW
        if USER_LO <= vaddr < USER_HI:
            flags |= 0x04                          # U: 用户区 (M1)
        set64(PT_BASE + 0x1000 * i + 8 * j, vaddr | flags)  # 恒等映射: 物理 = 虚拟

# ================= GDT 数据 =================
# 0x00: null / 0x08: 64-bit code / 0x10: 64-bit data
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
      f"rust entry {RUST_ENTRY:#x}, 4KiB pages 0..{MAP_PD_END:#x})")
