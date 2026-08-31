#! /usr/bin/env python3
"""make_fixtures.py — 生成结构性完整的 PE / ELF / Mach-O 测试样本（纯标准库）。

样本用途: fujopack 识别 -> .run 打包 -> fujorun 校验 的端到端回归。
每个样本都带真实文件头、正确的入口、段/程序头信息，可被任一标准解析器读取。
输出: sdk/fixtures/out/{sample-x64.elf, sample-x64-pe.exe, sample-x64-macho}
"""

import os
import struct

OUT = os.path.join(os.path.dirname(__file__), "out")


def w(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)
    print(f"  wrote {os.path.basename(path)}  ({len(data)} bytes)")


def gen_elf():
    # ELF64 LE, ET_EXEC, x86_64, 单个 PT_LOAD
    code = bytes([0xB8, 1, 0, 0, 0, 0xBF, 0, 0, 0, 0, 0xC3])  # mov eax,1; mov edi,0; ret
    ehsize, phentsize, phnum = 64, 56, 1
    phoff = ehsize
    code_off = ehsize + phentsize
    p_vaddr = 0x401000
    ehdr = struct.pack(
        "<16sHHIQQQIHHHHHH",
        b"\x7fELF" + bytes([2, 1, 1, 0]) + bytes(8),
        2, 0x3E, 1, p_vaddr, phoff, 0, 0,
        ehsize, phentsize, phnum, 0, 0, 0,
    )
    phdr = struct.pack(
        "<IIQQQQQQ",
        1, 7, code_off, p_vaddr, p_vaddr, len(code), len(code), 0x1000
    )
    w(os.path.join(OUT, "sample-x64.elf"), ehdr + phdr + code)


def gen_pe():
    # PE32+ LE, machine 0x8664, 一个 .text 段
    dos = bytearray(0x80)
    dos[0:2] = b"MZ"
    struct.pack_into("<I", dos, 0x3C, 0x80)

    coff_off = 0x80
    opt_off = coff_off + 24
    opt_size = 0xF0
    sec_off = opt_off + opt_size
    raw_off = sec_off + 40

    coff = struct.pack("<HHIIIHH", 0x8664, 1, 0x65000000, 0, 0, opt_size, 0x0022)

    opt = bytearray(opt_size)
    struct.pack_into("<H", opt, 0, 0x20B)            # Magic PE32+
    struct.pack_into("<B", opt, 2, 14)               # MajorLinkerVersion
    struct.pack_into("<B", opt, 3, 0)
    struct.pack_into("<I", opt, 4, 0x200)            # SizeOfCode
    struct.pack_into("<I", opt, 16, 0x1000)          # AddressOfEntryPoint (RVA)
    struct.pack_into("<I", opt, 20, 0x1000)          # BaseOfCode
    struct.pack_into("<Q", opt, 24, 0x140000000)     # ImageBase
    struct.pack_into("<I", opt, 32, 0x1000)          # SectionAlignment
    struct.pack_into("<I", opt, 36, 0x200)           # FileAlignment
    struct.pack_into("<H", opt, 40, 6)               # MajorOSVersion
    struct.pack_into("<H", opt, 48, 6)               # MajorSubsystemVersion
    struct.pack_into("<I", opt, 56, 0x2000)          # SizeOfImage
    struct.pack_into("<I", opt, 60, 0x200)           # SizeOfHeaders
    struct.pack_into("<H", opt, 68, 3)               # Subsystem = console
    struct.pack_into("<H", opt, 70, 0x8140)          # DllCharacteristics
    struct.pack_into("<Q", opt, 72, 0x100000)        # SizeOfStackReserve
    struct.pack_into("<Q", opt, 80, 0x1000)          # SizeOfStackCommit
    struct.pack_into("<Q", opt, 88, 0x100000)        # SizeOfHeapReserve
    struct.pack_into("<Q", opt, 96, 0x1000)          # SizeOfHeapCommit
    struct.pack_into("<I", opt, 108, 16)             # NumberOfRvaAndSizes

    sec = bytearray(40)
    sec[0:5] = b".text"
    struct.pack_into("<IIIIIIHHI", sec, 8,
                     0x200, 0x1000, 0x200, raw_off, 0, 0, 0, 0, 0x60000020)

    raw = bytearray(0x200)
    raw[0:8] = bytes([0x48, 0x31, 0xC0, 0xC3, 0x90, 0x90, 0x90, 0x90])  # xor rax,rax; ret

    w(os.path.join(OUT, "sample-x64-pe.exe"),
      bytes(dos) + b"PE\x00\x00" + coff + bytes(opt) + bytes(sec) + bytes(raw))


def gen_macho():
    # Mach-O 64 LE, MH_EXECUTE, x86_64, 单个 LC_MAIN
    hdr = struct.pack("<IIIIIIII", 0xFEEDFACF, 0x01000007, 3, 2, 1, 24, 0, 0)
    lc = struct.pack("<IIQQ", 0x80000028, 24, 0x1000, 0x1000000)  # LC_MAIN: entryoff
    total = 0x1100
    data = bytearray(total)
    data[0:32] = hdr
    data[32:56] = lc
    data[0x1000:0x1006] = bytes([0x55, 0x48, 0x89, 0xE5, 0xC3, 0xC3])  # push rbp; mov rbp,rsp; ret
    w(os.path.join(OUT, "sample-x64-macho"), bytes(data))


def main():
    gen_elf()
    gen_pe()
    gen_macho()
    print("fixtures ready in", OUT)


if __name__ == "__main__":
    main()
