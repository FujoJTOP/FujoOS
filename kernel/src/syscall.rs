//! syscall.rs — Linux ABI syscall gate (M1 内核化)
//!
//! 路径: 用户 syscall -> LSTAR(0xC0000082) -> fujo_syscall_entry(asm)
//!       -> 栈切换(内核栈 0x300000) -> fujo_syscall_dispatch(C)
//!       -> sysretq (STAR 数学: CS=0x20 SS=0x18 与 GDT 一致)
//!
//! 本版本直接实现 linux x86_64 编号: write(1) → 串口/VGA, exit(60)/exit_group(231)。
//! 这就是 "Linux ABI 第一公民" 的最短路径: ELF 里的 syscall 无需任何用户态垫片。

use core::arch::asm;

use crate::interrupts;
use crate::serial;
use crate::vga;

// ---------- 占位表数据（完整表由 tools 生成, 见 fujo-compat::abi） ----------

pub const LINUX_X64_SUBSET: &[(u16, &str)] = &[
    (0, "read"), (1, "write"), (2, "open"), (3, "close"), (4, "stat"), (5, "fstat"),
    (6, "lstat"), (7, "poll"), (8, "lseek"), (9, "mmap"), (10, "mprotect"), (11, "munmap"),
    (12, "brk"), (16, "ioctl"), (17, "pread64"), (19, "readv"), (20, "writev"), (21, "access"),
    (22, "pipe"), (23, "select"), (24, "sched_yield"), (35, "nanosleep"), (41, "socket"),
    (42, "connect"), (43, "accept"), (57, "fork"), (59, "execve"), (60, "exit"), (61, "wait4"),
    (63, "uname"), (72, "fcntl"), (78, "gettimeofday"), (79, "getcwd"), (157, "prctl"),
    (158, "arch_prctl"), (231, "exit_group"), (257, "openat"), (317, "getrandom"),
    (318, "memfd_create"),
];

pub const DARWIN_X64_SUBSET: &[(u64, &str)] = &[
    (0x200_0001, "exit"), (0x200_0003, "read"), (0x200_0004, "write"), (0x200_0005, "open"),
    (0x200_0006, "close"), (0x200_0013, "lseek"), (0x200_0014, "getpid"), (0x200_00C5, "mmap"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abi {
    LinuxX64,
    DarwinX64,
}

pub fn linux_x64_count() -> usize {
    LINUX_X64_SUBSET.len()
}

pub fn darwin_x64_count() -> usize {
    DARWIN_X64_SUBSET.len()
}

// ---------- 状态 ----------

#[no_mangle]
pub static mut user_rsp_tmp: u64 = 0;
#[no_mangle]
pub static mut sys_kernel_rsp: u64 = 0x300000;
#[no_mangle]
pub static mut spam_count: u64 = 0;

extern "C" {
    fn fujo_syscall_entry();
    fn fujo_enter_user(entry: u64, rsp: u64);
}

core::arch::global_asm!(r#"
    .text
    # ---- syscall 入口 (LSTAR) ----
    # M10 修复 (根因): 入口只恢复 rcx/r11 会破坏用户的 rdi/rsi/rdx/r8/r9/r10 ——
    # C 分发是 caller-saved 契约, 用户编译器认为这些寄存器跨 syscall 存活
    # (clang 会把跨调用基址放 r9 等), 实际被内核吃光, 造成 M9 的 "intent=3 /
    # context[1883]" 漂移与 M10 的 cr2=-3 #PF (r9 残留 0 -> slot-3 地址)。
    # 因此: 保存全部通用寄存器并在返回前原样恢复; rcx/r11 例外处理 (sysretq 需用)。
    .p2align 4
    .global fujo_syscall_entry
fujo_syscall_entry:
    mov [rip + user_rsp_tmp], rsp
    mov rsp, [rip + sys_kernel_rsp]
    push r11
    push rcx
    push r9
    push r8
    push r10
    push rdx
    push rsi
    push rdi
    mov rdi, rax
    mov rsi, rsp
    mov rdx, rcx
    call fujo_syscall_dispatch
    pop rdi
    pop rsi
    pop rdx
    pop r10
    pop r8
    pop r9
    pop rcx
    pop r11
    mov rsp, [rip + user_rsp_tmp]
    sysretq

    # ---- iretq 进入用户态 ----
    # rdi=entry, rsi=user_stack; 先 cli: 构造帧期间不允许中断 (M1 现场验证)
    .p2align 4
    .global fujo_enter_user
fujo_enter_user:
    cli
    mov rax, cr3
    mov cr3, rax
    mov r10, 60
    push 0x1b
    push rsi
    push 0x202
    push 0x23
    push rdi
    mov rax, r10
    iretq
"#);

unsafe fn wrmsr(msr: u32, val: u64) {
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") val as u32,
        in("edx") (val >> 32) as u32,
        options(nomem, nostack, preserves_flags)
    );
}

unsafe fn rdmsr(msr: u32) -> u64 {
    let hi: u32;
    let lo: u32;
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags)
    );
    ((hi as u64) << 32) | lo as u64
}

