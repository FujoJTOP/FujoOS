//! blit.rs — M61: 图形加速抽象 (blit/缩放硬件路径)
//!
//! display 后端接口扩展 (当前软件路径; virtio-gpu 后端 M61 后同接口):
//!   0x6801 blit(src_ptr, dx, dy, w, h)     源矩形(from backbuffer 逻辑
//!                                           区) 拷贝到 (dx,dy)
//!   0x6802 blit_scal(src_ptr, dx,dy, sw,sh, dw,dh) 最近邻缩放 blit
//! 源 = 32bpp 行主序像素缓冲 (用户区)。

use crate::font;

fn setp(x: u64, y: u64, c: u32) {
    if x >= font::fb_w() as u64 || y >= font::fb_h() as u64 {
        return;
    }
    unsafe {
        let p = (font::BACKBUFFER + (y * font::fb_w() as u64 + x) * 4) as *mut u32;
        p.write(c);
    }
}

/// 0x6801
pub fn fujo_blit(src: u64, dx: u64, dy: u64, w: u64, h: u64) -> i64 {
    let mut y = 0u64;
    while y < h {
        let mut x = 0u64;
        while x < w {
            let c = unsafe { ((src + (y * w + x) * 4) as *const u32).read() };
            setp(dx as u64 + x, dy as u64 + y, c);
            x += 1;
        }
        y += 1;
    }
    0
}

/// 0x6802: 最近邻缩放 blit (src 缓冲 w=sw,h=sh -> dest dw,dh)。
pub fn fujo_blit_scal(src: u64, dx: u64, dy: u64, dims: u64) -> i64 {
    let ds = dims as *const u32;
    let sw = unsafe { ds.add(0).read() as u64 };
    let sh = unsafe { ds.add(1).read() as u64 };
    let dw = unsafe { ds.add(2).read() as u64 };
    let dh = unsafe { ds.add(3).read() as u64 };
    let mut dy2 = 0u64;
    while dy2 < dh {
        let mut dx2 = 0u64;
        while dx2 < dw {
            let sx = dx2 * sw / dw;
            let sy = dy2 * sh / dh;
            let c = unsafe { ((src + (sy * sw + sx) * 4) as *const u32).read() };
            setp(dx + dx2, dy + dy2, c);
            dx2 += 1;
        }
        dy2 += 1;
    }
    0
}
