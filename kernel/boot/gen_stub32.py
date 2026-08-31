#! /usr/bin/env python3
"""gen_stub32.py — 生成 FujoOS 32 位引导桩 + 前 1 GiB 恒等页表 (kernel/boot_blob.bin)

本脚本把下列汇编手写为机器码并输出：
  .stub32  (blob 偏移 0x0000, 物理 0x101000)  可执行代码
  .tables  (blob 偏移 0x1000..0x6000, 物理 0x102000..0x107FFF)  页表数据

流程: 32 位保护模式 (QEMU multiboot v1 入口)
  cli
  mov esp, 0x300000                  ; 内核栈顶 (恒等映射内)
  mov edi, eax                       ; eax = multiboot magic (0x2BADB002)
  mov esi, ebx                       ; ebx = multiboot info 指针
  mov eax, PML4 / mov cr3, eax
  cr4.PAE = 1                        ; bts eax,5
  EFER.LME = 1                       ; rdmsr 0xC0000080 / bts eax,8 / wrmsr
  cr0.PG|PE = 1                      ; 最后开启分页+长模式
  ljmp 0x08:0x00200000               ; 进入长模式 Rust 入口 (rust64_entry)

页表: 0..1 GiB 恒等映射, 2 MiB 大页
  PML4[0] -> PDPT, PDPT[0..3] -> PD0..PD3, PDk[j] = (j*2MiB) | 0x83

常量必须与 kernel/kernel.ld 严格一致（M0 开发环；M1 换 Limine + 高半区）。
"""

import os
import struct

# ---- 布局常量（与 kernel/kernel.ld 一致） ----
BLOB_BASE = 0x101000   # .boot_blob 段基址
PML4 = 0x102000        # BLOB + 0x1000
PDPT = 0x103000        # BLOB + 0x2000
PD_BASE = 0x104000     # BLOB + 0x3000 .. +0x6000 (4 x 512 x 8B)
GDT = 0x107000         # BLOB + 0x6000 (自建 GDT: 0x08=64位码段, 0x10=64位数据段)
GDT_PTR = 0x107018     # BLOB + 0x6018 (lgdt 操作数)
STACK_TOP = 0x300000
RUST_ENTRY = 0x200000
BLOB_SIZE = 0x8000

blob = bytearray(BLOB_SIZE)


def emit(code: bytes, addr: int) -> None:
    """把代码字节写进 blob（用切片赋值, 恰好等长）。"""
    o = addr - BLOB_BASE
    assert o + len(code) <= BLOB_SIZE, "emit overflow"
    blob[o:o + len(code)] = code


def set64(addr: int, val: int) -> None:
    """页表项数据 (8 字节, 小端)。"""
    o = addr - BLOB_BASE
    assert o + 8 <= BLOB_SIZE, "table overflow"
    blob[o:o + 8] = struct.pack("<Q", val)


# ================= 引导桩（顺序发射，全部位于 blob 偏移 0） =================
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

# ================= 页表数据（0..1 GiB 恒等映射） =================
set64(PML4, PDPT | 3)
for i in range(4):
    set64(PDPT + 8 * i, (PD_BASE + 0x1000 * i) | 3)
for k in range(4):
    for j in range(512):
        set64(PD_BASE + 0x1000 * k + 8 * j, 0x83 | (j << 21))   # 2 MiB 大页, RW|PS|P

# ================= GDT 数据（长模式必需） =================
# 0x00: null
# 0x08: 64-bit code  base=0 limit=0xFFFFF G=1 L=1 DPL0 access 0x9B
# 0x10: 64-bit data   base=0 limit=0xFFFFF G=1 DPL0 access 0x93
set64(GDT + 0x00, 0x0000_0000_0000_0000)
set64(GDT + 0x08, 0x00AF_9B00_0000_FFFF)
set64(GDT + 0x10, 0x00CF_9300_0000_FFFF)
# lgdt 操作数: limit(16) = 0x17, base(32) = GDT
o = GDT_PTR - BLOB_BASE
blob[o:o + 6] = struct.pack("<HI", 0x17, GDT)

# ================= 写盘 =================
out = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", "boot_blob.bin"))
with open(out, "wb") as f:
    f.write(blob)
print(f"wrote {out}: {len(blob)} bytes  (stub {BLOB_BASE:#x}+{p - BLOB_BASE:#x}, "
      f"rust entry {RUST_ENTRY:#x}, pml4 {PML4:#x})")