const MSR_EFER: u32 = 0xC000_0080;
const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_SFMASK: u32 = 0xC000_0084;

/// 启用 syscall/sysret (EFER.SCE + STAR + LSTAR + SFMASK)
pub fn setup() {
    unsafe {
        let efer = rdmsr(MSR_EFER);
        wrmsr(MSR_EFER, efer | 0x1); // SCE

        // STAR:  kcs=0x08 @[47:32], user field=0x10 @[63:48]
        // sysret: CS=0x10+16=0x20, SS=0x10+8=0x18
        let star = (0x08u64 << 32) | (0x10u64 << 48);
        wrmsr(MSR_STAR, star);

        let lst = fujo_syscall_entry as usize as u64;
        wrmsr(MSR_LSTAR, lst);

        wrmsr(MSR_SFMASK, 0x200); // syscall 时屏蔽 IF (简单: 内核期无中断)
    }
}

pub fn lstar() -> u64 {
    unsafe { rdmsr(MSR_LSTAR) }
}

// ---------- 分发 ----------

/// linux-x64 syscall 分发 (由 asm 以 C ABI 调用; rdx = 用户返回 RIP)
#[no_mangle]
pub extern "C" fn fujo_syscall_dispatch(nr: u64, args: *const u64, ret: u64) -> i64 {
    let a0 = unsafe { args.read() };
    let a1 = unsafe { args.add(1).read() };
    let a2 = unsafe { args.add(2).read() };
    let a3 = unsafe { args.add(3).read() };
    let _a4 = unsafe { args.add(4).read() };
    let _a5 = unsafe { args.add(5).read() };

    let res = match nr {
        // write(fd, buf, len)
        1 => user_write(a0, a1, a2),
        // getpid() (x86-64: 39) — linuxsubsys v0 最小实现
        39 => 1,
        // ---- fujo 原生 Win32 shim 通道 (M3) ----
        // kernel32!WriteFile (fd, buf, len)
        0x5001 => user_write(a0, a1, a2),
        // kernel32!ExitProcess (code)
        0x5002 => {
            serial::write_line("user : ExitProcess(0) — 内核接管, M3 验证完成");
            serial::write_str("timer : pit ticks=");
            print_dec(interrupts::ticks());
            serial::write_line(" (~100 Hz since boot)");
            halt_forever();
        }
        // exit(code) / exit_group(code) -> 内核接管并停机
        60 | 231 => {
            serial::write_line("user : sys_exit() — 内核接管, M6 验证完成");
            serial::write_str("timer : pit ticks=");
            print_dec(interrupts::ticks());
            serial::write_line(" (~100 Hz since boot)");
            halt_forever();
        }
        // ---- M9/M10: fujonn 模型调用原语 (fujoos-ai-dev) ----
        // fujo_ai_classify(ptr, len) -> intent (engine=qwen COM2 链路 / 规则降级)
        0x5101 => crate::ai::fujo_ai_classify(a0, a1),
        // fujo_ai_fetch(ptr, len) -> n (fujoctx 上下文注入)
        0x5102 => crate::ai::fujo_ai_fetch(a0, a1),
        // fujo_read_kbd() -> char | 0 (M10 · Hermes CLI 交互输入)
        0x5103 => crate::kbd::try_poll().map(|c| c as i64).unwrap_or(0),
        // fujo_ai_info(ptr, len) -> n (引擎/模型/链路信息)
        0x5104 => crate::ai::fujo_ai_info(a0, a1),
        // ---- darwin BSD 空间 (0x2000000|nr, M6 darwinsubsys) ----
        0x200_0001 => {
            serial::write_line("user : darwin exit() — 内核接管, M6 验证完成");
            serial::write_str("timer : pit ticks=");
            print_dec(interrupts::ticks());
            serial::write_line(" (~100 Hz since boot)");
            halt_forever();
        }
        0x200_0004 => user_write(a0, a1, a2), // darwin write(fd, buf, len)
        // darwin getpid (BSD: 0x2000014)
        0x200_0014 => 2,
        _ => {
            // 未实现: 打印一次(带计数), 返回 -ENOSYS
            let c = unsafe {
                let p = core::ptr::addr_of_mut!(spam_count);
                p.write_volatile(p.read_volatile() + 1);
                p.read_volatile()
            };
            if c <= 3 {
                serial::write_str("syscall unimplemented nr=");
                print_dec(nr);
                serial::write_str(" (");
                serial::write_str(name_of(nr).unwrap_or("?"));
                serial::write_line(")");
            }
            -38 // -ENOSYS
        }
    };
    // 返回探针: M9 曾发现 ring3 收到 0x5101/0x5102 返回值与内核不一致 (DEV 项),
    // 此处如实记录内核侧返回值, 便于与用户侧对照。
    if nr == 0x5101 || nr == 0x5102 {
        serial::write_str("dbg  : nr=");
        print_dec(nr);
        serial::write_str(" -> ");
        print_dec(res as u64);
        serial::write_line("");
    }
    res
}

