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
    add rsp, 48
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

    match nr {
        // write(fd, buf, len)
        1 => user_write(a0, a1, a2),
        // exit(code) / exit_group(code) -> 内核接管并停机
        60 | 231 => {
            serial::write_line("user : sys_exit() — 内核接管, M1 验证完成");
            serial::write_str("timer : pit ticks=");
            print_dec(interrupts::ticks());
            serial::write_line(" (~100 Hz since boot)");
            halt_forever();
        }
        _ => {
            // 只打印前 5 次, 之后停机: 防止无限 spam 淹没日志
            let c = unsafe {
                let p = core::ptr::addr_of_mut!(spam_count);
                p.write_volatile(p.read_volatile() + 1);
                p.read_volatile()
            };
            if c <= 5 {
                serial::write_str("sys nr=");
                print_dec(nr);
                serial::write_str(" ret=");
                print_hex(ret);
                serial::write_str(" a0=");
                print_hex(a0);
                serial::write_str(" a1=");
                print_hex(a1);
                serial::write_str(" a2=");
                print_hex(a2);
                serial::write_line("");
            }
            if c > 6 {
                halt_forever();
            }
            -38 // -ENOSYS
        }
    }
}

fn user_write(fd: u64, ptr: u64, len: u64) -> i64 {
    let _ = fd;
    // 用户地址范围检查 (M1 单任务平坦空间: 用户程序 @0x400000, 栈 @0x600000)
    if ptr < 0x400000 || ptr > 0x800000 {
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

/// 进入用户态 (M1 单任务): 拷贝测试程序到 0x400000, iretq 到 ring3。
/// 注: include_bytes 保证在 .rodata, 与程序段一起被恒等映射覆盖。
pub fn enter_user_test() -> ! {
    const LOAD: u64 = 0x400000;
    const STACK: u64 = 0x600000;

    let bin: &[u8] = include_bytes!("user_test.bin");
    serial::write_str("test : loading user program @0x400000 (");
    print_dec(bin.len() as u64);
    serial::write_line(" bytes)");
    unsafe {
        core::ptr::copy_nonoverlapping(bin.as_ptr(), LOAD as *mut u8, bin.len());
    }
    // 诊断: dump 用户程序前 16 字节
    dump_hex_bytes(LOAD, 16);
    serial::write_line("test : iretq -> ring3 (cs=0x23 ss=0x1b, linux-x64 ABI)");
    unsafe { fujo_enter_user(LOAD, STACK) };
    unreachable!()
}
