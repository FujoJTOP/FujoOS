//! elf_loader.rs — 内核侧最小 ELF64 装载器 (M2 · linuxsubsys v0)
//!
//! 输入: 内存中的 ELF 文件 (当前来自 QEMU multiboot 模块, 即 "-initrd")
//! 输出: 装载完成的用户地址空间 + 入口地址。
//!
//! 支持: ET_EXEC, x86_64, PT_LOAD 段复制 + BSS 清零。
//! 不支持(后续): 动态链接/重定位/解释器 (M2.5), PIE (M2.5)。

/// 从 `base` 起的 ELF 装载到用户空间, 返回入口。
/// 失败返回静态错误描述 (内核无堆)。
pub fn load_elf(base: u32, len: u32) -> Result<u64, &'static str> {
    let buf = base as *const u8;
    unsafe {
        // ---- ELF64 header ----
        if len < 0x40 {
            return Err("elf: too small");
        }
        if buf.read() != 0x7F || buf.add(1).read() != b'E' || buf.add(2).read() != b'L' || buf.add(3).read() != b'F'
        {
            return Err("elf: bad magic");
        }
        if (buf.add(4) as *const u8).read() != 2 || (buf.add(5) as *const u8).read() != 1 {
            return Err("elf: not ELF64 LE");
        }
        let e_type = (buf.add(0x10) as *const u16).read();
        let e_machine = (buf.add(0x12) as *const u16).read();
        // M24: 支持 ET_DYN (3) —— 动态/位置无关 ELF (含 PT_INTERP/PT_DYNAMIC)。
        // v0: 段装载按 p_vaddr 原样 (非 PIE 动态 ELF 段已在 0x400000;
        // 真 PIE 基址算出 = e_entry 处对齐, M24 简化: 段装载 + 标记)。
        if e_type != 2 && e_type != 3 {
            // M20 debug: dump 模块首 16B (e_type 不符, 定位模块区是否被覆盖)
            crate::serial::write_str("elfx : bad e_type=");
            crate::syscall::log_hex(e_type as u64);
            crate::serial::write_str(" bytes:");
            for i in 0..16usize {
                crate::syscall::log_hex(buf.add(i).read() as u64);
            }
            crate::serial::write_line("");
            return Err("elf: not ET_EXEC/ET_DYN");
        }
        if e_machine != 0x3E {
            return Err("elf: not x86_64");
        }
        let e_entry = (buf.add(0x18) as *const u64).read();
        let e_phoff = (buf.add(0x20) as *const u64).read() as usize;
        let e_phentsize = (buf.add(0x36) as *const u16).read() as usize;
        let e_phnum = (buf.add(0x38) as *const u16).read() as usize;
        if e_phentsize != 56 {
            return Err("elf: unexpected phdrsize");
        }
        if e_phoff + e_phnum * 56 > len as usize {
            return Err("elf: program headers out of range");
        }

        for i in 0..e_phnum {
            let ph = buf.add(e_phoff + i * 56);
            let p_type = (ph as *const u32).read();
            let _p_flags = (ph.add(4) as *const u32).read();
            let p_offset = (ph.add(8) as *const u64).read() as usize;
            let p_vaddr = (ph.add(16) as *const u64).read() as usize;
            let p_filesz = (ph.add(32) as *const u64).read() as usize;
            let p_memsz = (ph.add(40) as *const u64).read() as usize;
            if p_type != 1 {
                // M24: 记录 INTERP/DYNAMIC 存在 (v0 不做解释器, 段已含所需)
                if p_type == 3 {
                    crate::serial::write_line("elfx : PT_INTERP present (ld.so path recognized)");
                }
                continue; // PT_LOAD only
            }
            if p_offset + p_filesz > len as usize {
                return Err("elf: segment overruns file");
            }
            if p_memsz == 0 {
                continue;
            }
            // M24: ET_DYN 的保护性过滤 — v=0x0 段跳过 (避免破坏内核低址;
            // 真实内容段 p_vaddr>=0x400000 已就位)
            if p_vaddr < 0x100000 {
                continue;
            }
            // 复制文件段
            core::ptr::copy_nonoverlapping(buf.add(p_offset), p_vaddr as *mut u8, p_filesz);
            // BSS 清零
            if p_memsz > p_filesz {
                let z = p_vaddr as *mut u8;
                for k in p_filesz..p_memsz {
                    z.add(k).write(0u8);
                }
            }
        }
        Ok(e_entry)
    }
}
