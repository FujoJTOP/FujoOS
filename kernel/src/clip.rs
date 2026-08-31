//! clip.rs — 剪贴板 + 拖放雏形 v0 (M43)
//!
//! 剪贴板: 内核 8KB 缓冲 + set/get/owner (0x5801..0x5803)。
//! 拖放: 会话 begin/move/drop (0x5804..0x5806) —— drop 时命中窗口
//! (wmsg 矩形表) → 队列投递 WM_DROPFILES (0x14, win, x, y, 载荷指针)。

use crate::mouse;
use crate::wmsg;

pub const CLIP_MAX: usize = 8192;
static mut CLIP: [u8; CLIP_MAX] = [0; CLIP_MAX];
static mut CLIP_LEN: usize = 0;

/// 0x5801: 剪贴板 set (ptr, len)。
pub fn fujo_clip_set(ptr: u64, len: u64) -> i64 {
    unsafe {
        let n = (len as usize).min(CLIP_MAX - 1);
        for k in 0..n {
            CLIP[k] = (ptr as *const u8).add(k).read();
        }
        CLIP[n] = 0;
        CLIP_LEN = n;
        n as i64
    }
}

/// 0x5802: 剪贴板 get (ptr, n) -> 拷贝 len。
pub fn fujo_clip_get(ptr: u64, n: u64) -> i64 {
    unsafe {
        let m = CLIP_LEN.min(n as usize);
        for k in 0..m {
            ((ptr + k as u64) as *mut u8).write(CLIP[k]);
        }
        ((ptr + m as u64) as *mut u8).write(0);
        CLIP_LEN as i64
    }
}

/// 0x5803: 当前长度。
pub fn fujo_clip_len() -> i64 {
    unsafe { CLIP_LEN as i64 }
}

/// 0x5804: 拖放 begin (win, x, y) — 会话起点。
pub fn fujo_dnd_begin(_win: u32, _x: u32, _y: u32) -> i64 {
    0
}

/// 0x5805: 拖放 move (tx, ty 当前拖点) — 命中预览; 返回命中窗口。
pub fn fujo_dnd_move(x: u32, y: u32) -> i64 {
    unsafe {
        let mut hit = 0xFFFF_FFFFu32;
        for i in 0..8usize {
            let (x0, y0, x1, y1, rid) = mouse::HIT_RECTS[i];
            if rid != 0xFFFF_FFFF && x >= x0 && x <= x1 && y >= y0 && y <= y1 {
                hit = rid;
                break;
            }
        }
        crate::serial::write_str("dnd : move(");
        crate::syscall::debug_dec(x as u64);
        crate::serial::write_str(",");
        crate::syscall::debug_dec(y as u64);
        crate::serial::write_str(") hit r0=(");
        crate::syscall::debug_dec(mouse::HIT_RECTS[0].0 as u64);
        crate::serial::write_str(",");
        crate::syscall::debug_dec(mouse::HIT_RECTS[0].1 as u64);
        crate::serial::write_str(",");
        crate::syscall::debug_dec(mouse::HIT_RECTS[0].2 as u64);
        crate::serial::write_str(",");
        crate::syscall::debug_dec(mouse::HIT_RECTS[0].3 as u64);
        crate::serial::write_str(",");
        crate::syscall::debug_dec(mouse::HIT_RECTS[0].4 as u64);
        crate::serial::write_line(")");
        hit as i64
    }
}

/// 0x5806: 拖放 drop (x, y, payload ptr) — 命中窗口 → WM_DROPFILES。
pub fn fujo_dnd_drop(x: u32, y: u32, payload: u64) -> i64 {
    unsafe {
        let mut hit = 0xFFFF_FFFFu32;
        for i in 0..8usize {
            let (x0, y0, x1, y1, rid) = mouse::HIT_RECTS[i];
            if rid != 0xFFFF_FFFF && x >= x0 && x <= x1 && y >= y0 && y <= y1 {
                hit = rid;
                break;
            }
        }
        if hit != 0xFFFF_FFFF {
            wmsg::push_external(0x14, hit, x, y, payload as u32);
            return hit as i64;
        }
        0
    }
}
