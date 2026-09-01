//! irq.rs — M67: 中断合并/减轻 v0 (串口/网卡中断面)
//!
//! 目标: 中断合并在"高频周期性中断"上把 N 个 tick 组批为 1 次处理
//! (窗口合并) + 中断时间预算统计 (单次成本/最坏成本)。
//!
//! v0 语义: 合并窗口 = 每 W 个 PIT tick 计一次"合并批", 调度/时钟保持
//! 逐 tick (合并层不动语义, 只做双账: irqs 与 batches); 串口/网卡
//! (无硬件中断源 v0) 的记录以 per-poller 为界, 见 文档 16。
//!
//! 接口:
//!   0x6D01 irq_set_window(w)        合并窗口 (1..64)
//!   0x6D02 irq_cost_stats(ptr)      u64×4: (irqs, batches, total_cyc, worst_cyc)

use crate::serial;

static mut WINDOW: u64 = 1;
static mut W_START: u64 = 0; // 窗口基点 (set 时 = 当前 IRQS)
static mut IRQS: u64 = 0;
static mut BATCHES: u64 = 0;
static mut LAST_TSC: u64 = 0;
static mut TOTAL_CYC: u64 = 0;
static mut WORST_CYC: u64 = 0;

fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack)); }
    ((hi as u64) << 32) | lo as u64
}

/// PIT tick 侧钩子 (smp::intr_note 调; M67 合并/成本记账)。
pub fn note() {
    let now = rdtsc();
    crate::perf::bump(0); // M68: 性能计数器 PIT IRQ
    unsafe {
        if LAST_TSC != 0 {
            let d = now - LAST_TSC;
            TOTAL_CYC += d;
            if d > WORST_CYC {
                WORST_CYC = d;
            }
        }
        LAST_TSC = now;
        IRQS += 1;
        // 合并批 = 自窗口基点以来每 W 个 tick 一次 (公式化, 不累计)
        BATCHES = (IRQS - W_START) / WINDOW.max(1);
    }
}

/// 0x6D01
pub fn fujo_irq_set_window(w: u64) -> i64 {
    unsafe {
        WINDOW = w.clamp(1, 64);
        W_START = IRQS; // 窗口切换: 基点重置 (批数从 0 起)
        BATCHES = 0;
    }
    0
}

/// 0x6D02
pub fn fujo_irq_cost_stats(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(IRQS);
        w.add(1).write(BATCHES);
        w.add(2).write(TOTAL_CYC);
        w.add(3).write(WORST_CYC);
        serial::write_str("irq  : merge window=");
        crate::syscall::debug_dec(WINDOW);
        serial::write_str(" irqs=");
        crate::syscall::debug_dec(IRQS);
        serial::write_str(" batches=");
        crate::syscall::debug_dec(BATCHES);
        serial::write_line("");
    }
    0
}
