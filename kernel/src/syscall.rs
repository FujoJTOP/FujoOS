//! Syscall ABI 层（M0 骨架）。
//!
//! 设计: FujoOS 在 Ring0 提供"syscall personality"——同一内核服务面被
//! 三套 ABI 表映射使用。第一公民是 Linux x86_64 表（应用生态最大、
//! 无需用户态垫片即可原生速度运行）；darwin 走 BSD+mach trap 兼容表；
//! win32 因 syscall 编号不透明，由用户态 ntdll/其他 shim DLL 再走本表。
//!
//! M1 会把这个表喂给真正的 syscall gate（MSR LSTAR + 内核栈切换 +
//! 参数校验 + 服务分发），本文件先承载表数据与分发占位。

/// Linux x86_64 子集（M0 内核内嵌, 供启动日志展示）;
/// 完整 334 项规范表由 tools/gen_syscall_tbl.py 从公开 syscall_64.tbl 生成。
pub const LINUX_X64_SUBSET: &[(u16, &str)] = &[
    (0, "read"),
    (1, "write"),
    (2, "open"),
    (3, "close"),
    (4, "stat"),
    (5, "fstat"),
    (6, "lstat"),
    (7, "poll"),
    (8, "lseek"),
    (9, "mmap"),
    (10, "mprotect"),
    (11, "munmap"),
    (12, "brk"),
    (13, "rt_sigaction"),
    (14, "rt_sigprocmask"),
    (16, "ioctl"),
    (17, "pread64"),
    (18, "pwrite64"),
    (19, "readv"),
    (20, "writev"),
    (21, "access"),
    (22, "pipe"),
    (23, "select"),
    (24, "sched_yield"),
    (32, "dup"),
    (35, "nanosleep"),
    (41, "socket"),
    (42, "connect"),
    (43, "accept"),
    (57, "fork"),
    (59, "execve"),
    (60, "exit"),
    (61, "wait4"),
    (63, "uname"),
    (72, "fcntl"),
    (78, "gettimeofday"),
    (79, "getcwd"),
    (157, "prctl"),
    (158, "arch_prctl"),
    (231, "exit_group"),
    (257, "openat"),
    (317, "getrandom"),
    (318, "memfd_create"),
];

/// Darwin x86_64 BSD 空间子集（0x2000000 | i386nr）。
pub const DARWIN_X64_SUBSET: &[(u64, &str)] = &[
    (0x200_0001, "exit"),
    (0x200_0003, "read"),
    (0x200_0004, "write"),
    (0x200_0005, "open"),
    (0x200_0006, "close"),
    (0x200_0013, "lseek"),
    (0x200_0014, "getpid"),
    (0x200_00C5, "mmap"),
];

pub const WIN32_SHIM_PLAN: &[&str] = &[
    "ntdll", "kernel32", "user32", "gdi32", "ws2_32", "advapi32",
    "d3d9", "d3d11", "dxgi", "xaudio2",
];

pub fn linux_x64_count() -> usize {
    LINUX_X64_SUBSET.len()
}

pub fn darwin_x64_count() -> usize {
    DARWIN_X64_SUBSET.len()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abi {
    LinuxX64,
    DarwinX64,
}

impl Abi {
    pub fn name(&self) -> &'static str {
        match self {
            Abi::LinuxX64 => "linux-x64",
            Abi::DarwinX64 => "darwin-x64",
        }
    }
}

/// syscall gate 分发骨架（M1 内核化）。
/// 返回 -ENOSYS (=-38) 表示未实现。
pub fn dispatch(abi: Abi, nr: u64, args: &[u64; 6]) -> i64 {
    let _ = (abi, args);
    match (abi, nr) {
        // (Abi::LinuxX64, 0) => todo!("read"),
        _ => -38,
    }
}
