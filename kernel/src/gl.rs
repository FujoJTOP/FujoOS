//! gl.rs — fujogl v0 (M55): OpenGL 1.x 子集, 软件光栅后端
//!
//! backbuffer 光栅化 (整数重心法三角形 + 矩形 + 线):
//!   0x6201 gl_clear(r,g,b)          清屏
//!   0x6202 gl_rect(x,y,w,h,r,g,b)   glRectf 等价
//!   0x6203 gl_tri(x0,y0,..,x2,y2,r,g,b) 光栅三角形
//!   0x6204 gl_line(x0,y0,x1,y1,r,g,b)    Bresenham 线
//!   0x6205 gl_pixel(x,y)            读回

use crate::font;

fn setp(x: u32, y: u32, col: u32) {
    if x >= font::fb_w() || y >= font::fb_h() {
        return;
    }
    unsafe {
        let p = (font::BACKBUFFER + ((y as u64) * font::fb_w() as u64 + x as u64) * 4) as *mut u32;
        p.write(col);
    }
}

fn rgb(r: u64, g: u64, b: u64) -> u32 {
    0xFF000000u32 | (((r & 0xFF) as u32) << 16) | (((g & 0xFF) as u32) << 8) | ((b & 0xFF) as u32)
}

/// 0x6201
pub fn fujo_gl_clear(r: u64, g: u64, b: u64) -> i64 {
    let c = rgb(r, g, b);
    let w = font::fb_w();
    let h = font::fb_h();
    for y in 0..h {
        for x in 0..w {
            unsafe {
                ((font::BACKBUFFER + ((y as u64) * w as u64 + x as u64) * 4) as *mut u32).write(c);
            }
        }
    }
    0
}

/// 0x6202
pub fn fujo_gl_rect(x: u64, y: u64, w: u64, h: u64, color: u64) -> i64 {
    let c = (color as u32) | 0xFF000000;
    for dy in 0..h {
        for dx in 0..w {
            setp(x as u32 + dx as u32, y as u32 + dy as u32, c);
        }
    }
    0
}

/// 0x6203: 三角形光栅 (verts ptr 6×u32: x0,y0,x1,y1,x2,y2 + 打包 color)。
pub fn fujo_gl_tri(verts: u64, color: u64) -> i64 {
    let v = verts as *const u32;
    let x0 = unsafe { v.add(0).read() as i64 };
    let y0 = unsafe { v.add(1).read() as i64 };
    let x1 = unsafe { v.add(2).read() as i64 };
    let y1 = unsafe { v.add(3).read() as i64 };
    let x2 = unsafe { v.add(4).read() as i64 };
    let y2 = unsafe { v.add(5).read() as i64 };
    let c = (color as u32) | 0xFF000000;
    let min_x = x0.min(x1).min(x2);
    let max_x = x0.max(x1).max(x2);
    let min_y = y0.min(y1).min(y2);
    let max_y = y0.max(y1).max(y2);

    let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    if area == 0 {
        return 0;
    }
    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let w0 = (x1 - x0) * (py - y0) - (y1 - y0) * (px - x0);
            let w1 = (x2 - x1) * (py - y1) - (y2 - y1) * (px - x1);
            let w2 = (x0 - x2) * (py - y2) - (y0 - y2) * (px - x2);
            let ok = if area > 0 {
                w0 >= 0 && w1 >= 0 && w2 >= 0
            } else {
                w0 <= 0 && w1 <= 0 && w2 <= 0
            };
            if ok {
                setp(px as u32, py as u32, c);
            }
        }
    }
    0
}

/// 0x6204: Bresenham 线 (color 打包)。
pub fn fujo_gl_line(x0: u64, y0: u64, x1: u64, y1: u64, color: u64) -> i64 {
    let c = (color as u32) | 0xFF000000;
    let mut x0 = x0 as i64;
    let mut y0 = y0 as i64;
    let x1 = x1 as i64;
    let y1 = y1 as i64;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        setp(x0 as u32, y0 as u32, c);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
    0
}

/// 0x6205: 读回.
pub fn fujo_gl_pixel(x: u64, y: u64) -> i64 {
    if x >= font::fb_w() as u64 || y >= font::fb_h() as u64 {
        return 0;
    }
    unsafe {
        let p = (font::BACKBUFFER + (y * font::fb_w() as u64 + x) * 4) as *const u32;
        p.read() as i64
    }
}
