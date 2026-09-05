//! shell.rs — os shell v0 (M10.1): 启动 Logo 之后的内核命令行
//!
//! 命令:
//!   os run hermes   装载并启动 Hermes CLI (ring3, 模型调用通道)
//!   help / ?        命令列表
//! 纯命令驱动: 无输入时一直等待, 不自动执行 (用户要求: 只有输入
//! `os run hermes` 才启动 Hermes)。
//! 输入: PS/2 键盘环形缓冲 (IRQ1); 回显到串口 (VGA 文本平面在 VBE 切换后
//! 不可见, 保持串口为唯一日志通道 —— 真机/图形版由后续 fujokit 承担)。

use crate::interrupts;
use crate::kbd;
use crate::serial;
use crate::syscall;
use crate::vga;

fn out_raw(s: &str) {
    vga::write_str(s);
    serial::write_str(s);
}

fn out_line(s: &str) {
    vga::write_line(s);
    serial::write_line(s);
}

/// 16 位十六进制字符串 (内核态打印辅助)。
fn hex_str(v: u64) -> &'static [u8; 18] {
    static mut BUF: [u8; 18] = [0; 18];
    unsafe {
        const HX: &[u8; 16] = b"0123456789abcdef";
        BUF[0] = b'0';
        BUF[1] = b'x';
        for i in 0..16 {
            let d = ((v >> (4 * (15 - i))) & 0xF) as u8;
            BUF[2 + i] = HX[d as usize];
        }
        &BUF
    }
}

/// 十进制字符串辅助 (shell dump)。
#[allow(dead_code)]
fn dec_str(v: u64) -> &'static [u8; 24] {
    static mut BUF: [u8; 24] = [0; 24];
    unsafe {
        let mut x = v;
        let mut i = 24;
        if x == 0 {
            BUF[23] = b'0';
            return &BUF;
        }
        while x > 0 {
            i -= 1;
            BUF[i] = b'0' + (x % 10) as u8;
            x /= 10;
        }
        &BUF
    }
}

/// 读取一行 (环形缓冲轮询, 无 hlt —— TCG 安全); 仅在回车后返回。
fn read_line(buf: &mut [u8]) -> usize {
    let mut n = 0usize;
    loop {
        while let Some(c) = kbd::try_poll() {
            match c {
                '\n' => {
                    out_raw("\n");
                    return n;
                }
                '\x08' => {
                    if n > 0 {
                        n -= 1;
                        out_raw("\x08 \x08");
                    }
                }
                _ => {
                    if n < buf.len() {
                        buf[n] = c as u8;
                        n += 1;
                        out_raw(core::str::from_utf8(&buf[n - 1..n]).unwrap_or(""));
                    }
                }
            }
        }
        // 无输入: 持续轮询 (纯命令驱动, 不自动执行)
        let _ = interrupts::ticks();
    }
}

/// M23: argv 模式标志 (shell 设置, enter_user_test 读取 —— 真 libc 程序
/// 需要 argc/argv 栈帧; 关闭则保持演示程序的裸栈约定)。
static mut ARGV_MODE: bool = false;

pub fn set_argv_mode(on: bool) {
    unsafe {
        ARGV_MODE = on;
    }
}

pub fn argv_mode() -> bool {
    unsafe { ARGV_MODE }
}

/// M23b: busybox 命令参数 (argv[1..]), 由 `os run busybox <args...>` 解析。
/// v0: 单槽 (最大 8 参数, 每参数 31 字符)。
static mut ARGV_CMD: [[u8; 32]; 8] = [[0; 32]; 8];
static mut ARGV_CMD_N: usize = 0;

pub fn set_argv_cmd(words: &[&str]) {
    unsafe {
        ARGV_CMD_N = words.len().min(8);
        for (i, w) in words.iter().enumerate().take(ARGV_CMD_N) {
            let bytes = w.as_bytes();
            let n = bytes.len().min(31);
            let mut k = 0;
            while k < n {
                ARGV_CMD[i][k] = bytes[k];
                k += 1;
            }
            ARGV_CMD[i][k] = 0;
        }
    }
}

pub fn argv_cmd() -> &'static [[u8; 32]; 8] {
    unsafe { &ARGV_CMD }
}

