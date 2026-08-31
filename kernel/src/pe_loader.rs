//! pe_loader.rs — 内核侧 PE32+ 装载器 (M3 · winsubsys v0 / M27 mingw CRT)
//!
//! 职责:
//!   1. 解析 MZ/PE 头 + 节表, 把节映射到 ImageBase 对应虚拟地址
//!   2. 解析导入目录, 把每个 IAT 槽位绑定到用户态垫片蹦床 (shim trampoline)
//!      —— Wine 式架构的 kernel 端: import -> shim -> 宏原生 syscall
//!   3. 返回入口 (ImageBase + AddressOfEntryPoint)
//!
//! M27: mingw-w64 CRT (mainCRTStartup) 原生运行 — 需要:
//!   * 垫片家族 (kernel32 23 符号 + msvcrt 32 符号) 全量绑定
//!   * 数据导入 (__initenv/_commode/_fmode) -> 用户态全局 cell 地址
//!   * 用户态数据块 0x7E0000..0x7E3000: FILE[2]/errno/lconv/TEB/argv0
//!   * GS 基址 -> TEB (0x7E1000, mingw 读 gs:[0x30] = Self -> [Self+8]=StackBase)

use crate::serial;
use crate::syscall;

/// 垫片表: (模块小写名, 函数原文, fujo 原生 shim syscall 号)
/// 槽位索引 = 蹦床地址 (SHIM_PAGE + idx*0x20)。
pub const SHIM_TABLE: &[(&str, &str, u64)] = &[
    // ---------- kernel32.dll (0x5001..0x5017) ----------
    ("kernel32.dll", "WriteFile", 0x5001),
    ("kernel32.dll", "ExitProcess", 0x5002),
    ("kernel32.dll", "ReadFile", 0x5003),
    ("kernel32.dll", "GetFileSize", 0x5004),
    ("kernel32.dll", "GetCurrentThreadId", 0x5005),
    ("kernel32.dll", "CloseHandle", 0x5006),
    ("kernel32.dll", "GetModuleHandleA", 0x5007),
    ("kernel32.dll", "GetProcAddress", 0x5008),
    ("kernel32.dll", "LoadLibraryA", 0x5009),
    ("kernel32.dll", "FreeLibrary", 0x500A),
    ("kernel32.dll", "GetLastError", 0x500B),
    ("kernel32.dll", "Sleep", 0x500C),
    ("kernel32.dll", "VirtualProtect", 0x500D),
    ("kernel32.dll", "VirtualQuery", 0x500E),
    ("kernel32.dll", "TlsGetValue", 0x500F),
    ("kernel32.dll", "SetUnhandledExceptionFilter", 0x5010),
    ("kernel32.dll", "EnterCriticalSection", 0x5011),
    ("kernel32.dll", "LeaveCriticalSection", 0x5012),
    ("kernel32.dll", "InitializeCriticalSection", 0x5013),
    ("kernel32.dll", "DeleteCriticalSection", 0x5014),
    ("kernel32.dll", "MultiByteToWideChar", 0x5015),
    ("kernel32.dll", "WideCharToMultiByte", 0x5016),
    ("kernel32.dll", "GetCPInfo", 0x5017),
    // ---------- msvcrt.dll (0x5201..0x5221, M27 mingw CRT) ----------
    ("msvcrt.dll", "__C_specific_handler", 0x5201),
    ("msvcrt.dll", "___lc_codepage_func", 0x5202),
    ("msvcrt.dll", "___mb_cur_max_func", 0x5203),
    ("msvcrt.dll", "__getmainargs", 0x5204),
    ("msvcrt.dll", "__iob_func", 0x5206),
    ("msvcrt.dll", "__set_app_type", 0x5207),
    ("msvcrt.dll", "__setusermatherr", 0x5208),
    ("msvcrt.dll", "_amsg_exit", 0x5209),
    ("msvcrt.dll", "_cexit", 0x520A),
    ("msvcrt.dll", "_errno", 0x520B),
    ("msvcrt.dll", "_initterm", 0x520C),
    ("msvcrt.dll", "_lock", 0x520D),
    ("msvcrt.dll", "_unlock", 0x520E),
    ("msvcrt.dll", "atexit", 0x520F),
    ("msvcrt.dll", "abort", 0x5210),
    ("msvcrt.dll", "calloc", 0x5211),
    ("msvcrt.dll", "exit", 0x5212),
    ("msvcrt.dll", "fflush", 0x5213),
    ("msvcrt.dll", "fprintf", 0x5214),
    ("msvcrt.dll", "fputc", 0x5215),
    ("msvcrt.dll", "free", 0x5216),
    ("msvcrt.dll", "localeconv", 0x5217),
    ("msvcrt.dll", "malloc", 0x5218),
    ("msvcrt.dll", "memcpy", 0x5219),
    ("msvcrt.dll", "puts", 0x521A),
    ("msvcrt.dll", "setvbuf", 0x521B),
    ("msvcrt.dll", "signal", 0x521C),
    ("msvcrt.dll", "strerror", 0x521D),
    ("msvcrt.dll", "strlen", 0x521E),
    ("msvcrt.dll", "strncmp", 0x521F),
    ("msvcrt.dll", "vfprintf", 0x5220),
    ("msvcrt.dll", "wcslen", 0x5221),
];

