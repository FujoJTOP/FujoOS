//! game2.rs — M69: 2D 游戏#2 + 输入延迟基准 (内核仪表)
//!
//! 输入延迟仪表: demo 每帧报 (输入采样时刻 → 渲染完成) 的 µs,
//! 内核累计 N/SUM/MAX; 汇总给验收报告 (M70)。
//!
//! 接口: 0x6F01 game2_latency(us) 累计一次延迟 / 0x6F02 game2_stats(ptr)
//!       → u64×4: (n, avg_us, max_us, hits_side) (hits 由 demo 经
//!       0x6F03 game2_hits(v) 写入)。

static mut L_N: u64 = 0;
static mut L_SUM: u64 = 0;
static mut L_MAX: u64 = 0;
static mut L_HITS: u64 = 0;

/// 0x6F01
pub fn fujo_game2_latency(us: u64) -> i64 {
    unsafe {
        L_N += 1;
        L_SUM = L_SUM.saturating_add(us);
        if us > L_MAX {
            L_MAX = us;
        }
    }
    0
}

/// 0x6F02
pub fn fujo_game2_stats(ptr: u64) -> i64 {
    unsafe {
        let n = L_N;
        let avg = if n > 0 { L_SUM / n } else { 0 };
        let w = ptr as *mut u64;
        w.write(n);
        w.add(1).write(avg);
        w.add(2).write(L_MAX);
        w.add(3).write(L_HITS);
    }
    0
}

/// 0x6F03: demo 上报命中数 (碰撞统计面)。
pub fn fujo_game2_hits(v: u64) -> i64 {
    unsafe {
        L_HITS = v;
    }
    0
}