/// 从 LINUX_X64_SUBSET 中查 syscall 名 (M2: 日志可读性)
pub fn name_of(nr: u64) -> Option<&'static str> {
    LINUX_X64_SUBSET.iter().find(|(n, _)| *n as u64 == nr).map(|(_, s)| *s)
}

/// M3: 记录垫片绑定 (由 pe_loader 调用)
pub fn log_shim(dll: &str, func: &str, addr: u64) {
    serial::write_str("shim : ");
    serial::write_str(dll);
    serial::write_str("!");
    serial::write_str(func);
    serial::write_str(" -> trampoline ");
    print_hex(addr);
    serial::write_line("");
}

/// M3 调试: 十六进制日志 (pe_loader 使用)
pub fn log_hex(v: u64) {
    print_hex(v);
}

fn user_write(fd: u64, ptr: u64, len: u64) -> i64 {
    let _ = fd;
    // 用户地址范围检查: linux/win 低区 (0x400000..0x800000) 或 darwin 区
    // (0x100000000..0x100800000, M6 Mach-O 原生地址)
    let in_low = ptr >= 0x400000 && ptr <= 0x800000;
    let in_darwin = ptr >= 0x100000000 && ptr <= 0x100800000;
    if !in_low && !in_darwin {
        serial::write_line("syscall write: bad user pointer");
        return -14; // -EFAULT
    }
    let len = len.min(256) as usize;
    let src = ptr as *const u8;
    let mut line = [0u8; 288];
    let mut n = 0;
    for i in 0..len {
        let b = unsafe { src.add(i).read() };
        line[n] = b;
        n += 1;
    }
    serial::write_str(core::str::from_utf8(&line[..n]).unwrap_or("<non-utf8>"));
    vga::write_str(core::str::from_utf8(&line[..n]).unwrap_or("<non-utf8>"));
    len as i64
}

