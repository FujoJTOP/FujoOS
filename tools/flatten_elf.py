#! /usr/bin/env python3
"""flatten_elf.py — 把 ELF 内核按 PT_LOAD 段展开为扁平二进制 (qemu -kernel 需要)。

用法: python tools/flatten_elf.py <kernel.elf> <out.bin> [--verbose]
"""
import struct
import sys


def main():
    src = sys.argv[1]
    dst = sys.argv[2]
    verbose = "--verbose" in sys.argv
    elf = open(src, "rb").read()

    if elf[:4] != b"\x7fELF":
        sys.exit("not an ELF")
    ei_class = elf[4]
    ei_data = elf[5]
    if ei_class != 2 or ei_data != 1:
        sys.exit("only 64-bit little-endian ELF supported")

    # ELF64 header
    (e_type, e_machine, _e_version, e_entry, e_phoff, _e_shoff, e_flags,
     _ehsize, e_phentsize, e_phnum, _e_shentsize, _e_shnum, _e_shstrndx) = \
        struct.unpack_from("<HHIQQQIHHHHHH", elf, 16)

    if e_phentsize != 56:
        sys.exit(f"unexpected program header size {e_phentsize}")
    if e_type != 2:
        sys.exit(f"expected ET_EXEC, got {e_type}")

    segs = []
    min_vaddr = None
    max_end = 0
    for i in range(e_phnum):
        (p_type, p_flags, p_offset, p_vaddr, p_paddr, p_filesz, p_memsz, p_align) = \
            struct.unpack_from("<IIQQQQQQ", elf, e_phoff + i * 56)
        if p_type == 1:  # PT_LOAD
            segs.append((p_offset, p_vaddr, p_filesz, p_memsz, p_flags))
            if min_vaddr is None or p_vaddr < min_vaddr:
                min_vaddr = p_vaddr
            if p_vaddr + p_memsz > max_end:
                max_end = p_vaddr + p_memsz

    if not segs:
        sys.exit("no PT_LOAD segments")

    base = min_vaddr
    image = bytearray(max_end - base)
    for (off, vaddr, filesz, memsz, flags) in segs:
        data = elf[off:off + filesz]
        start = vaddr - base
        image[start:start + filesz] = data
        # filesz..memsz 之间由 bss 补零（bytearray 已清零）
        if verbose:
            print(f"  PT_LOAD v={vaddr:#x} f={filesz:#x} m={memsz:#x} flags={flags:#x}")

    open(dst, "wb").write(bytes(image))
    if "--pad" in sys.argv:
        want = int(sys.argv[sys.argv.index("--pad") + 1], 0)
        if len(image) < want:
            image.extend(b"\0" * (want - len(image)))
            open(dst, "wb").write(bytes(image))
            print(f"padded to {want:#x} ({want} bytes)")
    print(f"flattened {src} -> {dst}  ({len(image)} bytes, entry {e_entry:#x}, base {base:#x})")


if __name__ == "__main__":
    main()