/// 数据导入表: IAT 槽绑定到用户态全局 cell 地址 (导出是数据对象)。
pub const SHIM_DATA: &[(&str, &str, u64)] = &[
    ("msvcrt.dll", "__initenv", 0x7E0310),
    ("msvcrt.dll", "_commode", 0x7E0318),
    ("msvcrt.dll", "_fmode", 0x7E0320),
];

/// 用户空间蹦床页基址 (0x7F0000..0x800000, U=1, 恒等映射内)
pub const SHIM_PAGE: usize = 0x7F0000;
/// 通用 no-op 蹦床 (xor eax,eax; ret) — GetProcAddress 未知符号 / 垫片兜底
pub const SHIM_HEAP_BASE: u64 = 0x800000;

/// (模块, 函数) -> 垫片槽索引 (大小写不敏感)
pub fn shim_resolve(module: &str, function: &str) -> Option<usize> {
    SHIM_TABLE
        .iter()
        .position(|(m, f, _)| m.eq_ignore_ascii_case(module) && f.eq_ignore_ascii_case(function))
}

/// 函数名 -> 槽索引 (忽略模块; GetProcAddress 用)
pub fn shim_resolve_any(function: &str) -> Option<usize> {
    SHIM_TABLE
        .iter()
        .position(|(_, f, _)| f.eq_ignore_ascii_case(function))
}

/// 数据导入 (模块, 函数) -> cell 地址
pub fn shim_data_resolve(module: &str, function: &str) -> Option<u64> {
    SHIM_DATA
        .iter()
        .find(|(m, f, _)| m.eq_ignore_ascii_case(module) && f.eq_ignore_ascii_case(function))
        .map(|(_, _, a)| *a)
}

pub fn shim_addr(index: usize) -> u64 {
    SHIM_PAGE as u64 + (index as u64) * 0x20
}

pub fn shim_noop_addr() -> u64 {
    SHIM_PAGE as u64 + (SHIM_TABLE.len() as u64) * 0x20
}

