//! wmsg.rs — win32k 等价消息环 v0 (M37)
//!
//! 内核侧窗口/消息抽象 (用户态 GUI 程序的 Win32 模式骨架):
//!   - 窗口类注册 (class -> id)
//!   - 窗口创建 (class, x, y, w, h -> win id; z-order 尾=顶层)
//!   - 环形消息队列 (非阻塞 getmsg)
//!   - 鼠标事件 -> 消息 (WM_MOUSEMOVE/WM_ENTER/WM_LEAVE/WM_BUTTON)
//!   - 置顶/移除 (z-order 调整)
//! fujo 原生: 0x5520 class / 0x5521 create / 0x5522 getmsg /
//!             0x5523 top / 0x5524 remove
//! 注: 所有表用哨兵扫描 (id==0/0xFFFFFFFF 为空位), 不依赖计数静态。

use crate::mouse;
use crate::serial;

pub const WMAX: usize = 8;
pub const QLEN: usize = 64;

// 消息种类
pub const WM_MOUSEMOVE: u32 = 0x01;
pub const WM_ENTER: u32 = 0x02;
pub const WM_LEAVE: u32 = 0x03;
pub const WM_BUTTON: u32 = 0x04;
pub const WM_CREATE: u32 = 0x10;
pub const WM_DESTROY: u32 = 0x11;
pub const WM_ZORDER: u32 = 0x12;

/// 类表: (name, id); id=0 为空位。
static mut CLASSES: [([u8; 16], u32); 8] = [([0; 16], 0); 8];
/// win 表: (class_id, x, y, w, h, flags), win_id — 表序 = z (尾=顶层);
/// win_id=0xFFFFFFFF 为空位。
static mut WINS: [([u32; 6], u32); WMAX] = [([0; 6], 0xFFFF_FFFF); WMAX];
/// 消息队列: (kind, win, a, b, c)
static mut Q: [(u32, u32, u32, u32, u32); QLEN] = [(0, 0, 0, 0, 0); QLEN];
static mut QRD: usize = 0;
static mut QWR: usize = 0;

fn push_msg(kind: u32, win: u32, a: u32, b: u32, c: u32) {
    unsafe {
        if (QWR + 1) % QLEN == QRD {
            return; // 满: 丢弃 (v0)
        }
        Q[QWR] = (kind, win, a, b, c);
        QWR = (QWR + 1) % QLEN;
    }
}

/// 0x5520: 注册窗口类 (name ptr) -> class_id (1 起)。
pub fn fujo_wm_class(name: u64) -> i64 {
    unsafe {
        let mut nb = [0u8; 16];
        let mut n = 0usize;
        while n < 15 {
            let b = (name as *const u8).add(n).read();
            if b == 0 {
                break;
            }
            nb[n] = b;
            n += 1;
        }
        // 查重
        for i in 0..8usize {
            if CLASSES[i].1 != 0 && CLASSES[i].0 == nb {
                return CLASSES[i].1 as i64;
            }
        }
        // 空位
        for i in 0..8usize {
            if CLASSES[i].1 == 0 {
                let id = i as u32 + 1;
                CLASSES[i] = (nb, id);
                serial::write_line("wm   : class registered");
                return id as i64;
            }
        }
        -12
    }
}

/// 从 WINS 表整体重建鼠标命中矩形 (create/remove/top 后调用)。
fn refresh_rects() {
    unsafe {
        let mut rects = [0u32; 8 * 5];
        let mut n = 0usize;
        for k in 0..WMAX {
            if WINS[k].1 != 0xFFFF_FFFF && n < 8 {
                rects[n * 5] = WINS[k].0[1];
                rects[n * 5 + 1] = WINS[k].0[2];
                rects[n * 5 + 2] = WINS[k].0[3];
                rects[n * 5 + 3] = WINS[k].0[4];
                rects[n * 5 + 4] = WINS[k].1;
                n += 1;
            }
        }
        let _ = mouse::fujo_mouse_rects(rects.as_ptr() as u64, n as u64);
    }
}

