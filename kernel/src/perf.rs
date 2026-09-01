//! perf.rs — M68: 帧时间表/性能计数器工具 v0
//!
//! 帧时间表: 用户程序在两帧边界调 perf_frame_mark, 内核记 interval
//! (µs, 经 timer 校准), 环形保存最近 64 项 + 汇总 (avg/max/sum)。
//! 性能计数器: 8 个 u64 槽, 内核侧挂钩: 0=PIT IRQ, 1=syscall,
//! 2=ctx-switch (sched/irq/syscall 各 bump 一行)。
//!
//! 接口: 0x6E01 perf_frame_mark() / 0x6E02 perf_frame_stats(ptr) /
//!       0x6E03 perf_counter_enable(id,on) / 0x6E04 perf_counter_read(ptr)

use crate::timer;

const FTAB: usize = 64;

static mut F_LAST: u64 = 0; // µs
static mut F_N: usize = 0;
static mut F_IDX: usize = 0;
static mut F_TAB: [u64; FTAB] = [0; FTAB];
static mut F_SUM: u64 = 0;
static mut F_MAX: u64 = 0;

static mut CTR_ON: [bool; 8] = [false; 8];
static mut CTR: [u64; 8] = [0; 8];

/// M68: 校准起点 (timer arm) + 计数器启用默认 (0=PIT, 1=syscalls)。
pub fn init() {
    timer::fujo_timer_arm();
    unsafe {
        CTR_ON[0] = true;
        CTR_ON[1] = true;
    }
}

pub fn bump(id: usize) {
    unsafe {
        if id < 8 && CTR_ON[id] {
            CTR[id] += 1;
        }
    }
}

/// 0x6E01: 帧边界标记 (µs 间隔入表)。
pub fn fujo_perf_frame_mark() -> i64 {
    let now = timer::fujo_timer_us() as u64;
    unsafe {
        if F_LAST != 0 {
            let d = now - F_LAST;
            F_TAB[F_IDX % FTAB] = d;
            F_IDX += 1;
            F_N += 1;
            if F_N > FTAB {
                F_N = FTAB;
            }
            F_SUM = F_SUM.saturating_add(d);
            if d > F_MAX {
                F_MAX = d;
            }
        }
        F_LAST = now;
    }
    0
}

/// 0x6E02: (frames, avg_us, max_us, sum_us)。
pub fn fujo_perf_frame_stats(ptr: u64) -> i64 {
    unsafe {
        let n = F_N as u64;
        let avg = if n > 0 { F_SUM / n } else { 0 };
        let w = ptr as *mut u64;
        w.write(n);
        w.add(1).write(avg);
        w.add(2).write(F_MAX);
        w.add(3).write(F_SUM);
    }
    0
}

/// 0x6E03
pub fn fujo_perf_counter_enable(id: u64, on: u64) -> i64 {
    unsafe {
        if id < 8 {
            CTR_ON[id as usize] = on != 0;
        }
    }
    0
}

/// 0x6E04: u64×8 计数器转储。
pub fn fujo_perf_counter_read(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        for i in 0..8 {
            w.add(i).write(CTR[i]);
        }
    }
    0
}
