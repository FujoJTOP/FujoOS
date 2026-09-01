//! gdi.rs — win32 GDI 字体兼容层 (M109)
//!
//! win32 二进制经 gdi32 垫片调用 CreateFontA/TextOutA 等; 本模块持有
//! 字体对象表 (用于 GetTextExtentPoint 的长度计算), TextOut 渲染到
//! backbuffer 用 font::draw_char (8x8 位图, Windows GDI 等宽风格)。
//!
//! ponytail: 字体句柄存 16 槽表 (CreateFont 只登记, 不解析 TTF);
//! 需要真字体解析时再挂 freetype 级路径。所有坐标以像素为单位
//! (GDI 的 Y 方向向下, 路径一致)。

use crate::font;

/// 字体对象表 (最大 16 个; 0 = 空槽)。
/// 每项 12 字节: [height, width, weight] (i32 像素 + 权重), 槽 0 保留
/// 为系统默认字体 (DEFAULT_GUI_FONT 等价)。
const MAX_FONT: usize = 16;
static mut FONTS: [u32; MAX_FONT] = [0; MAX_FONT];
static mut FONT_N: u32 = 1;
/// 当前 DC 的字体/文字色/背景色 (单 DC 模型, ponytail: 多 DC 需表)。
static mut CUR_FONT: u32 = 1;
static mut CUR_COLOR: u32 = 0x00_00_00; // 黑字 (RGB)
static mut CUR_BK: u32 = 0xFF_FF_FF; // 白底

/// CreateFontA/W: (height, width, weight) -> 字体句柄 (>=1) / 0 失败。
pub fn create_font(height: u32, _width: u32, _weight: u32) -> i64 {
    unsafe {
        for i in 1..MAX_FONT {
            if FONTS[i] == 0 {
                FONTS[i] = (height & 0xFF) | 1; // 标 1=用户字体
                FONT_N += 1;
                return i as i64;
            }
        }
        0
    }
}

/// DeleteObject(h) -> 0 (删除字体句柄)。
pub fn delete_object(h: u32) -> i64 {
    unsafe {
        if (h as usize) < MAX_FONT {
            FONTS[h as usize] = 0;
        }
    }
    0
}

/// SelectObject(hdc, h) -> 旧句柄 (GDI: 返回旧对象)。
pub fn select_object(h: u32) -> i64 {
    unsafe {
        let old = CUR_FONT;
        if (h as usize) < MAX_FONT && FONTS[h as usize] != 0 {
            CUR_FONT = h;
        }
        old as i64
    }
}

/// GetStockObject(id) -> 句柄 (默认字体 0x0F=DEFAULT_GUI_FONT -> 1)。
pub fn stock_object(id: u32) -> i64 {
    if id == 0x0F {
        return 1; // DEFAULT_GUI_FONT 代称
    }
    if id == 0x0A {
        return 1; // DEFAULT_FONT
    }
    0
}

/// SetTextColor(hdc, color) -> 旧色 (color = 0x00RRGGBB)。
pub fn set_text_color(color: u32) -> i64 {
    unsafe {
        let old = CUR_COLOR;
        CUR_COLOR = color & 0x00FF_FFFF;
        old as i64
    }
}

/// SetBkMode(hdc, mode) -> 旧模式 (TRANSPARENT=1 不画底色; OPAQUE=2 画)。
pub fn set_bk_mode(mode: u32) -> i64 {
    unsafe { let _ = mode; CUR_BK as i64 }
}

/// TextOutA/W(hdc, x, y, str, len): 渲染 str 到 backbuffer (8x8 字模)。
/// ponytail: 第 5 参 (len) 在 Win64 栈上, 蹦床不传; 内部按 NUL 终止
/// 计数 (字符串字面量调用时等价; 需定长时再用 GetTextExtent 语义扩展)。
pub fn text_out(x: u32, y: u32, strp: u64, _len: u32) -> i64 {
    let mut cx = x;
    let mut n = 0u32;
    unsafe {
        loop {
            let b = (strp as *const u8).add(n as usize).read();
            if b == 0 || n >= 512 {
                break;
            }
            if b >= 0x20 && b <= 0x7E {
                font::draw_char(cx, y, b, 1, CUR_COLOR);
            }
            cx += 8;
            n += 1;
        }
    }
    n as i64
}

/// GetTextExtentPointA(hdc, str, len, size_ptr): 写 (w, h) 到 *size_ptr。
/// 8x8 字模: w = len*8, h = 8 (height 项在当前字体, 简化固定 8)。
pub fn text_extent(strp: u64, len: u32, size_ptr: u64) -> i64 {
    unsafe {
        let p = size_ptr as *mut u32;
        p.write(len * 8); // cx
        p.add(1).write(8); // cy
    }
    0
}

/// GetDC(hwnd) -> 1 (单 DC 模型)。
pub fn get_dc(_hwnd: u32) -> i64 {
    1
}

/// ReleaseDC(hwnd, hdc) -> 1 (成功)。
pub fn release_dc(_hwnd: u32, _hdc: u32) -> i64 {
    1
}
