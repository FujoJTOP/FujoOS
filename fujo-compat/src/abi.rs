//! 三平台 ABI 表 —— 兼容层"声明式映射"的起点。
//!
//! 设计：内核/用户垫片不用写 C 代码，而是把这些表编译进：
//! - 内核 syscall gate（Linux 第一公民，直接映射到 FujoOS 内部调用）
//! - darwin 兼容层（BSD 空间 + Mach trap 空间）
//! - win32 垫片交互（ntdll 由 FujoOS 提供，API 级映射）

// ---------------------------------------------------------------------------
// Linux x86_64 syscall 表（第一公民 ABI）
// 来源: linux arch/x86/entry/syscalls/syscall_64.tbl（公开内核源码规范）
// ---------------------------------------------------------------------------
pub const LINUX_X64_TABLE: &[(u16, &str)] = &[
    (0, "read"), (1, "write"), (2, "open"), (3, "close"), (4, "stat"), (5, "fstat"),
    (6, "lstat"), (7, "poll"), (8, "lseek"), (9, "mmap"), (10, "mprotect"), (11, "munmap"),
    (12, "brk"), (13, "rt_sigaction"), (14, "rt_sigprocmask"), (15, "rt_sigreturn"), (16, "ioctl"),
    (17, "pread64"), (18, "pwrite64"), (19, "readv"), (20, "writev"), (21, "access"),
    (22, "pipe"), (23, "select"), (24, "sched_yield"), (25, "mremap"), (32, "dup"),
    (33, "dup2"), (34, "pause"), (35, "nanosleep"), (37, "alarm"), (38, "setitimer"),
    (40, "sendfile"), (41, "socket"), (42, "connect"), (43, "accept"), (44, "sendto"),
    (45, "recvfrom"), (46, "sendmsg"), (47, "recvmsg"), (48, "shutdown"), (49, "bind"),
    (50, "listen"), (51, "getsockname"), (52, "getpeername"), (53, "socketpair"),
    (54, "setsockopt"), (55, "getsockopt"), (56, "clone"), (57, "fork"), (58, "vfork"),
    (59, "execve"), (60, "exit"), (61, "wait4"), (62, "kill"), (63, "uname"),
    (72, "fcntl"), (73, "flock"), (74, "fsync"), (75, "fdatasync"), (78, "gettimeofday"),
    (79, "getcwd"), (80, "chdir"), (81, "fchdir"), (82, "rename"), (83, "mkdir"), (84, "rmdir"),
    (85, "creat"), (86, "link"), (87, "unlink"), (88, "readlink"), (89, "chmod"),
    (90, "fchmod"), (91, "chown"), (92, "fchown"), (93, "lchown"), (94, "umask"),
    (96, "getppid"), (97, "getpgrp"), (98, "setsid"), (102, "getuid"), (104, "getgid"),
    (105, "setuid"), (106, "setgid"), (107, "geteuid"), (108, "getegid"), (110, "getpgid"),
    (111, "setpgid"), (114, "gettid"), (115, "sysinfo"), (116, "getsid"), (157, "prctl"),
    (158, "arch_prctl"), (186, "gettid"), (231, "exit_group"), (257, "openat"),
    (258, "mkdirat"), (262, "unlinkat"), (263, "renameat"), (264, "linkat"), (265, "symlinkat"),
    (266, "readlinkat"), (267, "fchmodat"), (268, "faccessat"), (269, "pselect6"),
    (270, "ppoll"), (272, "set_robust_list"), (273, "get_robust_list"), (274, "splice"),
    (279, "utimensat"), (280, "epoll_pwait"), (283, "eventfd"), (284, "fallocate"),
    (287, "accept4"), (289, "eventfd2"), (290, "epoll_create1"), (291, "dup3"), (292, "pipe2"),
    (293, "inotify_init1"), (294, "preadv"), (295, "pwritev"), (297, "perf_event_open"),
    (298, "recvmmsg"), (301, "prlimit64"), (305, "syncfs"), (306, "sendmmsg"),
    (312, "finit_module"), (313, "sched_setattr"), (314, "sched_getattr"), (315, "renameat2"),
    (316, "seccomp"), (317, "getrandom"), (318, "memfd_create"), (320, "bpf"),
    (321, "execveat"), (322, "userfaultfd"), (323, "membarrier"), (324, "mlock2"),
    (325, "copy_file_range"), (326, "preadv2"), (327, "pwritev2"), (331, "statx"),
    (333, "rseq"),
];

