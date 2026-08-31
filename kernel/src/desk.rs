//! desk.rs — 桌面环境 v0 (M46): 任务栏 + 开始菜单雏形
//!
//! backbuffer 合成: 桌面背景 + 底部 40px 任务栏(开始按钮图标+时钟位)
//! + 开始菜单框(点击后状态机)。fujo 原语:
//!   0x5B01 desk_init()            清屏+背景+任务栏
//!   0x5B02 desk_taskbar(text)     任务栏时钟/标题
//!   0x5B03 desk_start(x,y)        开始按钮命中 (x,y 屏幕坐标) -> 1 命中
//!   0x5B04 desk_menu(on)          菜单框渲染 (从 y=40 起 200x180)
//!   0x5B05 desk_pixel(x,y)        像素读回

use crate::font;
use crate::icon;

const TB_H: u32 = 40;
const MENU_W: u32 = 200;
const MENU_H: u32 = 180;

fn setp(x: u32, y: u32, col: u32) {
    if x >= font::fb_w() || y >= font::fb_h() {
        return;
    }
    unsafe {
        let p = (font::BACKBUFFER + ((y as u64) * font::fb_w() as u64 + x as u64) * 4) as *mut u32;
        p.write(col);
    }
}

fn readp(x: u32, y: u32) -> u32 {
    if x >= font::fb_w() || y >= font::fb_h() {
        return 0;
    }
    unsafe {
        let p = (font::BACKBUFFER + ((y as u64) * font::fb_w() as u64 + x as u64) * 4) as *const u32;
        p.read()
    }
}

/// 矩形填充。
fn fill(x: u32, y: u32, w: u32, h: u32, col: u32) {
    for dy in 0..h {
        for dx in 0..w {
            setp(x + dx, y + dy, col);
        }
    }
}

fn font_line(x: u32, y: u32, scale: u32, color: u32, text: &str) {
    for (i, b) in text.bytes().enumerate() {
        let g = font::GLYPHS[(b - 0x20) as usize];
        for gy in 0..5u32 {
            for gx in 0..7u32 {
                if (g[gy as usize] >> (6 - gx)) & 1 != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            setp(
                                x + i as u32 * 8 * scale + gx * scale + sx,
                                y + gy * scale + sy,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }
}

/// 0x5B01: 桌面初始化 (bg + 任务栏)。
pub fn fujo_desk_init() -> i64 {
    unsafe {
        let bg = icon::PAL[1];
        let tb = icon::PAL[2];
        fill(0, 0, font::fb_w(), font::fb_h(), bg);
        // 任务栏
        fill(0, font::fb_h() - TB_H, font::fb_w(), TB_H, tb);
        // 开始按钮 (60x36 方块 + logo 图标)
        fill(8, font::fb_h() - TB_H + 2, 56, 36, 0xFFFFFFFFu32);
        let _ = icon::fujo_icon_draw(10, font::fb_h() as u64 - TB_H as u64 + 4, 3, 2);
        crate::serial::write_line("desk : desktop + taskbar rendered");
    }
    0
}

/// 0x5B02: 任务栏时钟/标题。
pub fn fujo_desk_taskbar(text: u64) -> i64 {
    unsafe {
        let mut n = 0usize;
        let mut tb = [0u8; 48];
        while n < 47 {
            let b = (text as *const u8).add(n).read();
            if b == 0 {
                break;
            }
            tb[n] = b;
            n += 1;
        }
        let s = core::str::from_utf8(&tb[..n]).unwrap_or("");
        let color = icon::PAL[0];
        font_line(700, font::fb_h() - TB_H + 10, 2, color, s);
    }
    0
}

/// 0x5B03: 开始按钮命中。
pub fn fujo_desk_start(x: u64, y: u64) -> i64 {
    if y >= (font::fb_h() - TB_H) as u64 && x < 64 && y >= (font::fb_h() - 38) as u64 {
        1
    } else {
        0
    }
}

/// 0x5B04: 开始菜单框 (on=1 渲染)。
pub fn fujo_desk_menu(on: u64) -> i64 {
    if on == 0 {
        return 0;
    }
    unsafe {
        let surf = icon::PAL[7];
        let ink = icon::PAL[0];
        fill(8, 0, MENU_W, MENU_H, surf);
        // 边框
        for x in 0..MENU_W {
            setp(8 + x, 0, ink);
            setp(8 + x, MENU_H - 1, ink);
        }
        for y in 0..MENU_H {
            setp(8, 8 + y - (if y < 8 { y } else { 0 }), ink); // 简化: 左框
            setp(8 + MENU_W - 1, y, ink);
        }
        // 菜单项文本
        font_line(24, 8, 1, ink, "Programs");
        font_line(24, 28, 1, ink, "Files");
        font_line(24, 48, 1, ink, "Terminal");
        font_line(24, 68, 1, ink, "Shut Down");
    }
    0
}

/// 0x5B05: 像素读回。
pub fn fujo_desk_pixel(x: u64, y: u64) -> i64 {
    readp(x as u32, y as u32) as i64
}
