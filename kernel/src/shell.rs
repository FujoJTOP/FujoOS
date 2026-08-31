//! shell.rs — os shell v0 (M10.1): 启动 Logo 之后的内核命令行
//!
//! 命令:
//!   os run hermes   装载并启动 Hermes CLI (ring3, 模型调用通道)
//!   help / ?        命令列表
//! 无输入 10s 自动执行 `os run hermes` (演示友好; 日志标注 auto)。
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

/// 读取一行 (环形缓冲轮询, 无 hlt —— TCG 安全); 10s 无输入返回自动化命令。
fn read_line(buf: &mut [u8]) -> (usize, bool) {
    let mut n = 0usize;
    let t0 = interrupts::ticks();
    loop {
        while let Some(c) = kbd::try_poll() {
            match c {
                '\n' => {
                    out_raw("\n");
                    return (n, false);
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
        if interrupts::ticks().wrapping_sub(t0) > 1000 {
            let s = b"os run hermes";
            buf[..s.len()].copy_from_slice(s);
            return (s.len(), true);
        }
    }
}

pub fn shell(mbi: u32) -> ! {
    vga::set_color(0x07);
    out_line("");
    out_line("os   : fujo shell v0 - commands:");
    out_line("os   :   os run hermes    launch Hermes CLI (agent + qwen model call)");
    out_line("os   :   help             show this list");
    out_line("os   : (10s idle -> auto 'os run hermes')");
    let mut line = [0u8; 64];
    loop {
        out_raw("os> ");
        let (n, idle) = read_line(&mut line);
        let cmd = core::str::from_utf8(&line[..n]).unwrap_or("");
        let mut parts = cmd.split_ascii_whitespace();
        match parts.next().unwrap_or("") {
            "os" => {
                let t1 = parts.next().unwrap_or("");
                let t2 = parts.next().unwrap_or("");
                if t1 == "run" && t2 == "hermes" {
                    if idle {
                        out_line("os   : no input 10s - auto 'os run hermes'");
                    }
                    out_line("os   : launching hermes (ring3) ...");
                    syscall::enter_user_test(mbi); // > !: 不再返回
                } else {
                    out_line("os   : unknown os subcommand (try: os run hermes)");
                }
            }
            "help" | "?" => out_line("os   : commands: os run hermes | help"),
            "" => {}
            _ => out_line("os   : unknown command (try: os run hermes)"),
        }
    }
}