/// 把蹦床写入 SHIM_PAGE + 用户态数据块 (只执行一次)。
/// 每个槽 0x20 字节; 最后加一槽通用 no-op (xor eax,eax; ret)。
pub unsafe fn install_shims() {
    let p = SHIM_PAGE as *mut u8;
    // 通用 trampoline (Win64 调用约定): rcx=arg1, rdx=arg2, r8=arg3, r9=arg4
    //   mov rdi, rcx; mov rsi, rdx; mov rdx, r8; mov rcx, r9;
    //   mov rax, <id>; syscall; pop rcx; pop rax; ret
    let stub: [u8; 32] = [
        0x51, // push rcx (1) — 调用方 arg1/返回 RIP 槽 (后续 pop 恢复)
        0x57, // push rdi (1) — Win64 callee-saved! (trampoline 会改 rdi)
        0x56, // push rsi (1) — Win64 callee-saved! (trampoline 会改 rsi)
        0x48, 0x89, 0xCF, // mov rdi, rcx (3)
        0x48, 0x89, 0xD6, // mov rsi, rdx (3)
        0x4C, 0x89, 0xC2, // mov rdx, r8 (3)
        0x4C, 0x89, 0xC9, // mov rcx, r9 (3)
        0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00, // mov rax, imm32 (7) — imm @13..16
        0x0F, 0x05, // syscall (2) — rax = 内核返回值
        0x5E, // pop rsi (1)
        0x5F, // pop rdi (1)
        0x59, // pop rcx (1)
        0xC3, // ret (1)
        0x90, 0x90, 0x90, 0x90, // pad 4
    ];
    for (i, (_, _, nr)) in SHIM_TABLE.iter().enumerate() {
        let off = i * 0x20;
        let mut s = stub;
        s[18] = (nr & 0xFF) as u8;
        s[19] = ((nr >> 8) & 0xFF) as u8;
        s[20] = ((nr >> 16) & 0xFF) as u8;
        s[21] = ((nr >> 24) & 0xFF) as u8;
        core::ptr::copy_nonoverlapping(s.as_ptr(), p.add(off), s.len());
    }
    // 通用 no-op 蹦床: 31 c0 c3 (xor eax,eax; ret) + pad
    let noop = p.add(SHIM_TABLE.len() * 0x20);
    core::ptr::write_bytes(noop, 0x90, 0x20);
    noop.add(0).write(0x31);
    noop.add(1).write(0xC0);
    noop.add(2).write(0xC3);

    // ---- 用户态数据块 0x7E0000.. (M27) ----
    // FILE[2] @0x7E0000 (仅指针号, 本实现从不解引用)
    let db = 0x7E0000u64 as *mut u8;
    core::ptr::write_bytes(db, 0, 0x80);
    // errno cell @0x7E0100
    (0x7E0100u64 as *mut u32).write(0);
    // lconv @0x7E0200: decimal_point="." thousands="" grouping=""
    (0x7E0200u64 as *mut u64).write(0x7E0300u64);
    (0x7E0200u64 as *mut u64).add(1).write(0x7E0302u64);
    (0x7E0200u64 as *mut u64).add(2).write(0x7E0302u64);
    (0x7E0300u64 as *mut u8).write(b'.');
    (0x7E0301u64 as *mut u8).write(0);
    (0x7E0302u64 as *mut u8).write(0);
    // 数据导入 cells
    (0x7E0310u64 as *mut u64).write(0);
    (0x7E0318u64 as *mut u32).write(0);
    (0x7E0320u64 as *mut u32).write(0);
    // strerror 固定串 @0x7E0330
    let msg = b"unknown error";
    for (k, b) in msg.iter().enumerate() {
        (0x7E0330u64 as *mut u8).add(k).write(*b);
    }
    (0x7E0330u64 as *mut u8).add(msg.len()).write(0);
    // argv0 占位 @0x7E0420 (装载时由 syscall 模块填入)
    // TEB @0x7E1000: [0x08]=StackBase(0x600000), [0x30]=Self
    let teb = 0x7E1000u64;
    core::ptr::write_bytes(teb as *mut u8, 0, 0x80);
    (teb as *mut u64).add(1).write(0x600000u64); // +0x08 StackBase
    (teb as *mut u64).add(6).write(teb); // +0x30 Self
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
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

                    if let Some(idx) = shim_resolve(dname_s, fname_s) {
                        let addr = shim_addr(idx);
                        (iat as *mut u64).write(addr);
                        syscall::log_shim(dname_s, fname_s, addr);
                    } else if let Some(cell) = shim_data_resolve(dname_s, fname_s) {
                        (iat as *mut u64).write(cell);
                        serial::write_str("shim : ");
                        serial::write_str(dname_s);
                        serial::write_str("!");
                        serial::write_str(fname_s);
                        serial::write_line(" -> data cell (0x7E0000 block)");
                    } else {
                        return Err("pe: unresolved import");
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
