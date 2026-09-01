//! dbg.rs — M75: 调试器 v0: 单步/断点 (调试寄存器 DR0/DR7)
//!
//! - 执行断点: DR0=addr, DR7 置 L0 (bit0) + RW0=00 (执行) + LEN0=00 (1B);
//!   命中 → #DB (向量 1) → fujo_dbg_exc 记录 + 清 DR7 (防无限循环)。
//! - 单步: 用户在用户态置 TF (pushfq|or 0x100|popfq), 每条指令 #DB;
//!   内核清 TF (帧 RFLAGS & !0x100) → dtf 返回继续。
//! - 接口: 0x7601 dbg_step(on) 状态 / 0x7602 dbg_bp0(addr) 设断点 /
//!   0x7603 dbg_info(ptr) → u64×4: (count, last_rip, steps, bps) /
//!   0x7604 dbg_clear()。

use crate::serial;

static mut DBG_COUNT: u64 = 0;
static mut DBG_LAST_RIP: u64 = 0;
static mut DBG_STEPS: u64 = 0;
static mut DBG_BPS: u64 = 0;
static mut DBG_BP_ENABLED: bool = false;
static mut DBG_STEP_ON: bool = false;
static mut BP_ORIG: u8 = 0x00;

fn wr_dr0(v: u64) {
    unsafe { core::arch::asm!("mov dr0, rax", in("rax") v, options(nostack)); }
}
fn wr_dr7(v: u64) {
    unsafe { core::arch::asm!("mov dr7, rax", in("rax") v, options(nostack)); }
}

/// #DB 桩入口 (向量 1): 记录 + 清 TF/断点后 iretq 返回。
#[no_mangle]
pub extern "C" fn fujo_dbg_exc(_vec: u64, regs: *mut u64) {
    unsafe {
        // regs 布局 (同 exc): [0]=r11 ... [8]=rax, [9]=RIP [10]=CS
        // [11]=RFLAGS [12]=RSP [13]=SS
        DBG_COUNT += 1;
        DBG_LAST_RIP = regs.add(9).read();
        DBG_STEPS += 1;
        // 清 TF: 单步不级联
        let fl = regs.add(11).read();
        regs.add(11).write(fl & !0x100);
        if DBG_COUNT <= 8 || DBG_COUNT % 128 == 0 {
            serial::write_str("dbg  : #DB (step) rip=");
            crate::syscall::debug_hex(DBG_LAST_RIP);
            serial::write_line("");
        }
    }
}

/// #BP (向量 3, int3) 入口: 恢复原字节 + RIP-1 后返回。
#[no_mangle]
pub extern "C" fn fujo_dbg_bp_exc(_vec: u64, regs: *mut u64) {
    unsafe {
        DBG_COUNT += 1;
        let rip = regs.add(9).read() - 1; // int3 命中后 RIP 指向其后
        DBG_LAST_RIP = rip;
        DBG_BPS += 1;
        DBG_BP_ENABLED = false;
        (rip as *mut u8).write(BP_ORIG); // 恢复
        regs.add(9).write(rip); // 退回重执原指令
        let fl = regs.add(11).read();
        regs.add(11).write(fl & !0x100);
        serial::write_str("dbg  : #BP hit @");
        crate::syscall::debug_hex(rip);
        serial::write_line("");
    }
}

/// 0x7601
pub fn fujo_dbg_step(on: u64) -> i64 {
    unsafe {
        DBG_STEP_ON = on != 0;
    }
    0
}

/// 0x7602: 软件断点 (int3 替换首字节; 命中自动恢复并回退 RIP)。
pub fn fujo_dbg_bp0(addr: u64) -> i64 {
    unsafe {
        BP_ORIG = (addr as *const u8).read();
        (addr as *mut u8).write(0xCC);
        DBG_BP_ENABLED = true;
        serial::write_str("dbg  : int3 bp @");
        crate::syscall::debug_hex(addr);
        serial::write_line("");
    }
    0
}

/// 0x7603
pub fn fujo_dbg_info(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(DBG_COUNT);
        w.add(1).write(DBG_LAST_RIP);
        w.add(2).write(DBG_STEPS);
        w.add(3).write(DBG_BPS);
    }
    0
}

/// 0x7604
pub fn fujo_dbg_clear() -> i64 {
    unsafe {
        wr_dr7(0);
        wr_dr0(0);
        DBG_BP_ENABLED = false;
        DBG_STEP_ON = false;
    }
    0
}