/// 0x5521: 创建窗口 (class_id, x, y, w, h) -> win_id (哨兵扫描空位)。
pub fn fujo_wm_create(class_id: u32, x: u32, y: u32, w: u32, h: u32) -> i64 {
    unsafe {
        let mut ok_cls = false;
        for i in 0..8usize {
            if CLASSES[i].1 == class_id {
                ok_cls = true;
                break;
            }
        }
        if !ok_cls {
            return -22;
        }
        let mut slot = None;
        for i in 0..WMAX {
            if WINS[i].1 == 0xFFFF_FFFF {
                slot = Some(i);
                break;
            }
        }
        let slot = match slot {
            Some(s) => s,
            None => return -12,
        };
        let wid = slot as u32 + 1; // v0: win id = 槽+1 (简单; M38 起独立序列)
        WINS[slot] = ([class_id, x, y, w, h, 0], wid);
        refresh_rects(); // M37: 全表重建 (create 单独注册会互相覆盖, 实证)
        push_msg(WM_CREATE, wid, x, y, w);
        serial::write_line("wm   : window created");
        wid as i64
    }
}

/// 0x5522: 取消息 -> 队列？写 5×u32 (kind, win, a, b, c), 返回 1/0。
pub fn fujo_wm_getmsg(ptr: u64) -> i64 {
    unsafe {
        if QRD == QWR {
            return 0;
        }
        let (k, w, a, b, c) = Q[QRD];
        QRD = (QRD + 1) % QLEN;
        let p = ptr as *mut u32;
        p.add(0).write(k);
        p.add(1).write(w);
        p.add(2).write(a);
        p.add(3).write(b);
        p.add(4).write(c);
    }
    1
}

/// 0x5523: 置顶 (z-order 调整: 移到表尾)。
pub fn fujo_wm_top(win: u32) -> i64 {
    unsafe {
        let mut idx = None;
        let mut n = 0usize;
        for i in 0..WMAX {
            if WINS[i].1 != 0xFFFF_FFFF {
                n = i + 1;
            }
            if WINS[i].1 == win {
                idx = Some(i);
            }
        }
        if let Some(i) = idx {
            let save = WINS[i];
            for k in i..(n - 1) {
                WINS[k] = WINS[k + 1];
            }
            WINS[n - 1] = save;
            push_msg(WM_ZORDER, win, 0, 0, 0);
            return 0;
        }
        -2
    }
}

/// 0x5524: 移除窗口 (关闭; 释放槽; 鼠标矩形表整体重建)。
pub fn fujo_wm_remove(win: u32) -> i64 {
    unsafe {
        let mut idx = None;
        for i in 0..WMAX {
            if WINS[i].1 == win {
                idx = Some(i);
                break;
            }
        }
        if let Some(i) = idx {
            WINS[i] = ([0; 6], 0xFFFF_FFFF);
            push_msg(WM_DESTROY, win, 0, 0, 0);
            refresh_rects(); // 重建鼠标矩形表
            return 0;
        }
        -2
    }
}

/// 位置消息投递 (mouse.rs: 每包)。
pub fn notify_moved(x: u32, y: u32, btn: u32, focus: u32) {
    if focus != 0xFFFF_FFFF {
        push_msg(WM_MOUSEMOVE, focus, x, y, btn);
    }
}

pub fn notify_focus_changed(old: u32, new: u32) {
    if old != 0xFFFF_FFFF && old != new {
        push_msg(WM_LEAVE, old, 0, 0, 0);
    }
    if new != 0xFFFF_FFFF && old != new {
        push_msg(WM_ENTER, new, 0, 0, 0);
    }
}

pub fn notify_button(x: u32, y: u32, btn: u32, focus: u32) {
    if focus != 0xFFFF_FFFF {
        push_msg(WM_BUTTON, focus, x, y, btn);
    }
}
