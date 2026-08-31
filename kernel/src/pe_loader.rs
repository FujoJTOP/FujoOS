//! pe_loader.rs — 内核侧 PE32+ 装载器 (M3 · winsubsys v0)
//!
//! 职责:
//!   1. 解析 MZ/PE 头 + 节表, 把节映射到 ImageBase 对应虚拟地址
//!   2. 解析导入目录, 把每个 IAT 槽位绑定到用户态垫片蹦床 (shim trampoline)
//!      —— Wine 式架构的 kernel 端: import -> shim -> 宏原生 syscall
//!   3. 返回入口 (ImageBase + AddressOfEntryPoint)
//!
//! 范围: PE32+ (x64 console), 固定基址 /base:0x400000, 无重定位表需求。
//! 后续: 基址重定位(DYNAMIC_BASE)、资源、TLS (M3.5)。

use crate::serial;
use crate::syscall;

/// 垫片蹦床并查表: (module, function) -> fujo 原生 syscall 号
fn shim_syscall_nr(module: &str, function: &str) -> Option<u64> {
    match (module, function) {
        ("KERNEL32.dll", "WriteFile") | ("kernel32.dll", "WriteFile") => Some(0x5001),
        ("KERNEL32.dll", "ExitProcess") | ("kernel32.dll", "ExitProcess") => Some(0x5002),
        _ => None,
    }
}

/// 用户空间蹦床页基址 (0x7F0000..0x800000, 64MiB 恒等映射内, U=1)
pub const SHIM_PAGE: usize = 0x7F0000;

/// 把蹦床写入 SHIM_PAGE (只执行一次; ID 表: 0=WriteFile 1=ExitProcess)
pub unsafe fn install_shims() {
    let p = SHIM_PAGE as *mut u8;
    // WriteFile(hFile=rcx, buf=rdx, len=r8):
    //   mov rdi, rcx; mov rsi, rdx; mov rdx, r8;
    //   mov rax, 0x5001; syscall; ret
    let wf: [u8; 26] = [
        0x48, 0x89, 0xCF,                   // mov rdi, rcx
        0x48, 0x89, 0xD6,                   // mov rsi, rdx
        0x4C, 0x89, 0xC2,                   // mov rdx, r8
        0x48, 0xC7, 0xC0, 0x01, 0x50, 0x00, 0x00, // mov rax, 0x5001
        0x0F, 0x05,                         // syscall
        0xC3,                               // ret
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, // padding
    ];
    core::ptr::copy_nonoverlapping(wf.as_ptr(), p, 26);

    // ExitProcess(code=rcx):
    //   mov rdi, rcx; mov rax, 0x5002; syscall; ret
    let ep: [u8; 33] = [
        0x48, 0x89, 0xCF,                   // mov rdi, rcx
        0x48, 0xC7, 0xC0, 0x02, 0x50, 0x00, 0x00, // mov rax, 0x5002
        0x0F, 0x05,                         // syscall
        0xC3,                               // ret
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
        0x90, 0x90, 0x90, 0x90,
    ];
    core::ptr::copy_nonoverlapping(ep.as_ptr(), p.add(0x20), 28);
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}

fn shim_addr(id: u64) -> u64 {
    SHIM_PAGE as u64 + id * 0x20
}

/// 装载 PE 模块 (base 起, len 长), 返回入口。
pub fn load_pe(base: u32, len: u32) -> Result<u64, &'static str> {
    let buf = base as *const u8;
    unsafe {
        if (buf as *const u16).read() != 0x5A4D {
            return Err("pe: not MZ");
        }
        let pe_off = (buf.add(0x3C) as *const u32).read() as usize;
        if pe_off + 4 > len as usize {
            return Err("pe: truncated");
        }
        if (buf.add(pe_off) as *const u32).read() != 0x0000_4550 {
            return Err("pe: no PE signature");
        }
        let num_sections = (buf.add(pe_off + 6) as *const u16).read() as usize;
        let opt_size = (buf.add(pe_off + 20) as *const u16).read() as usize;
        let opt = pe_off + 24;
        let magic = (buf.add(opt) as *const u16).read();
        if magic != 0x20B {
            return Err("pe: not PE32+");
        }
        let image_base = (buf.add(opt + 24) as *const u64).read();
        let entry_rva = (buf.add(opt + 16) as *const u32).read();
        let import_dir_rva = (buf.add(opt + 112 + 8) as *const u32).read();
        let import_dir_size = (buf.add(opt + 112 + 12) as *const u32).read();

        let sec_tab = opt + opt_size;
        // ---- 节映射 ----
        for i in 0..num_sections {
            let sec = buf.add(sec_tab + i * 40);
            let vaddr = (sec.add(12) as *const u32).read() as usize;
            let raw_size = (sec.add(16) as *const u32).read() as usize;
            let raw_ptr = (sec.add(20) as *const u32).read() as usize;
            if raw_ptr + raw_size > len as usize {
                return Err("pe: section overruns file");
            }
            core::ptr::copy_nonoverlapping(
                buf.add(raw_ptr),
                (image_base as usize + vaddr) as *mut u8,
                raw_size,
            );
        }

        // ---- 导入绑定 ----
        if import_dir_rva != 0 && import_dir_size != 0 {
            let mut desc = image_base as usize + import_dir_rva as usize;
            let mut limit = 0usize;
            while limit < 64 {
                let orig_thunk = (desc as *const u32).read();
                // IMAGE_IMPORT_DESCRIPTOR: [OT@0][TS@4][FwdChain@8][NameRVA@12][FirstThunk@16]
                let name_rva = ((desc + 12) as *const u32).read();
                let first_thunk = ((desc + 16) as *const u32).read();
                if orig_thunk == 0 && name_rva == 0 {
                    break;
                }
                // DLL 名
                let dll = image_base as usize + name_rva as usize;
                let mut dname = [0u8; 48];
                let mut dn = 0usize;
                while dn < 47 {
                    let b = (dll as *const u8).add(dn).read();
                    if b == 0 {
                        break;
                    }
                    dname[dn] = b;
                    dn += 1;
                }
                let dname_s = core::str::from_utf8(&dname[..dn]).unwrap_or("?");

                let mut thunk_rva = if orig_thunk != 0 { orig_thunk } else { first_thunk };
                let mut iat = image_base as usize + first_thunk as usize;
                let mut nimp = 0usize;
                while nimp < 256 {
                    let val = ((image_base as usize + thunk_rva as usize) as *const u64)
                        .read();
                    if val == 0 {
                        break;
                    }
                    let name_off = (val as u32) as usize; // hint(16) + name
                    let fname = image_base as usize + name_off + 2;
                    let mut fname_b = [0u8; 64];
                    let mut fn_n = 0usize;
                    while fn_n < 63 {
                        let b = (fname as *const u8).add(fn_n).read();
                        if b == 0 {
                            break;
                        }
                        fname_b[fn_n] = b;
                        fn_n += 1;
                    }
                    let fname_s = core::str::from_utf8(&fname_b[..fn_n]).unwrap_or("?");

                    match shim_syscall_nr(dname_s, fname_s) {
                        Some(nr) => {
                            let id = if nr == 0x5001 { 0 } else { 1 };
                            let addr = shim_addr(id);
                            (iat as *mut u64).write(addr);
                            syscall::log_shim(dname_s, fname_s, addr);
                        }
                        None => {
                            return Err("pe: unresolved import");
                        }
                    }
                    thunk_rva += 8;
                    iat += 8;
                    nimp += 1;
                }
                desc += 20;
                limit += 1;
            }
        }

        Ok(image_base + entry_rva as u64)
    }
}
