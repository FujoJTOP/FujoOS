//! macho_loader.rs — 内核侧 Mach-O 64 装载器 (M6 · darwinsubsys v0)
//!
//! 支持: MH_MAGIC_64 (LE), LC_SEGMENT_64 (段映射 + BSS 清零),
//!       LC_MAIN / LC_UNIXTHREAD (入口), 运行时重定位。
//! 重定位: macOS 原生 __TEXT 基址 0x100000000; 本内核把装载基址统一为
//! 用户低区 0x400000 (已验证路径, 与 ELF/PE 一致) —— 段复制与入口指针
//! 同步平移。后续 (M6.5) 支持任意基址/ASLR。

use crate::syscall;

const MACHO_BASE: u64 = 0x100000000; // macOS 原生基址
const RUNTIME_BASE: u64 = 0x400000;  // FujoOS 用户装载基址

fn rloc(v: u64) -> u64 {
    RUNTIME_BASE + (v - MACHO_BASE)
}

/// 装载 Mach-O (base 起, len 长), 返回 (重定位后) 入口。
pub fn load_macho(base: u32, len: u32) -> Result<u64, &'static str> {
    let buf = base as *const u8;
    unsafe {
        if len < 0x20 {
            return Err("macho: too small");
        }
        let magic = (buf as *const u32).read();
        if magic != 0xFEED_FACF {
            return Err("macho: not MH_MAGIC_64 LE");
        }
        let ncmds = (buf.add(0x10) as *const u32).read() as usize;
        let mut off = 0x20usize;
        let mut text_vmaddr: u64 = 0;
        let mut text_fileoff: u64 = 0;
        let mut entry_off: u64 = 0;
        let mut entry_va: Option<u64> = None;

        for _ in 0..ncmds {
            let cur = off;
            if cur + 8 > len as usize {
                return Err("macho: load command overrun");
            }
            let cmd = (buf.add(cur) as *const u32).read();
            let csize = (buf.add(cur + 4) as *const u32).read() as usize;
            if csize < 8 || cur + csize > len as usize {
                return Err("macho: bad load command size");
            }
            off = cur + csize; // 统一 advance (M6: continue 不能跳过)

            match cmd {
                // LC_SEGMENT_64
                0x19 => {
                    let vmaddr = (buf.add(cur + 24) as *const u64).read() as usize;
                    let vmsize = (buf.add(cur + 32) as *const u64).read() as usize;
                    let fileoff = (buf.add(cur + 40) as *const u64).read() as usize;
                    let filesize = (buf.add(cur + 48) as *const u64).read() as usize;
                    let segname = &*(buf.add(cur + 8) as *const [u8; 16]);
                    // __TEXT 前 4 字节为 "__TE" (M6 踩坑实录: 写成 "__TX" 永不命中!)
                    if segname[0..4] == *b"__TE" {
                        text_vmaddr = vmaddr as u64;
                        text_fileoff = fileoff as u64;
                    }
                    if fileoff + filesize > len as usize {
                        return Err("macho: segment overruns file");
                    }
                    if filesize > 0 {
                        let dst = rloc(vmaddr as u64) as *mut u8;
                        core::ptr::copy_nonoverlapping(buf.add(fileoff), dst, filesize);
                        if vmsize > filesize {
                            for k in filesize..vmsize {
                                dst.add(k).write(0u8);
                            }
                        }
                    }
                }
                // LC_MAIN
                0x8000_0028 => {
                    let eoff = (buf.add(cur + 8) as *const u64).read();
                    entry_off = eoff;
                }
                // LC_UNIXTHREAD
                0x5 => {
                    let flavor = (buf.add(cur + 8) as *const u32).read();
                    if flavor == 4 || flavor == 5 {
                        let rip = (buf.add(cur + 16 + 16 * 8) as *const u64).read();
                        entry_va = Some(rip);
                    }
                }
                _ => {}
            }
        }

        if let Some(va) = entry_va {
            return Ok(rloc(va));
        }
        if entry_off == 0 {
            return Err("macho: no LC_MAIN/LC_UNIXTHREAD");
        }
        if text_vmaddr == 0 {
            return Err("macho: no __TEXT segment");
        }
        let entry = text_vmaddr + (entry_off - text_fileoff);
        Ok(rloc(entry))
    }
}
