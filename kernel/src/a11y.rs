//! a11y.rs — 无障碍 (M49): 高对比 / 大字模式
//!
//! 0x5D01 a11y_set(mode): 0=正常 1=高对比 2=大字 3=高对比+大字
//! 0x5D02 a11y_get() -> mode
//! 高对比: 调色板 fg/bg 反转 (icon::PAL[0/1])。
//! 大字: font_text 渲染自动 +1 scale (font::fujo_font_text 查询)。

use crate::icon;

static mut A11Y_MODE: u64 = 0;

pub fn mode() -> u64 {
    unsafe { A11Y_MODE }
}

/// 高对比颜色应用 (或恢复正常)。
fn apply_contrast(on: bool) {
    unsafe {
        if on {
            icon::PAL[0] = 0xFFFFFFFFu32; // fg -> 白
            icon::PAL[1] = 0xFF000000u32; // bg -> 黑
            icon::PAL[7] = 0xFF111111u32; // surface -> 深
            icon::PAL[8] = 0xFFFFFFFFu32; // ink -> 白
        } else {
            icon::PAL[0] = 0xFF202020u32;
            icon::PAL[1] = 0xFFFFFFFFu32;
            icon::PAL[7] = 0xFFE5E7EBu32;
            icon::PAL[8] = 0xFF0F172Au32;
        }
    }
}

/// 0x5D01: 设置辅助模式。
pub fn fujo_a11y_set(m: u64) -> i64 {
    unsafe {
        A11Y_MODE = m;
        apply_contrast(m == 1 || m == 3);
        crate::serial::write_line("a11y : mode applied");
    }
    0
}

/// 0x5D02: 当前模式。
pub fn fujo_a11y_get() -> i64 {
    unsafe { A11Y_MODE as i64 }
}

/// 大字 boost (font 查询; 0 或 1)。
pub fn scale_boost() -> u32 {
    unsafe {
        if A11Y_MODE == 2 || A11Y_MODE == 3 {
            1
        } else {
            0
        }
    }
}
