//! term.rs — 终端窗口控件 (M45): 80x25 文本屏 + GUI 渲染
//!
//! 内核镜像 user_write 的文本入环形屏幕 (term_feed), 控件把屏渲染到
//! backbuffer (font + palette): 0x5A01 term_put(x,y,ch,color) /
//! 0x5A02 term_draw(x,y,scale) / 0x5A03 term_pixel(x,y)。

use crate::font;
use crate::icon;

pub const TW: u32 = 80;
pub const TH: u32 = 25;
const SCR_MAX: usize = 2560;

static mut SCR: [u16; SCR_MAX] = [0x0F20; SCR_MAX]; // 0x20 = 空格, 低8=char 高8=color
static mut SCR_ROW: u32 = 0;
static mut SCR_COL: u32 = 0;

/// user_write 镜像: 逐字符进屏 (终端窗口跟随输出)。
pub fn term_feed(bytes: &[u8]) {
    unsafe {
        for &b in bytes {
            if b == b'\n' {
                SCR_ROW += 1;
                SCR_COL = 0;
                if SCR_ROW >= TH {
                    // 上滚: 整体上移一行
                    for r in 0..(TH - 1) as usize {
                        for c in 0..TW as usize {
                            SCR[r * TW as usize + c] = SCR[(r + 1) * TW as usize + c];
                        }
                    }
                    for c in 0..TW as usize {
                        SCR[(TH - 1) as usize * TW as usize + c] = 0x0F20;
                    }
                    SCR_ROW = TH - 1;
                }
            } else if b == b'\r' {
                SCR_COL = 0;
            } else if b >= 32 || b == 8 {
                if b == 8 {
                    if SCR_COL > 0 {
                        SCR_COL -= 1;
                    }
                } else {
                    let idx = (SCR_ROW as usize) * TW as usize + SCR_COL as usize;
                    if idx < SCR_MAX {
                        SCR[idx] = 0x0F00 | b as u16;
                    }
                    SCR_COL += 1;
                    if SCR_COL >= TW {
                        SCR_COL = 0;
                        SCR_ROW += 1;
                        if SCR_ROW >= TH {
                            SCR_ROW = TH - 1;
                        }
                    }
                }
            }
        }
    }
}

/// 0x5A01: 直接写屏 (x, y 行, ch, color 低 8)。
pub fn fujo_term_put(x: u64, y: u64, ch: u64, color: u64) -> i64 {
    unsafe {
        if x < TW as u64 && y < TH as u64 {
            SCR[(y as usize) * TW as usize + x as usize] =
                ((color as u16 & 0x0F) << 8) | (ch as u16 & 0xFF);
            return 0;
        }
    }
    -22
}

/// 0x5A02: 渲染整屏 (ox, oy 原点, scale) -> backbuffer。
pub fn fujo_term_draw(ox: u64, oy: u64, scale: u64) -> i64 {
    let scale = scale.clamp(1, 3) as u32;
    unsafe {
        let mut count = 0i64;
        for y in 0..TH {
            for x in 0..TW {
                let e = SCR[(y as usize) * TW as usize + x as usize];
                let ch = (e & 0xFF) as u8;
                let color = palette_to_u32((e >> 8) & 0x0F);
                let px = ox as u32 + x * 8 * scale;
                let py = oy as u32 + y * 5 * scale;
                // 用 font 渲染 (整字符块画, 无背景隔断)
                if ch >= 0x20 {
                    draw_char_block(px, py, ch, scale, color);
                    count += 1;
                }
            }
        }
        crate::serial::write_line("term : screen rendered to backbuffer");
        crate::serial::write_str("term : scr0=");
        crate::syscall::debug_dec((SCR[0] as u64) & 0xFF);
        crate::serial::write_str(" scr1=");
        crate::syscall::debug_dec((SCR[1] as u64) & 0xFF);
        crate::serial::write_line("");
        count
    }
}

fn palette_to_u32(c: u16) -> u32 {
    // VGA 颜色索引 -> ARGB (粗略)
    const VGA: [u32; 16] = [
        0xFF000000, 0xFF0000AA, 0xFF00AA00, 0xFF00AAAA, 0xFFAA0000, 0xFFAA00AA,
        0xFFAA5500, 0xFFAAAAAA, 0xFF555555, 0xFF5555FF, 0xFF55FF55, 0xFF55FFFF,
        0xFFFF5555, 0xFFFF55FF, 0xFFFFFF55, 0xFFFFFFFF,
    ];
    VGA[(c & 0x0F) as usize]
}

/// 渲染单字符 (8x8·scale, 用 font GLYPHS —— VGA 8x8 字模 bit7..0)。
fn draw_char_block(x: u32, y: u32, ch: u8, scale: u32, color: u32) {
    if ch < 0x20 {
        return;
    }
    let g = font::GLYPHS[(ch - 0x20) as usize];
    for gy in 0..8u32 {
        for gx in 0..8u32 {
            if (g[gy as usize] >> (7 - gx)) & 1 != 0 {
                for sy in 0..scale {
                    for sx in 0..scale {
                        setp(x + gx * scale + sx, y + gy * scale + sy, color);
                    }
                }
            }
        }
    }
}

fn setp(x: u32, y: u32, col: u32) {
    if x >= font::fb_w() || y >= font::fb_h() {
        return;
    }
    unsafe {
        let p = (font::BACKBUFFER + ((y as u64) * font::fb_w() as u64 + x as u64) * 4) as *mut u32;
        p.write(col);
    }
}

/// 0x5A03: 像素读回。
pub fn fujo_term_pixel(x: u64, y: u64) -> i64 {
    if x >= font::fb_w() as u64 || y >= font::fb_h() as u64 {
        return 0;
    }
    unsafe {
        let p = (font::BACKBUFFER + (y * font::fb_w() as u64 + x) * 4) as *const u32;
        p.read() as i64
    }
}