pub fn argv_cmd_n() -> usize {
    unsafe { ARGV_CMD_N }
}

static mut ARGV0: [u8; 32] = [0; 32];
static mut ARGV0_LEN: usize = 0;

/// W16: argv[0] 通用化 (os run <app> [args...] 用; busybox 路径兼容默认值)。
pub fn set_argv0(s: &str) {
    unsafe {
        let b = s.as_bytes();
        let n = b.len().min(31);
        for k in 0..n {
            ARGV0[k] = b[k];
        }
        ARGV0[n] = 0;
        ARGV0_LEN = n;
    }
}

pub fn argv0_bytes() -> &'static [u8] {
    unsafe { &ARGV0[..ARGV0_LEN] }
}

pub fn shell(mbi: u32) -> ! {
    vga::set_color(0x07);
    out_line("");
    out_line("os   : fujo shell v0 - commands:");
    out_line("os   :   os run <app>    launch app (hermes|threads|...; registry names too)");
    out_line("os   :   app list        show app registry");
    out_line("os   :   ls              list /boot /proc /dev /tmp");
    out_line("os   :   cat <path>      dump file (kernel VFS)");
    out_line("os   :   runfile <path>  run ELF from file (W16 compile output)");
    out_line("os   :   echo <text>     print text");
    out_line("os   :   help            show this list");
    shell_loop(mbi)
}

/// W34/FUFORALL: `.shell` 解释器 (FUJR 容器 EMBED = 脚本文本, 首行 `#!fujoshell`).
/// 命令集 (最小): `#` 注释 / `echo <text>` 输出 / 空白行; 未知行报告后继续。
pub fn run_script(p: u64, len: u64) -> ! {
    out_line("script: #!fujoshell interpreter");
    vga::set_color(0x07);
    let src = p as *const u8;
    let mut i = 0usize;
    let mut line = [0u8; 64];
    let mut ln = 0usize;
    loop {
        if i >= len as usize {
            break;
        }
        let b = unsafe { src.add(i).read() };
        if b == b'\n' || i == (len as usize) - 1 {
            if b != b'\n' && ln < 63 {
                line[ln] = b;
                ln += 1;
            }
            let s = core::str::from_utf8(&line[..ln]).unwrap_or("");
            let t = s.trim();
            if t.starts_with("echo ") {
                out_line(&t[5..]);
            } else if !t.is_empty() && !t.starts_with('#') {
                out_line("script: unknown command (ignored)");
            }
            ln = 0;
        } else if ln < 63 {
            line[ln] = b;
            ln += 1;
        }
        i += 1;
    }
    out_line("script: EOF");
    loop {}
}

