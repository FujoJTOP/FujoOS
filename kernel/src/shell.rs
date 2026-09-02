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

pub fn shell(mbi: u32) -> ! {
    vga::set_color(0x07);
    out_line("");
    out_line("os   : fujo shell v0 - commands:");
    out_line("os   :   os run <app>    launch app (hermes|threads|...; registry names too)");
    out_line("os   :   app list        show app registry");
    out_line("os   :   ls              list /boot /proc /dev /tmp");
    out_line("os   :   cat <path>      dump file (kernel VFS)");
    out_line("os   :   echo <text>     print text");
    out_line("os   :   help            show this list");
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
                    // W15: 通用注册表应用 (多模块镜像的 2..n 项; lib_find)
                    match crate::vfs::lib_find(t2) {
                        Some((addr, len)) => {
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
