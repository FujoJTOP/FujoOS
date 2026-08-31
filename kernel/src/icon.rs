//! icon.rs — 调色板/主题/图标系统 (M44)
//!
//! 调色板: 16 槽 ARGB 颜色表 (0x5901 get / 0x5902 set)。
//! 主题: 2 内置 (LIGHT=0 / DARK=1, 0x5903 apply) —— 重载槽 0..15。
//! 图标: 内置 8x8 位图 (file/folder/app, 0x5904 draw x,y,id,scale) +
//!       像素读回 (0x5905)。
//! 渲染: RAM backbuffer (0xC00000, 同 font.rs)。

use crate::font;
use crate::serial;

/// 调色板槽 (默认 LIGHT)
pub static mut PAL: [u32; 16] = [
    0xFFFFFFFF, // 0 fg
    0xFF202020, // 1 bg
    0xFF1CAA5E, // 2 accent green
    0xFF3B82F6, // 3 accent blue
    0xFFF87171, // 4 accent red
    0xFFFBBF24, // 5 accent amber
    0xFF94A3B8, // 6 dim
    0xFFE5E7EB, // 7 surface
    0xFF0F172A, // 8 ink
    0xFF22D3EE, // 9 cyan
    0xFFA78BFA, // 10 violet
    0xFFFB923C, // 11 orange
    0xFF4ADE80, // 12 green2
    0xFFF472B6, // 13 pink
    0xFF64748B, // 14 gray
    0xFF111827, // 15 deep
];

/// 图标位图 (8x8, 每行 2 字节 = 16bit 高字节左)
const ICON_FILE: [u16; 8] = [
    0xF000, 0x8800, 0x8800, 0x8800, 0x8800, 0x8800, 0x8800, 0xF000,
];
const ICON_FOLDER: [u16; 8] = [
    0xF000, 0xF000, 0x0800, 0x0800, 0x0800, 0x0800, 0x0800, 0xF800,
];
const ICON_APP: [u16; 8] = [
    0xF900, 0x8900, 0x8900, 0x8900, 0x8900, 0x8900, 0x8900, 0xF900,
];

fn icondata(id: u64) -> Option<&'static [u16; 8]> {
    match id {
        1 => Some(&ICON_FILE),
        2 => Some(&ICON_FOLDER),
        3 => Some(&ICON_APP),
        _ => None,
    }
}

fn setp(x: u32, y: u32, col: u32) {
    if x >= font::FB_W || y >= font::FB_H {
        return;
    }
    unsafe {
        let p = (font::BACKBUFFER + ((y as u64) * font::FB_W as u64 + x as u64) * 4) as *mut u32;
        p.write(col);
    }
}

fn readp(x: u32, y: u32) -> u32 {
    if x >= font::FB_W || y >= font::FB_H {
        return 0;
    }
    unsafe {
        let p = (font::BACKBUFFER + ((y as u64) * font::FB_W as u64 + x as u64) * 4) as *const u32;
        p.read()
    }
}

/// 0x5901: 调色板 get(idx) -> ARGB。
pub fn fujo_pal_get(idx: u64) -> i64 {
    unsafe { PAL[(idx % 16) as usize] as i64 }
}

/// 0x5902: 调色板 set(idx, color)。
pub fn fujo_pal_set(idx: u64, color: u64) -> i64 {
    unsafe {
        PAL[(idx % 16) as usize] = color as u32;
    }
    0
}

/// 0x5903: 主题 apply (0=LIGHT, 1=DARK)。
pub fn fujo_theme_apply(id: u64) -> i64 {
    unsafe {
        if id == 1 {
            PAL[0] = 0xFFE5E7EB; // fg
            PAL[1] = 0xFF0B0F1A; // bg
            PAL[6] = 0xFF475569; // dim
            PAL[7] = 0xFF1E293B; // surface
            PAL[8] = 0xFFF8FAFC; // ink
        } else {
            PAL[0] = 0xFF202020;
            PAL[1] = 0xFFFFFFFF;
            PAL[6] = 0xFF94A3B8;
            PAL[7] = 0xFFE5E7EB;
            PAL[8] = 0xFF0F172A;
        }
    }
    serial::write_line("icon : theme applied");
    0
}

/// 0x5904: 图标绘制 (x, y, id 1..=3, scale 1..=4)。
pub fn fujo_icon_draw(x: u64, y: u64, id: u64, scale: u64) -> i64 {
    unsafe {
        let g = match icondata(id) {
            Some(g) => g,
            None => return -22,
        };
        let scale = scale.clamp(1, 4) as u32;
        let ink = PAL[0];
        for gy in 0..8u32 {
            for gx in 0..8u32 {
                let on = (g[gy as usize] >> (15 - gx)) & 1 != 0;
                if on {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            setp(
                                x as u32 + gx * scale as u32 + sx,
                                y as u32 + gy * scale as u32 + sy,
                                ink,
                            );
                        }
                    }
                }
            }
        }
    }
    0
}

/// 0x5905: 像素读回 (验证)。
pub fn fujo_icon_pixel(x: u64, y: u64) -> i64 {
    readp(x as u32, y as u32) as i64
}
