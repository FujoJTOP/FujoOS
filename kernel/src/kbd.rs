//! kbd.rs — PS/2 键盘驱动 + IRQ1 (M5 · 输入系统 v0)
//!
//! 流程: 8042 初始化(命令字节 bit0 使能键盘 IRQ)
//!       -> IRQ1 (IDT 0x21) 中断 -> 读 0x60 扫描码 -> 环形缓冲
//!       -> 主循环解码(Set 1 基本集) -> 显示(5x7 字体) + 串口日志
//!
//! 验证: QEMU monitor 'sendkey' 注入, 内核逐键打印并在终端窗绘制。

use crate::graphics;
use crate::interrupts;
use crate::serial;

const BUF_SIZE: usize = 256;
const TERM_X: u32 = 120;
const TERM_Y: u32 = 150;

static mut KBD_BUF: [u8; BUF_SIZE] = [0; BUF_SIZE];
static mut KBD_HEAD: usize = 0;
static mut KBD_TAIL: usize = 0;

/// 8042 使能键盘 IRQ (命令字节 bit0)。
pub fn init() {
    unsafe {
        let mut spin = 0u32;
        while serial::inb(0x64) & 2 != 0 {
            spin += 1;
            if spin > 100000 {
                break;
            }
        }
        serial::outb(0x64, 0x20); // 读命令字节
        let mut c = 0u32;
        while serial::inb(0x64) & 1 == 0 {
            c += 1;
            if c > 100000 {
                break;
            }
        }
        let cmd = serial::inb(0x60);
        let mut spin2 = 0u32;
        while serial::inb(0x64) & 2 != 0 {
            spin2 += 1;
            if spin2 > 100000 {
                break;
            }
        }
        serial::outb(0x64, 0x60);
        serial::outb(0x60, cmd & !0x01); // 先禁键盘 IRQ (bit0=0)
        // 清空 8042 输出缓冲 (丢弃 BAT/自检挂起字节)
        for _ in 0..3 {
            let mut w = 0u32;
            while serial::inb(0x64) & 1 == 0 {
                w += 1;
                if w > 10000 {
                    break;
                }
            }
            if serial::inb(0x64) & 1 != 0 {
                let _ = serial::inb(0x60);
            }
        }
        // 数据清空后再使能键盘 IRQ
        serial::outb(0x64, 0x60);
        serial::outb(0x60, cmd | 0x01);
        // 8042 初始化完成后才开放 IRQ1 (避免初始化期间中断风暴)
        serial::outb(0x21, 0xFD); // PIC: IRQ0 + IRQ1 unmasked
    }
    serial::write_line("kbd  : ps/2 keyboard IRQ1 armed (scancode set 1)");
}

/// IRQ1 中断处理 (asm stub 以符号 fujo_kbd_irq 调用; 最小化操作)。
#[no_mangle]
pub extern "C" fn fujo_kbd_irq() {
    unsafe {
        let sc = serial::inb(0x60);
        let t = KBD_TAIL;
        if (t + 1) % BUF_SIZE != KBD_HEAD {
            KBD_BUF[t] = sc;
            KBD_TAIL = (t + 1) % BUF_SIZE;
        }
        serial::outb(0x20, 0x20); // EOI master
    }
}

/// 扫描码 (Set 1 基本集, 无 E0/F0 前缀) -> ASCII 小写/符号。
fn decode(sc: u8) -> Option<char> {
    Some(match sc {
        0x1E => 'a',
        0x30 => 'b',
        0x2E => 'c',
        0x20 => 'd',
        0x12 => 'e',
        0x21 => 'f',
        0x22 => 'g',
        0x23 => 'h',
        0x17 => 'i',
        0x24 => 'j',
        0x25 => 'k',
        0x26 => 'l',
        0x32 => 'm',
        0x31 => 'n',
        0x18 => 'o',
        0x19 => 'p',
        0x10 => 'q',
        0x13 => 'r',
        0x1F => 's',
        0x14 => 't',
        0x16 => 'u',
        0x2F => 'v',
        0x11 => 'w',
        0x2D => 'x',
        0x15 => 'y',
        0x2C => 'z',
        0x02 => '1',
        0x03 => '2',
        0x04 => '3',
        0x05 => '4',
        0x06 => '5',
        0x07 => '6',
        0x08 => '7',
        0x09 => '8',
        0x0A => '9',
        0x0B => '0',
        0x1C => '\n',
        0x39 => ' ',
        0x0E => '\x08',
        0x3A | 0x2A | 0x36 => ' ',
        _ => return None,
    })
}

fn print_dec_usize(v: usize) {
    let mut buf = [0u8; 24];
    let mut i = 24;
    let mut x = v as u64;
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

fn print_hex64(v: u64) {
    let mut buf = [0u8; 16];
    for i in 0..16 {
        let d = ((v >> (4 * i)) & 0xF) as u8;
        buf[15 - i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
    }
    serial::write_str("0x");
    serial::write_str(core::str::from_utf8(&buf).unwrap());
}

/// 按键演示: 3 秒输入窗口(显示在终端窗), 每键串口日志 + 缓冲。
pub fn demo() {
    graphics::draw_str(
        TERM_X,
        TERM_Y,
        "FUJOS TERMINAL - TYPE KEYS (QEMU sendkey) -",
        0x9FE0A0,
        1,
    );
    let mut line = [0u8; 80];
    let mut used = 0usize;
    let t0 = interrupts::ticks();
    serial::write_str("kbd  : window start t0=");
    print_dec_usize(t0 as usize);
    serial::write_line("");

    let mut last_mark = t0;
    while interrupts::ticks() - t0 < 300 {
        crate::hlt();
        let now = interrupts::ticks();
        if now - last_mark >= 100 {
            last_mark = now;
            serial::write_str("kbd  : tick keepalive=");
            print_dec_usize(now as usize);
            serial::write_line("");
        }
        loop {
            let sc = unsafe {
                if KBD_TAIL == KBD_HEAD {
                    break;
                }
                let t = KBD_HEAD;
                KBD_HEAD = (t + 1) % BUF_SIZE;
                KBD_BUF[t]
            };
            match decode(sc) {
                Some(c) => {
                    let display_c = if c == '\n' {
                        '['
                    } else if c == '\x08' {
                        '<'
                    } else if c == ' ' {
                        '_'
                    } else {
                        c.to_ascii_uppercase()
                    };
                    if used < 78 {
                        line[used] = display_c as u8;
                        used += 1;
                    }
                    serial::write_str("key  : '");
                    let one = [display_c as u8];
                    serial::write_str(core::str::from_utf8(&one).unwrap_or("?"));
                    serial::write_line("'");
                    // 终端窗滚动显示
                    let row_y = TERM_Y + 24 + ((used as u32 / 26) * 16);
                    graphics::draw_str(
                        TERM_X,
                        row_y,
                        core::str::from_utf8(&line).unwrap_or("?"),
                        0xD0E0FF,
                        1,
                    );
                }
                None => {}
            }
        }
    }

    // 汇总
    serial::write_str("kbd  : captured ");
    print_dec_usize(used);
    serial::write_line(" keys");
    if used > 0 {
        graphics::draw_str(TERM_X, TERM_Y + 100, "KEYS:", 0xFFE080, 1);
        graphics::draw_str(
            TERM_X + 66,
            TERM_Y + 100,
            core::str::from_utf8(&line).unwrap_or(""),
            0xFFE080,
            1,
        );
    }
    let sum = graphics::frame_checksum();
    serial::write_str("kbd  : frame checksum=");
    print_hex64(sum);
    serial::write_line("");
}