fn print_dec(v: u64) {
    let mut buf = [0u8; 24];
    let mut i = 24;
    let mut x = v;
    if x == 0 {
        serial::write_str("0");
        return;
    }
    while x > 0 {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    serial::write_str(core::str::from_utf8(&buf[i..]).unwrap());
}

fn print_hex(v: u64) {
    let mut buf = [0u8; 16];
    for i in 0..16 {
        let d = ((v >> (4 * i)) & 0xF) as u8;
        buf[15 - i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
    }
    serial::write_str("0x");
    serial::write_str(core::str::from_utf8(&buf).unwrap());
    serial::write_str(" ");
}

fn halt_forever() -> ! {
    loop {
        crate::hlt();
    }
}

fn dump_hex_bytes(addr: u64, n: usize) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    serial::write_str("test : bytes@0x400000: ");
    unsafe {
        for i in 0..n {
            let b = (addr as *const u8).add(i).read();
            let mut line = [0u8; 3];
            line[0] = HEX[(b >> 4) as usize];
            line[1] = HEX[(b & 0xF) as usize];
            line[2] = b' ';
            serial::write_str(core::str::from_utf8(&line).unwrap());
        }
    }
    serial::write_line("");
}

/// 进入用户态 (M2: 优先装载 multiboot 模块中的 ELF 文件; 回退内嵌二进制)。
pub fn enter_user_test(mbi: u32) -> ! {
    const LOAD_DEFAULT: u64 = 0x400000;
    const STACK: u64 = 0x600000;

    let mut load_addr: u64 = LOAD_DEFAULT;
    let mut used_module = false;

    // ---- M2/M3: 模块装载路径 (ELF 或 PE, 格式嗅探统一路由) ----
    match unsafe { find_module(mbi) } {
        Some((start, len, name_ptr)) => {
            // 模块名 (bootloader 提供零终止字符串)
            let mut name = [0u8; 64];
            let mut n = 0usize;
            unsafe {
                while n < 63 {
                    let b = name_ptr.add(n).read();
                    if b == 0 {
                        break;
                    }
                    name[n] = b;
                    n += 1;
                }
            }
            let name_s = core::str::from_utf8(&name[..n]).unwrap_or("?");
            serial::write_str("fmod : '");
            serial::write_str(name_s);
            serial::write_str("' @");
            print_hex(start as u64);
            print_dec(len as u64);
            serial::write_line(" bytes");

            let is_pe = unsafe {
                (start as *const u8).read() == b'M'
                    && (start as *const u8).add(1).read() == b'Z'
            };
            let is_macho = unsafe {
                let m = (start as *const u8).read();
                (m == 0xCF && (start as *const u8).add(1).read() == 0xFA)
                    || (m == 0xFE
                        && (start as *const u8).add(1).read() == 0xED
                        && (start as *const u8).add(2).read() == 0xFA)
            };
            if is_pe {
                serial::write_line("fmt  : PE32+ -> winsubsys (M3)");
                unsafe { crate::pe_loader::install_shims(); }
                match crate::pe_loader::load_pe(start, len) {
                    Ok(entry) => {
                        serial::write_str("pexc : ImageBase+EntryPoint=");
                        print_hex(entry);
                        serial::write_line("");
                        load_addr = entry;
                        used_module = true;
                    }
                    Err(e) => {
                        serial::write_str("pexc : FAILED (");
                        serial::write_str(e);
                        serial::write_line(") — fallback...");
                    }
                }
            } else if is_macho {
                serial::write_line("fmt  : Mach-O 64 -> darwinsubsys (M6)");
                match crate::macho_loader::load_macho(start, len) {
                    Ok(entry) => {
                        serial::write_str("mach : LC_SEGMENT_64 mapped, entry=");
                        print_hex(entry);
                        serial::write_line("");
                        load_addr = entry;
                        used_module = true;
                    }
                    Err(e) => {
                        serial::write_str("mach : FAILED (");
                        serial::write_str(e);
                        serial::write_line(") — fallback...");
                    }
                }
            } else {
                serial::write_line("fmt  : ELF64 -> linuxsubsys (M2)");
                match crate::elf_loader::load_elf(start, len) {
                    Ok(entry) => {
                        serial::write_str("elfx : entry=");
                        print_hex(entry);
                        serial::write_line("");
                        load_addr = entry;
                        used_module = true;
                    }
                    Err(e) => {
                        serial::write_str("elfx : FAILED (");
                        serial::write_str(e);
                        serial::write_line(") — fallback...");
                    }
                }
            }
        }
        None => {
            serial::write_line("fmod : no boot module (use -initrd) — embedded bin fallback");
        }
    }

    // ---- 回退: 内嵌二进制路径 (M1) ----
    if !used_module {
        let bin: &[u8] = include_bytes!("user_test.bin");
        serial::write_str("test : loading embedded user bin @0x400000 (");
        print_dec(bin.len() as u64);
        serial::write_line(" bytes)");
        unsafe {
            core::ptr::copy_nonoverlapping(bin.as_ptr(), LOAD_DEFAULT as *mut u8, bin.len());
        }
    }

    serial::write_line("test : iretq -> ring3 (cs=0x23 ss=0x1b, linux-x64 ABI)");
    unsafe { fujo_enter_user(load_addr, STACK) };
    unreachable!()
}

/// 解析 multiboot v1 模块表, 返回 (start, len, name)。
unsafe fn find_module(mbi: u32) -> Option<(u32, u32, *const u8)> {
    if mbi == 0 {
        return None;
    }
    let p = mbi as *const u32;
    let flags = p.read();
    if flags & 0x8 == 0 {
        return None;
    }
    let count = p.add(5).read();
    let mods_addr = p.add(6).read();
    if count == 0 || mods_addr == 0 {
        return None;
    }
    let m = mods_addr as *const u32;
    let start = m.read();
    let end = m.add(1).read();
    let name = *m.add(2) as *const u8;
    if end <= start {
        return None;
    }
    Some((start, end - start, name))
}
