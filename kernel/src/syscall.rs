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
/// M11: 用户 FPU/SIMD 状态 (syscall 进出保存恢复; 16 对齐 —— fxsave 要求)。
#[repr(C, align(16))]
pub struct FpuSave {
    data: [u8; 512],
}
#[no_mangle]
pub static mut fpu_saved: FpuSave = FpuSave { data: [0; 512] };

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
    fxsave [rip + fpu_saved]
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
    fxrstor [rip + fpu_saved]
    mov rsp, [rip + user_rsp_tmp]
    sysretq

    # ---- iretq 进入用户态 ----
    # rdi=entry, rsi=user_stack; 先 cli: 构造帧期间不允许中断 (M1 现场验证)
    # M23: 用户入口寄存器清零 (Linux _start 契约: rsp=argc 帧, 其他未定义但
    # glibc _start 用 rdx=rtld_fini; 清零保证 rtld_fini=NULL)。保留 rsp 语义。
    .p2align 4
    .global fujo_enter_user
fujo_enter_user:
    cli
    mov rax, cr3
    mov cr3, rax          # TLB flush (M2 原有; 幂等)
    mov r8, rdi
    mov r10, rsi
    xor rax, rax
    xor rbx, rbx
    xor rcx, rcx
    xor rdx, rdx
    xor rsi, rsi
    xor rdi, rdi
    xor r9, r9
    xor r11, r11
    xor r12, r12
    xor r13, r13
    xor r14, r14
    xor r15, r15
    mov r9, 60            # spare (未用)
    push 0x1b
    push r10
    push 0x202
    push 0x23
    push r8
    mov rax, r9
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

        // STAR:  kcs=0x08 @[47:32], user field=0x13 @[63:48]
        // sysret: CS=0x13+16=0x23 (RPL3!), SS=0x13+8=0x1B —— RPL 必须落进 STAR,
        // 否则 sysret 以 RPL0 返回 -> 用户实际跑在 CPL0 (M13 现场教训:
        // 无栈切换/中断帧紧凑/U-guard 失效, 三处异常同源)。
        let star = (0x08u64 << 32) | (0x13u64 << 48);
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
    let a4 = unsafe { args.add(4).read() };
    let a5 = unsafe { args.add(5).read() };

    let res = match nr {
        // read(fd, buf, len) — M15 VFS
        0 => crate::vfs::fujo_read(a0, a1, a2),
        // write(fd, buf, len)
        1 => user_write(a0, a1, a2),
        // open(path, flags, mode) — M15 VFS
        2 => crate::vfs::fujo_open(a0, a1, a2),
        // close(fd) — M15 VFS
        3 => crate::vfs::fujo_close(a0),        // ---- M11: 内存原语 (linux ABI 直通) ----
        // mmap(addr, len, prot, flags, fd, off) — 匿名私有子集
        9 => crate::mem::fujo_mmap(a0, a1, a2, a3, a4, a5),
        // munmap(addr, len) — v0 no-op
        11 => crate::mem::fujo_munmap(a0, a1),
        // brk(ptr) — 堆尾, 恒等 heap 区 bump
        12 => crate::mem::fujo_brk(a0),
        // getpid() (x86-64: 39) — linuxsubsys v0 最小实现
        39 => 1,
        // fork() — M22: 克隆当前任务 (v0 共享地址空间 + 用户栈物理拷贝)
        57 => fork_self(args),
        // execve(path, argv, envp) — M22 v0: 未实现 (M23 直通扩展)
        59 => -38, // -ENOSYS
        // ---------------------------------------------------------------
        // M21: linuxsubsys syscall 面扩展 (~20 个常用)
        // 原则: 行为合理的哨兵返回 + 必要回填 (用户缓冲地址检查同 VFS)。
        // ---------------------------------------------------------------
        // stat(path, buf) — 简化: mode=REG|0644, size=len(path)
        4 => sys_stat(a0, a1),
        // fstat(fd, buf)
        5 => sys_fstat(a0, a1),
        // lstat(path, buf) — 同 stat (无符号链接)
        6 => sys_stat(a0, a1),
        // writev(fd, iovec, count) — 逐个 iovec 写串口
        20 => sys_writev(a0, a1, a2),
        // access(path, mode) -> 0 (允许)
        21 => 0,
        // pipe(fds[2]) — linux ABI 22 号 (M18 内核实现)
        22 => crate::ipc::fujo_pipe(a0),
        // nanosleep(req, rem) — PIT 忙等 (100Hz 粒度)
        35 => sys_nanosleep(a0),
        // uname(buf) — 回填 c_* 字段 (FujoOS)
        63 => sys_uname(a0),
        // gettimeofday(tv, tz) — 单调钟 (PIT ticks 派生)
        78 => sys_gettimeofday(a0, a1),
        // getuid/getgid/geteuid/getegid -> 1000
        102 => 1000,
        104 => 1000,
        107 => 1000,
        108 => 1000,
        // arch_prctl(arch, addr) — ARCH_SET_FS=0x1002 写 FS_BASE (glibc TLS);
        // ARCH_GET_FS=0x1003 读回。M23: busybox glibc %fs 寻址必需。
        // v0: 写 MSR_FS_BASE; 多任务切换保存/恢复由 sched::save/restore 处理。
        158 => {
            match a0 {
                0x1002 => {
                    unsafe {
                        core::arch::asm!(
                            "wrmsr",
                            in("ecx") 0xC000_0100u32,
                            in("eax") a1 as u32,
                            in("edx") (a1 >> 32) as u32,
                            options(nomem, nostack, preserves_flags)
                        );
                    }
                    0
                }
                0x1003 => {
                    if user_ok(a1, 8) {
                        let lo: u32;
                        let hi: u32;
                        unsafe {
                            core::arch::asm!(
                                "rdmsr",
                                in("ecx") 0xC000_0100u32,
                                out("eax") lo,
                                out("edx") hi,
                                options(nomem, nostack, preserves_flags)
                            );
                            (a1 as *mut u64).write((lo as u64) | ((hi as u64) << 32));
                        }
                    }
                    0
                }
                _ => 0,
            }
        }
        // prctl(option, ...) -> 0 (no-op)
        157 => 0,
        // mprotect(addr, len, prot) -> 0 (直通; busybox 除 exec 区外全 RWX)
        10 => 0,
        // set_tid_address(ptr) -> tid
        218 => crate::sched::current_task() as i64 + 1,
        // set_robust_list(ptr, len) -> 0
        273 => 0,
        // rseq(ptr, len, flags, sig) -> -ENOSYS (glibc 可回退)
        334 => -38,
        // get_robust_list -> 0
        274 => 0,
        // gettid -> 当前任务 id+1
        186 => crate::sched::current_task() as i64 + 1,
        // time(ptr) -> 单调秒
        201 => sys_time(a0),
        // futex(op, uaddr, val) -> 0 (no-op)
        202 => 0,
        // openat(dirfd, path, flags, mode) — 转发 open (忽略 dirfd=AT_FDCWD)
        257 => crate::vfs::fujo_open(a1, a2, a3),
        // getrandom(buf, len, flags) — PIT 混哈希假熵
        317 => sys_getrandom(a0, a1),
        // ---- fujo 原生 Win32 shim 通道 (M3) ----
        // kernel32!WriteFile (fd, buf, len)
        0x5001 => user_write(a0, a1, a2),
        // kernel32!ExitProcess (code)
        0x5002 => {
            serial::write_line("user : ExitProcess(0) - kernel takeover, M3 verified");
            serial::write_str("timer : pit ticks=");
            print_dec(interrupts::ticks());
            serial::write_line(" (~100 Hz since boot)");
            halt_forever();
        }
        // exit(code) / exit_group(code) -> 内核接管并停机
        60 | 231 => {
            serial::write_line("user : sys_exit() - kernel takeover, M6 verified");
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
        // fujo_get_task_id() -> tid (M14: 进程/任务标识原语)
        0x5105 => crate::sched::current_task() as i64,
        // ---- M18: IPC 原语 (管道/共享内存/信号) ----
        // fujo_pipe(ptr) -> 0 (ptr 处写 [rfd, wfd])
        0x5110 => crate::ipc::fujo_pipe(a0),
        // fujo_shm() -> 共享窗口基址 0xA00000
        0x5111 => crate::ipc::fujo_shm(),
        // fujo_sigset(handler) -> 0
        0x5120 => crate::ipc::fujo_sigset(a0),
        // fujo_sigkill(tid, sig) -> 0
        0x5121 => crate::ipc::fujo_sigkill(a0, a1),
        // fujo_sigret() -> 0
        0x5122 => crate::ipc::fujo_sigret(),
        // ---- M19: 内核对象/句柄表 (统一资源抽象) ----
        // fujo_kobj_create(kind) -> slot
        0x5130 => crate::kobj::fujo_kobj_create(a0),
        // fujo_kobj_free(handle) -> 0
        0x5131 => crate::kobj::fujo_kobj_free(a0),
        // fujo_kobj_info(ptr, n) -> 写入 i32×min(4,n) 计数
        0x5132 => crate::kobj::fujo_kobj_info(a0, a1),
        // ---- darwin BSD 空间 (0x2000000|nr, M6 darwinsubsys) ----
        0x200_0001 => {
            serial::write_line("user : darwin exit() - kernel takeover, M6 verified");
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

fn user_write(fd: u64, ptr: u64, len: u64) -> i64 {    // M15: fd>=3 先走 VFS (内存盘追加); /dev/tty 与 fd<3 走串口
    if fd >= 3 {
        if let Some(n) = crate::vfs::file_write(fd, ptr, len) {
            return n;
        }
        // /dev/tty: 落到串口
    }
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

/// M22 fork 实现: 从当前 syscall 帧克隆任务。
/// 帧布局 (fujo_syscall_entry push 序, 栈顶->下):
///   [0]=rdi [1]=rsi [2]=rdx [3]=r10 [4]=r8 [5]=r9 [6]=rcx [7]=r11
/// 用户返回 RIP = rcx (syscall 指令后的地址, sysretq 用); RSP = user_rsp_tmp。
fn fork_self(args: *const u64) -> i64 {
    unsafe {
        let rip = args.add(6).read(); // rcx = 用户返回地址
        let rsp = user_rsp_tmp;
        let regs8: [u64; 8] = [
            args.add(7).read(), // r11
            args.add(3).read(), // r10
            args.add(5).read(), // r9
            args.add(4).read(), // r8
            args.add(0).read(), // rdi
            args.add(1).read(), // rsi
            args.add(2).read(), // rdx
            args.add(6).read(), // rcx
        ];
        match crate::sched::fork_current(rip, rsp, &regs8) {
            Some(tid) => {
                // 父返回子 tid (=1 首个); 子返回 0 (rax 槽置零)
                serial::write_str("fork : parent returns tid ");
                print_dec(tid as u64);
                serial::write_line("");
                tid as i64
            }
            None => -12, // -ENOMEM (任务表满)
        }
    }
}

/// M21: linuxsubsys syscall 面扩展实现 (~20 个常用)

/// 用户指针区域检查 (linux 低区 + darwin 区)。
fn user_ok(ptr: u64, len: u64) -> bool {
    let in_low = ptr >= 0x400000 && ptr <= 0x800000;
    let in_darwin = ptr >= 0x100000000 && ptr <= 0x100800000;
    in_low || in_darwin
}

/// stat(path, buf): 简化填充 — mode=REG|0644(size=路径长度), dev/ino 固定。
fn sys_stat(ptr: u64, buf: u64) -> i64 {
    if !user_ok(buf, 128) {
        return -14; // -EFAULT
    }
    let mut len = 0u64;
    unsafe {
        if user_ok(ptr, 1) {
            while len < 255 {
                let b = (ptr as *const u8).add(len as usize).read();
                if b == 0 {
                    break;
                }
                len += 1;
            }
        }
        let s = buf as *mut u64;
        // struct stat (x86_64): st_dev(0) st_ino(8) st_nlink(16) st_mode(24=u32)
        s.add(0).write(1u64); // st_dev
        s.add(1).write(1u64); // st_ino
        s.add(2).write(1u64); // st_nlink
        (s.add(3) as *mut u32).write(0o100644); // S_IFREG|0644
        (s.add(4) as *mut u32).write(1000u32); // uid
        ((s.add(4) as *mut u32).add(1)).write(1000u32); // gid
        s.add(6).write(len); // st_size
    }
    0
}

/// fstat(fd, buf): 与 stat 相同简化。
fn sys_fstat(fd: u64, buf: u64) -> i64 {
    let _ = fd;
    sys_stat(0, buf)
}

/// writev(fd, iov, cnt): iovec 数组 [{base,len}..], 逐段写 (串口直通)。
fn sys_writev(fd: u64, iov: u64, cnt: u64) -> i64 {
    if !user_ok(iov, cnt.saturating_mul(16)) || cnt > 64 {
        return -14; // -EFAULT
    }
    let mut total = 0i64;
    unsafe {
        for i in 0..cnt as usize {
            let base = (iov as *const u64).add(i * 2).read();
            let len = (iov as *const u64).add(i * 2 + 1).read();
            let n = user_write(fd, base, len);
            if n < 0 {
                return n;
            }
            total += n;
        }
    }
    total
}

/// nanosleep(req, _rem): v1 模型约束 no-op。
/// 说明: SFMASK=0x200 在 syscall 期间屏蔽 IF, 内核态无法等待 PIT 中断;
/// 真正的睡眠在调度器 wakeup 后实现 (M22+)。此刻返回 0 (立即完成),
/// 用户态忙等/时间推进由 gettimeofday 用户态调用验证。
fn sys_nanosleep(_req: u64) -> i64 {
    0
}

/// uname(buf): utsname 回填 (FujoOS / fujokernel / fujo / x86_64)。
fn sys_uname(buf: u64) -> i64 {
    if !user_ok(buf, 256) {
        return -14;
    }
    unsafe {
        let u = buf as *mut u8;
        let mut off = 0usize;
        for field in [
            b"FujoOS\0".as_slice(),
            b"FujoKernel\0".as_slice(),
            b"0.1.0\0".as_slice(),
            b"FujoOS\0".as_slice(),
            b"x86_64\0".as_slice(),
        ] {
            for &c in field {
                if off < 255 {
                    u.add(off).write(c);
                    off += 1;
                }
            }
        }
    }
    0
}

/// gettimeofday(tv, tz): 单调钟 (PIT ticks/100 = 秒)。
fn sys_gettimeofday(tv: u64, tz: u64) -> i64 {
    let _ = tz;
    if !user_ok(tv, 16) {
        return -14;
    }
    let ticks = crate::interrupts::ticks();
    let sec = ticks / 100;
    let usec = (ticks % 100) * 10000;
    unsafe {
        (tv as *mut u64).write(sec);
        (tv as *mut u64).add(1).write(usec);
    }
    0
}

/// time(ptr): 单调秒 (PIT ticks/100)。
fn sys_time(ptr: u64) -> i64 {
    let ticks = crate::interrupts::ticks();
    let sec = (ticks / 100) as i64;
    if ptr != 0 && user_ok(ptr, 8) {
        unsafe { (ptr as *mut i64).write(sec); }
    }
    sec
}

/// getrandom(buf, len, _flags): PIT 混哈希伪熵 (非加密, 仅时序验证)。
fn sys_getrandom(buf: u64, len: u64) -> i64 {
    if !user_ok(buf, len) {
        return -14;
    }
    let n = len.min(64) as usize;
    unsafe {
        for i in 0..n {
            let tick = crate::interrupts::ticks();
            let x = (tick.wrapping_mul(0x9E37_79B9).rotate_left(13)
                ^ (i as u64).wrapping_mul(0x85EB_CA6B))
                & 0xFF;
            (buf as *mut u8).add(i).write(x as u8);
        }
    }
    n as i64
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
    // 用户栈初始 RSP: %16==8 (SysV 函数入口约定: clang 生成的 _start 按
    // "call 之后 rsp%16==8" 布局, 若给 0x600000(%16==0) 则 movaps 类 16 对齐
    // 访问错位 -> #GP; M17 res_test 现场 0x400156 movaps 实证)
    const STACK: u64 = 0x5FFFF8;

    let mut load_addr: u64 = LOAD_DEFAULT;
    let mut used_module = false;
    // M23: argv 模式 (busybox 等真 libc 程序需要 argc/argv/envp 栈帧)
    let argv_mode = crate::shell::argv_mode();

    // ---- M2/M3: 模块装载路径 (ELF 或 PE, 格式嗅探统一路由) ----
    match unsafe { module_snapshot().or_else(|| find_module(mbi)) } {
        Some((mut start, mut len, name_ptr)) => {
            // M17: FUJR 容器嗅探 -> 提取 EMBED 可执行体
            let is_run = unsafe {
                (start as *const u8).read() == b'F'
                    && (start as *const u8).add(1).read() == b'U'
                    && (start as *const u8).add(2).read() == b'J'
                    && (start as *const u8).add(3).read() == b'R'
            };
            if is_run {
                if let Some((eaddr, elen)) = crate::fujr::load(start as u64, len as u64) {
                    start = eaddr as u32;
                    len = elen as u32;
                    serial::write_line("run  : exec extracted -> format sniff");
                }
            }
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
                        serial::write_line(") - fallback...");
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
                        serial::write_line(") - fallback...");
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
                        serial::write_line(") - fallback...");
                    }
                }
            }
        }
        None => {
            serial::write_line("fmod : no boot module (use -initrd) - embedded bin fallback");
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
    // M13: 双任务模式 (os run threads) —— 装载后克隆第二个任务 (同一镜像, 独立栈)
    if crate::sched::multi_task() {
        crate::sched::spawn_tasks(load_addr);
    }
    // M23: argv 模式 —— 用户栈顶构造 [argc][argv…][0][envp…][0][auxv…][0]
    // 静态 glibc busybox 初始化需要 auxv (AT_PHDR/AT_PHNUM/AT_ENTRY/AT_RANDOM
    // /AT_SECURE/AT_NULL) 用于 TLS 与 libc 早期状态 (M23 现场: 缺 auxv ->
    // __libc_start_main 读垃圾指针 #PF cr2=rip=0x56198468)。
    let mut user_rsp = STACK;
    if argv_mode {
        // 帧区选 0x5F0000 起 (与 STACK=0x5FFFF8 不冲突的独立区域)
        let sp0 = 0x5F0000u64;
        unsafe {
            // 字符串: argv0="busybox" 逆序放 (栈向低生长)
            let mut cur = sp0;
            let argv0 = b"busybox\0";
            for &b in argv0.iter().rev() {
                cur -= 1;
                (cur as *mut u8).write(b);
            }
            let mut ptrs = [0u64; 8];
            ptrs[0] = cur; // argv[0]
            // 指针区放 0x5F0400: [argc][argv0][0][envp][0(空envp)][auxv...][0]
            let argp = 0x5F0400u64;
            let n = 1usize;
            (argp as *mut u64).write(n as u64); // argc=1
            (argp as *mut u64).add(1).write(ptrs[0]);
            (argp as *mut u64).add(2).write(0u64); // argv 结束
            (argp as *mut u64).add(3).write(0u64); // envp 结束 (无环境)
            // auxv (起始于 argp+4*8): 至少 AT_PHDR/AT_PHNUM/AT_ENTRY/AT_SECURE/AT_NULL
            // 注意 AT_PHDR 必须指向 ELF program header —— busybox 的入口已知
            // 0x40b300, ELF 头在 0x400000, 段表在 0x400040 (e_phoff=64)
            let aux = argp + 32;
            (aux as *mut u64).add(0).write(3u64); // AT_PHDR
            (aux as *mut u64).add(1).write(0x400040u64);
            (aux as *mut u64).add(2).write(4u64); // AT_PHENT
            (aux as *mut u64).add(3).write(56u64);
            (aux as *mut u64).add(4).write(5u64); // AT_PHNUM
            (aux as *mut u64).add(5).write(5u64);
            (aux as *mut u64).add(6).write(9u64); // AT_ENTRY
            (aux as *mut u64).add(7).write(0x40b300u64);
            (aux as *mut u64).add(8).write(23u64); // AT_SECURE
            (aux as *mut u64).add(9).write(0u64);
            (aux as *mut u64).add(10).write(25u64); // AT_RANDOM
            let rnd = 0x5F0300u64;
            for k in 0..16usize {
                ((rnd + k as u64) as *mut u8).write((0x41 + k) as u8);
            }
            (aux as *mut u64).add(11).write(rnd);
            (aux as *mut u64).add(12).write(6u64); // AT_PAGESZ
            (aux as *mut u64).add(13).write(0x1000u64);
            (aux as *mut u64).add(14).write(0u64); // AT_NULL
            (aux as *mut u64).add(15).write(0u64);
            user_rsp = argp;
            serial::write_str("argv : argc=1 stack @");
            print_hex(argp);
            serial::write_line("");
        }
    }
    unsafe { fujo_enter_user(load_addr, user_rsp) };
    unreachable!()
}

/// M15: 引导模块信息 (addr, len) —— VFS /boot/module 后端。
pub fn boot_module_info(mbi: u32) -> Option<(u64, u64)> {
    unsafe {
        find_module(mbi).map(|(s, l, _)| (s as u64, l as u64))
    }
}

// 模块快照: 引导期记录一次 (enter 阶段二次解析 mbi 偶发不可靠 —— 快照绕过)
static mut MOD_SNAP: (u32, u32, u32) = (0, 0, 0);

/// 引导期调用: 记住 (start, len, name_ptr)。
pub fn remember_module(mbi: u32) {
    unsafe {
        if let Some((s, l, n)) = find_module(mbi) {
            MOD_SNAP = (s, l, n as u32);
        }
    }
}

pub fn module_snapshot() -> Option<(u32, u32, *const u8)> {
    unsafe {
        let (s, l, n) = MOD_SNAP;
        if s == 0 || l == 0 || n == 0 {
            return None;
        }
        Some((s, l, n as *const u8))
    }
}

/// 解析 multiboot v1 模块表, 返回 (start, len, name)。
unsafe fn find_module(mbi: u32) -> Option<(u32, u32, *const u8)> {    if mbi == 0 {
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
