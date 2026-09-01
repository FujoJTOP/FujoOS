//! modelcard.rs — M87: 模型卡 (权限/计费/审计元数据, 资源节面)
//!
//! 模型卡 (120B): name[24] version[8] perm_mask u64 cost u32 calls u32
//!                     tokens u64 budget u64 res u64
//! 审计环 (16 项): [ts u64, model u64(卡索引), tokens u64, result u64]
//! 接口: 0x7D01 mc_register(ptr) / 0x7D02 mc_call(ptr,len,perm_need) /
//!       0x7D03 mc_info(ptr) / 0x7D04 mc_audit(ptr,cap)。

use crate::serial;

const AUDIT_N: usize = 16;

static mut MC_NAME: [u8; 24] = [0; 24];
static mut MC_VERSION: u8 = 0;
static mut MC_PERM: u64 = 0;
static mut MC_COST: u32 = 0;
static mut MC_CALLS: u32 = 0;
static mut MC_TOKENS: u64 = 0;
static mut MC_BUDGET: u64 = 0;
static mut MC_REGISTERED: bool = false;

static mut AUDIT: [(u64, u64, u64, u64); AUDIT_N] = [(0, 0, 0, 0); AUDIT_N];
static mut AUDIT_POS: u64 = 0;
static mut AUDIT_NUM: u64 = 0;

fn rd64(p: u64) -> u64 {
    unsafe { (p as *const u64).read() }
}

/// 0x7D01: 装载模型卡 (布局见头注释)。
pub fn fujo_mc_register(ptr: u64) -> i64 {
    unsafe {
        let b = ptr as *const u8;
        for i in 0..24 {
            MC_NAME[i] = b.add(i).read();
        }
        MC_VERSION = b.add(24).read();
        MC_PERM = rd64(ptr + 32);
        MC_COST = rd64(ptr + 40) as u32;
        MC_CALLS = 0;
        MC_TOKENS = 0;
        MC_BUDGET = rd64(ptr + 56);
        MC_REGISTERED = true;
        serial::write_str("mcard: registered '");
        serial::write_str(core::str::from_utf8(&MC_NAME).unwrap_or("?"));
        serial::write_str("' perm=");
        crate::syscall::debug_hex(MC_PERM);
        serial::write_line("");
    }
    0
}

/// 0x7D02: 调用 (计费+审计). perm_need 掩码检查.
pub fn fujo_mc_call(ptr: u64, len: u64, perm_need: u64) -> i64 {
    unsafe {
        if !MC_REGISTERED {
            return -19; // -ENODEV
        }
        let _ = ptr;
        let tokens = len;
        let denied = (MC_PERM & perm_need) != perm_need;
        let over_budget = MC_BUDGET != 0 && MC_TOKENS + tokens > MC_BUDGET && MC_BUDGET != u64::MAX;
        let result: i64 = if denied || over_budget {
            -1 // -EPERM (审计记 deny)
        } else {
            MC_CALLS += 1;
            MC_TOKENS += tokens;
            0
        };
        AUDIT[(AUDIT_POS % AUDIT_N as u64) as usize] =
            (crate::interrupts::ticks(), 0, tokens, result as u64);
        AUDIT_POS += 1;
        AUDIT_NUM += 1;
        serial::write_str("mcard: call tokens=");
        crate::syscall::debug_dec(tokens);
        serial::write_str(" result=");
        crate::syscall::debug_dec(result as u64);
        serial::write_line("");
        result
    }
}

/// 0x7D03: (calls, tokens, budget, perm_lo32? u64 直接)。
pub fn fujo_mc_info(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(MC_CALLS as u64);
        w.add(1).write(MC_TOKENS);
        w.add(2).write(MC_BUDGET);
        w.add(3).write(MC_PERM);
    }
    0
}

/// 0x7D04: 审计拷贝: 每项 32B (ts, model, tokens, result) × min(cap/32, 16)。
pub fn fujo_mc_audit(ptr: u64, cap: u64) -> i64 {
    unsafe {
        let n = ((cap / 32) as usize).min(AUDIT_N).min(AUDIT_NUM as usize);
        for i in 0..n {
            let idx = (AUDIT_POS as usize).wrapping_add(AUDIT_N - n + i) % AUDIT_N;
            let (ts, m, tk, r) = AUDIT[idx];
            let w = (ptr as *mut u64).add(i * 4);
            w.write(ts);
            w.add(1).write(m);
            w.add(2).write(tk);
            w.add(3).write(r);
        }
        let na = (cap / 32).min(AUDIT_N as u64).min(AUDIT_NUM);
        na as i64
    }
}
