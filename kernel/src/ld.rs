//! ld.rs — M72: 系统内链接器 v0 (ELF64 静态最小)
//!
//! 输入 cfg 布局 (用户内存, 全部 u64):
//!   [0] dst (输出缓冲)    [1] text1  [2] n1    [3] text2  [4] n2
//!   [5] syms  [6] nsyms   [7] relocs [8] nrelocs
//!   syms: 条目 [name:32B][vma:8B] × nsyms (绝对 VMA 符号)
//!   relocs: 条目 [place:8B (输出偏移)][symidx:8B] × nrelocs
//! 输出: ELF64 ET_EXEC, 1× PT_LOAD, 段 = text1+text2 (0x400000 起),
//! 重定位写绝对地址 (u64 LE)。
//!
//! 接口: 0x7101 ld_link(cfg) → 输出字节数 (负=err) /
//!       0x7102 ld_info() → 输出字节数。

use crate::serial;

fn rd64(p: u64) -> u64 {
    unsafe { (p as *const u64).read() }
}

/// 0x7101
pub fn fujo_ld_link(cfg: u64) -> i64 {
    let dst = rd64(cfg);
    let t1 = rd64(cfg + 8);
    let n1 = rd64(cfg + 16);
    let t2 = rd64(cfg + 24);
    let n2 = rd64(cfg + 32);
    let syms = rd64(cfg + 40);
    let nsyms = rd64(cfg + 48);
    let relocs = rd64(cfg + 56);
    let nrelocs = rd64(cfg + 64);

    let base: u64 = 0x400000;
    let off1 = 0x8000u64; // header(64) + phdr(56) 后按段对齐
    let off2 = off1 + ((n1 + 15) & !15);
    let total = off2 + n2;

    unsafe {
        // 段数据拷贝
        let mut i = 0usize;
        while (i as u64) < n1 {
            (dst as *mut u8)
                .add(off1 as usize + i)
                .write((t1 as *const u8).add(i).read());
            i += 1;
        }
        i = 0;
        while (i as u64) < n2 {
            (dst as *mut u8)
                .add(off2 as usize + i)
                .write((t2 as *const u8).add(i).read());
            i += 1;
        }

        // 符号表 (vma) 快照
        let mut syms_vma = [0u64; 16];
        let ns = nsyms.min(16);
        for k in 0..ns {
            syms_vma[k as usize] = rd64(syms + k * 40 + 32);
        }

        // 重定位: place (输出偏移) 写符号绝对地址 (base + vma)
        let mut i = 0usize;
        while (i as u64) < nrelocs {
            let place = rd64(relocs + (i as u64) * 16);
            let symidx = rd64(relocs + (i as u64) * 16 + 8);
            if symidx < 16 && symidx < ns {
                let v = base + syms_vma[symidx as usize];
                for k in 0..8 {
                    (dst as *mut u8)
                        .add(place as usize + k)
                        .write((v >> (8 * k)) as u8);
                }
            }
            i += 1;
        }

        let h = dst as *mut u8;
        // e_ident
        h.add(0).write(0x7F);
        h.add(1).write(b'E');
        h.add(2).write(b'L');
        h.add(3).write(b'F');
        h.add(4).write(2); // class 64
        h.add(5).write(1); // LE
        h.add(6).write(1); // version
        h.add(7).write(0);
        for k in 8..16 {
            h.add(k).write(0);
        }
        // e_type = ET_EXEC, e_machine = x86-64, e_version = 1
        h.add(16).write(2);
        h.add(17).write(0);
        h.add(18).write(0x3E);
        h.add(19).write(0);
        h.add(20).write(1);
        h.add(21).write(0);
        h.add(22).write(0);
        h.add(23).write(0);
        // e_entry
        for k in 0..8 {
            h.add(24 + k).write((base >> (8 * k)) as u8);
        }
        // e_phoff = 64
        h.add(32).write(64);
        for k in 1..8 {
            h.add(32 + k).write(0);
        }
        // e_shoff = 0
        for k in 0..8 {
            h.add(40 + k).write(0);
        }
        // e_flags = 0
        for k in 0..4 {
            h.add(48 + k).write(0);
        }
        // e_ehsize=64 e_phentsize=56 e_phnum=1
        h.add(52).write(64);
        h.add(53).write(0);
        h.add(54).write(56);
        h.add(55).write(0);
        h.add(56).write(1);
        h.add(57).write(0);
        for k in 58..64 {
            h.add(k).write(0);
        }
        // phdr @ 0x40 (56B)
        h.add(0x40).write(1); // p_type (u32)
        h.add(0x41).write(0);
        h.add(0x42).write(0);
        h.add(0x43).write(0);
        h.add(0x44).write(7); // p_flags RWX (u32)
        h.add(0x45).write(0);
        h.add(0x46).write(0);
        h.add(0x47).write(0);
        for k in 0..8 {
            h.add(0x48 + k).write(0); // p_offset = 0
        }
        for k in 0..8 {
            h.add(0x50 + k).write((base >> (8 * k)) as u8); // p_vaddr
        }
        for k in 0..8 {
            h.add(0x58 + k).write((base >> (8 * k)) as u8); // p_paddr
        }
        for k in 0..8 {
            h.add(0x60 + k).write((total >> (8 * k)) as u8); // p_filesz
        }
        for k in 0..8 {
            h.add(0x68 + k).write((total >> (8 * k)) as u8); // p_memsz
        }
        h.add(0x70).write(0x00); // p_align = 0x1000
        h.add(0x71).write(0x10);
        for k in 0x72..0x78 {
            h.add(k).write(0);
        }
        LAST_SIZE = total;
    }

    serial::write_str("ld   : linked ");
    crate::syscall::debug_dec(total);
    serial::write_line(" bytes (elf64 static)");

    total as i64
}

/// 0x7102: 信息 (最后一次 link 长度)。
pub fn fujo_ld_info() -> i64 {
    unsafe { LAST_SIZE as i64 }
}

static mut LAST_SIZE: u64 = 0;
