//! timer.rs — 高精度定时器 (M54): rdtsc 基 + 帧同步
//!
//! 校准: 跨中断期两阶段 (0x6100 arm 记录 rdtsc+tick 进度; 用户态期间
//! PIT 继续 tick (用户态 IF=1), 下次查询时按 (Δtick, Δrdtsc) 求 cyc/µs
//! —— 内核 syscall 期 IF 被 SFMASK 屏蔽, 单次调用内不能等 tick!
//! 0x6101 timer_us() / 0x6102 timer_ms() / 0x6103 sleep_us(us) /
//! 0x6104 frame_wait(us_per_frame) / 0x6105 timer_info(ptr)。

use crate::interrupts;
use crate::serial;

static mut CALIBRATED: bool = false;
static mut CYCLES_PER_US: u64 = 800; // TCG 标称初值 (校准后才可靠)
static mut START_TICKS: u64 = 0;
static mut START_RDTS: u64 = 0;

fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack)); }
    ((hi as u64) << 32) | lo as u64
}

/// 0x6100: 校准起点 (用户态调用后, PIT 中断在用户执行期推进)。
pub fn fujo_timer_arm() -> i64 {
    unsafe {
        START_TICKS = interrupts::ticks();
        START_RDTS = rdtsc();
    }
    0
}

fn calib() {
    unsafe {
        if CALIBRATED {
            return;
        }
        if START_TICKS > 0 {
            let dt = interrupts::ticks() - START_TICKS;
            if dt >= 1 {
                let dc = rdtsc() - START_RDTS;
                CYCLES_PER_US = dc / (dt * 10000);
                if CYCLES_PER_US == 0 {
                    CYCLES_PER_US = 1;
                }
                CALIBRATED = true;
                serial::write_str("timer: calibrated cyc/us=");
                crate::syscall::debug_dec(CYCLES_PER_US);
                serial::write_line("");
            }
        }
    }
}

/// 0x6101: 当前 µs (单调).
pub fn fujo_timer_us() -> i64 {
    calib();
    unsafe { (rdtsc() / CYCLES_PER_US) as i64 }
}

/// 0x6102: 当前 ms.
pub fn fujo_timer_ms() -> i64 {
    fujo_timer_us() / 1000
}

/// 0x6103: 忙等 us.
pub fn fujo_timer_sleep_us(us: u64) -> i64 {
    calib();
    let t = fujo_timer_us() as u64 + us;
    while (fujo_timer_us() as u64) < t {
        core::hint::spin_loop();
    }
    0
}

/// 0x6104: 帧同步 (忙等至下一帧边界).
pub fn fujo_timer_frame_wait(us_per_frame: u64) -> i64 {
    calib();
    let now = fujo_timer_us() as u64;
    let phase = now % us_per_frame.max(1);
    let left = us_per_frame - phase;
    if left > 0 {
        fujo_timer_sleep_us(left);
    }
    0
}

/// 0x6105: 信息 (ptr 写 cyc_per_us, ticks).
pub fn fujo_timer_info(ptr: u64) -> i64 {
    calib();
    unsafe {
        (ptr as *mut u64).write(CYCLES_PER_US);
        (ptr as *mut u64).add(1).write(interrupts::ticks());
    }
    0
}