/// W16: shell 主循环 (exit_to_shell 的跳转目标; 不再返回)。
pub fn shell_loop(mbi: u32) -> ! {
    vga::set_color(0x07);
    let mut line = [0u8; 64];
    loop {
        out_raw("os> ");
        let n = read_line(&mut line);
        let cmd = core::str::from_utf8(&line[..n]).unwrap_or("");
        let mut parts = cmd.split_ascii_whitespace();
        match parts.next().unwrap_or("") {
            "os" => {
                let t1 = parts.next().unwrap_or("");
                let t2 = parts.next().unwrap_or("");
                if t1 != "run" {
                    out_line("os   : unknown os subcommand (try: os run hermes)");
                } else if t2 == "hermes" {
                    out_line("os   : launching hermes (ring3) ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else if t2 == "threads" {
                    // M13: 同一镜像双任务, PIT 时间片轮转 (验证抢占调度)
                    crate::sched::set_multi();
                    out_line("os   : launching 2 tasks (timeslice round-robin) ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else if t2 == "ipc" {
                    // M18: IPC demo (管道 + 共享内存 + 信号)
                    crate::sched::set_multi();
                    out_line("os   : launching IPC demo (2 tasks: pipe/shm/sig) ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else if t2 == "kobj" {
                    // M19: 内核对象表 demo
                    out_line("os   : launching kobj table demo ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else if t2 == "crash" {
                    // M20: 用户态异常隔离 (A 存活 / B ud2 崩溃)
                    crate::sched::set_multi();
                    out_line("os   : launching exc-isolation demo (A vs B#UD) ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else if t2 == "stress" {
                    // M20: 资源压力/泄漏检测 (管道×128 + kobj×512)
                    out_line("os   : launching leak-stress demo ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else if t2 == "m21" {
                    // M21: linuxsubsys syscall 面 (~20 个)
                    out_line("os   : launching syscall-surface demo ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else if t2 == "fork" {
                    // M22: fork 克隆 (父/子共享地址空间, 用户栈物理拷贝)
                    out_line("os   : launching fork demo ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else if t2 == "busybox" {
                    // M23: 静态 busybox (argc/argv 栈帧); 额外词 -> argv[1..]
                    set_argv_mode(true);
                    set_argv0("busybox");
                    let mut words: [&str; 8] = [""; 8];
                    let mut wn = 0usize;
                    while wn < 8 {
                        match parts.next() {
                            Some(w) => {
                                words[wn] = w;
                                wn += 1;
                            }
                            None => break,
                        }
                    }
                    set_argv_cmd(&words[..wn]);
                    out_line("os   : launching busybox (argv mode) ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else if t2 == "win" {
                    // M26: winsubsys PE32+ (kernel32 垫片家族)
                    out_line("os   : launching winsubsys PE demo ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else {
                    // W15/W16: 通用注册表应用 (多模块镜像的 2..n 项; lib_find), 剩余词为 argv
                    match crate::vfs::lib_find(t2) {
                        Some((addr, len)) => {
                            set_argv_mode(true);
                            set_argv0(t2);
                            let mut words: [&str; 8] = [""; 8];
                            let mut wn = 0usize;
                            while wn < 8 {
                                match parts.next() {
                                    Some(w) => {
                                        words[wn] = w;
                                        wn += 1;
                                    }
                                    None => break,
                                }
                            }
                            set_argv_cmd(&words[..wn]);
                            out_line("os   : launching registry app '");
                            out_raw(t2);
                            out_line("' ...");
                            syscall::set_module_override(addr as u32, len as u32, t2);
                            syscall::enter_user_test(mbi); // > !: 不再返回
                        }
                        None => {
                            out_line("os   : unknown app (try: app list)");
                        }
                    }
                }
            }
            "app" => {
                let t1 = parts.next().unwrap_or("");
                if t1 == "list" {
                    out_line("os   : app registry:");
                    // 内核直读注册表 (LIBS 私有; 经 vfs 访问器)
                    if crate::vfs::lib_count() == 0 {
                        out_line("os   :   (none - single-module image)");
                    } else {
                        for i in 0..crate::vfs::lib_count() {
                            out_raw("os   :     ");
                            out_raw(crate::vfs::lib_name_at(i));
                            out_raw(" @ 0x");
                            out_raw(core::str::from_utf8(hex_str(crate::vfs::lib_addr_at(i))).unwrap_or("?"));
                            out_line("");
                        }
                    }
                } else {
                    out_line("os   : app subcommands: list");
                }
            }
            "ls" => {
                out_line("os   : /boot/module  /proc/meminfo  /dev/tty  /dev/model0");
                for i in 0..crate::vfs::tmpfs_count() {
                    out_raw("os   : /tmp/");
                    out_raw(crate::vfs::tmpfs_name(i));
                    out_line("");
                }
            }
            "cat" => {
                let name = parts.next().unwrap_or("");
                let fd = crate::vfs::fujo_open_name(name, 0);
                if fd < 0 {
                    out_line("os   : cat: no such file");
                } else {
                    let mut buf = [0u8; 256];
                    loop {
                        let k = crate::vfs::read_kernel(fd as u64, &mut buf);
                        if k == 0 {
                            break;
                        }
                        out_raw(core::str::from_utf8(&buf[..k]).unwrap_or("<bin>"));
                    }
                    crate::vfs::fujo_close(fd as u64);
                    out_line("");
                }
            }
            "mbuild" => {
                // W16: 自托管编译链一步命令: 写 hello.c (单文件; tcc 无 GOT 需同编译单元)
                // -> 启动 tcc 编译 (argv 内核构造) -> runfile 运行
                // W21: mbuild [path] —— 带路径参数时为 W21 HTTP clone 闭环:
                // 源码已由 m139 拉到 (tmpfs/磁盘), 不覆盖, 直接走 tcc 编译该文件。
                const SRC: &str = "typedef long i64;\n\
                static i64 sy4(i64 n, i64 a, i64 b, i64 c) {\n\
                \x20 i64 r;\n\
                \x20 asm volatile(\"syscall\" : \"=a\"(r) : \"a\"(n), \"D\"(a), \"S\"(b), \"d\"(c) : \"rcx\", \"r11\", \"memory\");\n\
                \x20 return r;\n\
                }\n\
                static const char MSG[] = \"tcc-compiled hello from fujo!\\n\";\n\
                void _start(void) {\n\
                \x20 sy4(1, 1, (i64)MSG, sizeof(MSG) - 1);\n\
                \x20 for (;;) {}\n\
                }\n";
                let src_path = parts.next().unwrap_or("/tmp/hello.c");
                if src_path == "/tmp/hello.c" {
                    let w1 = crate::vfs::write_kernel_file("/tmp/hello.c", SRC.as_bytes());
                    if w1 <= 0 {
                        out_line("os   : mbuild write fail");
                        continue;
                    }
                }
                match crate::vfs::lib_find("tcc-static") {
                    Some((addr, len)) => {
                        set_argv_mode(true);
                        set_argv0("tcc-static");
                        let args: [&str; 5] = [
                            "-nostdlib", "-static", "-o", "/tmp/hello", src_path,
                        ];
                        set_argv_cmd(&args[..]);
                        out_line("os   : launching tcc-static (compile hello) ...");
                        syscall::set_module_override(addr as u32, len as u32, "tcc-static");
                        syscall::enter_user_test(mbi); // > !: 不再返回
                    }
                    None => {
                        out_line("os   : mbuild: tcc-static not registered");
                    }
                }
            }
            "runfile" => {
                // W16: 从文件系统运行 ELF (tcc 编译产物; 载入帧区 -> override -> enter_user_test)
                let name = parts.next().unwrap_or("");
                let fd = crate::vfs::fujo_open_name(name, 0);
                if fd < 0 {
                    out_line("os   : runfile: no such file");
                } else {
                    let size = crate::vfs::fujo_size(fd as u64);
                    if size <= 0 {
                        out_line("os   : runfile: empty");
                        crate::vfs::fujo_close(fd as u64);
                    } else {
                        let n = ((size as usize + 0xFFF) / 0x1000).max(1);
                        match crate::mem::alloc_frames_kernel(n) {
                            Some(phys) => {
                                let mut buf = [0u8; 256];
                                let mut off = 0usize;
                                let p = phys as *mut u8;
                                loop {
                                    let k = crate::vfs::read_kernel(fd as u64, &mut buf);
                                    if k == 0 {
                                        break;
                                    }
                                    unsafe {
                                        for i in 0..k {
                                            p.add(off + i).write(buf[i]);
                                        }
                                    }
                                    off += k;
                                }
                                crate::vfs::fujo_close(fd as u64);
                                // W16: rebase 装到 0x400000 (tcc 输出默认 0x200000; 现为 0x400000 系, delta=0)
                                match crate::elf_loader::load_elf_rebase(phys as u32, off as u32) {
                                    Ok(entry) => {
                                        out_line("os   : runfile entry=0x");
                                        out_raw(core::str::from_utf8(hex_str(entry)).unwrap_or("?"));
                                        out_line("");
                                        crate::syscall::run_user(entry); // > !: 不再返回
                                    }
                                    Err(e) => {
                                        out_line("os   : runfile: bad elf (");
                                        out_raw(e);
                                        out_line(")");
                                    }
                                }
                            }
                            None => {
                                crate::vfs::fujo_close(fd as u64);
                                out_line("os   : runfile: oom");
                            }
                        }
                    }
                }
            }
            "echo" => {
                let rest = cmd.strip_prefix("echo").unwrap_or("");
                out_raw(rest.trim_start());
                out_line("");
            }
            "help" | "?" => out_line("os   : commands: os run <app> | app list | ls | cat | echo | help"),
            "" => {}
            _ => out_line("os   : unknown command (try: help)"),
        }
    }
}