pub fn linux_x64_name(nr: u64) -> Option<&'static str> {
    LINUX_X64_TABLE
        .iter()
        .find(|(n, _)| *n as u64 == nr)
        .map(|(_, name)| *name)
}

/// Linux i386 表（32 位兼容入口 int 0x80 / sysenter）—— M2 填充。
pub const LINUX_I386_TABLE: &[(u16, &str)] = &[
    (1, "exit"), (3, "read"), (4, "write"), (5, "open"), (6, "close"),
    (19, "lseek"), (20, "getpid"), (33, "access"), (39, "mkdir"),
];

// ---------------------------------------------------------------------------
// Darwin (macOS) x86_64: BSD 用户空间 = 0x2000000 | i386nr
// ---------------------------------------------------------------------------
pub const DARWIN_X64_TABLE: &[(u64, &str)] = &[
    (0x200_0001, "exit"), (0x200_0002, "fork"), (0x200_0003, "read"), (0x200_0004, "write"),
    (0x200_0005, "open"), (0x200_0006, "close"), (0x200_0013, "lseek"), (0x200_0014, "getpid"),
    (0x200_00C5, "mmap"), (0x200_0049, "munmap"), (0x200_004A, "mprotect"), (0x200_00B7, "fcntl"),
];

/// Mach trap 空间（0x1F0000 区段）—— M6 通过 mach_msg 兼容层实现；此处仅声明。
pub const DARWIN_MACH_TRAPS: &[(u16, &str)] = &[
    (0x01, "mach_msg_trap"), (0x02, "mach_msg_overwrite_trap"), (0x03, "mach_msg2_trap"),
    (0x05, "mach_reply_port"), (0x08, "task_self_trap"), (0x09, "task_for_pid"),
    (0x0E, "mach_vm_allocate"), (0x0F, "mach_vm_deallocate"), (0x12, "mach_vm_protect"),
    (0x15, "vm_map"), (0x19, "mach_port_deallocate"),
];

// ---------------------------------------------------------------------------
// Windows: 不依赖 syscall 编号（不透明），走 API 级垫片 —— 需要提供的 shim DLL
// ---------------------------------------------------------------------------
pub const WIN32_SHIM_MODULES: &[&str] = &[
    "ntdll", "kernel32", "kernelbase", "user32", "gdi32", "ws2_32", "advapi32",
    "shell32", "ole32", "comctl32", "d3d9", "d3d11", "dxgi", "xaudio2", "winmm",
    "imm32", "shlwapi", "version", "dwrite", "d2d1", "uxtheme", "dwmapi", "setupapi",
];

/// Win32 子系统 API 集（kernel32 摘录，M3 首批目标）。
pub const WIN32_KERNEL32_API_FIRST: &[&str] = &[
    "ExitProcess", "GetCommandLineW", "GetStdHandle", "WriteFile", "ReadFile",
    "HeapAlloc", "HeapFree", "GetProcAddress", "LoadLibraryW", "GetModuleHandleW",
    "GetLastError", "SetLastError", "CreateFileW", "CloseHandle", "FindFirstFileW",
    "Sleep", "GetTickCount64", "VirtualAlloc", "VirtualFree", "CreateThread",
];

// ---------------------------------------------------------------------------
// 入口（M1 的 syscall gate 会使用）
// ---------------------------------------------------------------------------

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

/// 当前 syscall gate 的占位分发（M1 接入真实内核服务）。
/// 返回 -ENOSYS (-38) 表示未实现。
pub fn dispatch(abi: Abi, nr: u64, args: &[u64; 6]) -> i64 {
    let _ = args;
    // placeholder: 真实实现会把 abi+nr 映射到内核服务表
    -38
}
