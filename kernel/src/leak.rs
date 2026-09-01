//! leak.rs — M83: 内存泄漏检测 (分配器统计, 快照差分)
//!
//! 锚定: kobj 对象表 (M19) 计数 4 类 [file, pipe, shm, sig];
//! leak_begin 快照 → 用户分配/释放 → leak_end 差分:
//!   >0 = 未释放候选 (泄漏可检), ==0 = 无泄漏。
//! 接口: 0x7A01 leak_begin() / 0x7A02 leak_end(ptr) →
//!       u64×4: (d_total, allocs, frees, baseline_total) /
//!       0x7A03 leak_stats(ptr) → 当前计数 u64×4。

use crate::kobj;
use crate::serial;

static mut BASE: [u64; 4] = [0; 4];
static mut BASE_TOTAL: u64 = 0;
static mut ARMED: bool = false;

fn snap() -> [u64; 4] {
    let c = kobj::counts();
    [c[0] as u64, c[1] as u64, c[2] as u64, c[3] as u64]
}

/// 0x7A01
pub fn fujo_leak_begin() -> i64 {
    unsafe {
        let s = snap();
        BASE = s;
        BASE_TOTAL = s[0] + s[1] + s[2] + s[3];
        ARMED = true;
    }
    0
}

/// 0x7A02
pub fn fujo_leak_end(ptr: u64) -> i64 {
    unsafe {
        let s = snap();
        let total = s[0] + s[1] + s[2] + s[3];
        let allocs = total.saturating_sub(BASE_TOTAL);
        let frees = BASE_TOTAL.saturating_sub(total);
        let w = ptr as *mut u64;
        w.write(total.saturating_sub(BASE_TOTAL));
        w.add(1).write(allocs);
        w.add(2).write(frees);
        w.add(3).write(BASE_TOTAL);
        if total > BASE_TOTAL {
            serial::write_str("leak : delta +");
            crate::syscall::debug_dec(total - BASE_TOTAL);
            serial::write_line(" (unreleased slots)");
        } else if total < BASE_TOTAL {
            serial::write_str("leak : delta -");
            crate::syscall::debug_dec(BASE_TOTAL - total);
            serial::write_line(" (freed below baseline)");
        } else {
            serial::write_line("leak : balanced (no leak)");
        }
        ARMED = false;
    }
    0
}

/// 0x7A03: 当前计数。
pub fn fujo_leak_stats(ptr: u64) -> i64 {
    let s = snap();
    unsafe {
        let w = ptr as *mut u64;
        for i in 0..4 {
            w.add(i).write(s[i]);
        }
    }
    0
}

pub fn armed() -> bool {
    unsafe { ARMED }
}
