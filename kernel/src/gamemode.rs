//! gamemode.rs — M59: 游戏模式 (前台调度/资源预留/全屏)
//!
//! 0x6601 game_mode(on)     前台调度标记 (sched::set_game_mode: PIT 对
//!                          游戏任务独占切换)
//! 0x6602 game_status(ptr)  读 (mode, ticks, heap_base) — 资源预留面
//! 0x6603 game_fullscreen(on) 全屏 (VBE 1024x768; on=确认全屏)
//!
//! M114: 策略执行面 —— cfg 3=game_ban (1=禁), 4/5=时段 (小时), 命中时段
//! 拒绝启用游戏模式 (0x6601 返回 -1)。

use crate::font;
use crate::graphics;
use crate::interrupts;
use crate::sched;

pub static mut GAME_MODE: u64 = 0;

/// M114: 当前小时 (PIT 100Hz: 360000 ticks/h)。
pub fn hour_of_day() -> u64 {
    (interrupts::ticks() / 360_000) % 24
}

/// 0x6601
pub fn fujo_game_mode(on: u64) -> i64 {
    if on != 0 && crate::capability::fujo_cfg_get(3) == 1 {
        let h = hour_of_day();
        let s = crate::capability::fujo_cfg_get(4) as u64;
        let e = crate::capability::fujo_cfg_get(5) as u64;
        if s < e && h >= s && h < e {
            crate::serial::write_line("game : denied by policy (work-hour ban)");
            return -1;
        }
    }
    unsafe {
        GAME_MODE = on;
        sched::set_game_mode(on != 0);
        crate::serial::write_line("game : foreground mode set (M59)");
    }
    0
}

/// 0x6602: 状态 (ptr 写 3×u64: mode, ticks, heap).
pub fn fujo_game_status(ptr: u64) -> i64 {
    unsafe {
        (ptr as *mut u64).write(GAME_MODE);
        (ptr as *mut u64).add(1).write(interrupts::ticks());
        (ptr as *mut u64).add(2).write(0x800000u64); // 预留堆基址
    }
    0
}

/// 0x6603: 全屏 (1024x768 确认; off 恢复).
pub fn fujo_game_fullscreen(on: u64) -> i64 {
    let which = if on != 0 { 0u64 } else { 0u64 };
    let r = graphics::fujo_vbe_set(which);
    unsafe {
        let _ = font::fb_w();
    }
    r
}
