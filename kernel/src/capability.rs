//! capability.rs — M91: 权限与审计 (四件套④): 能力表 + 审计日志
//!
//! 能力表: 8 槽 {perm u64, granted bool}; 审计环 32 项
//! {ts, action, subject, result}; cap_check 拒绝自动入审计。
//! 接口: 0x8101 cap_grant(idx,perm) / 0x8102 cap_check(idx,perm) /
//!       0x8103 aud_log(action,subject) / 0x8104 aud_read(ptr,cap)。

use crate::serial;

const N_CAP: usize = 8;
const N_AUD: usize = 32;

static mut CAP_PERM: [u64; N_CAP] = [0; N_CAP];
static mut CAP_GRANT: [bool; N_CAP] = [false; N_CAP];

static mut AUD: [(u64, u64, u64, u64); N_AUD] = [(0, 0, 0, 0); N_AUD];
static mut AUD_POS: u64 = 0;
static mut AUD_NUM: u64 = 0;
static mut DENIES: u64 = 0;

fn aud_note(action: u64, subject: u64, result: u64) {
    unsafe {
        AUD[(AUD_POS % N_AUD as u64) as usize] =
            (crate::interrupts::ticks(), action, subject, result);
        AUD_POS += 1;
        AUD_NUM += 1;
    }
}

/// 0x8101
pub fn fujo_cap_grant(idx: u64, perm: u64) -> i64 {
    let i = (idx as usize).min(N_CAP - 1);
    unsafe {
        CAP_PERM[i] = perm;
        CAP_GRANT[i] = true;
        serial::write_str("cap  : grant #");
        crate::syscall::debug_dec(i as u64);
        serial::write_str(" perm=");
        crate::syscall::debug_hex(perm);
        serial::write_line("");
    }
    0
}

/// 0x8102: 检查 (deny 记审计)。
pub fn fujo_cap_check(idx: u64, perm: u64) -> i64 {
    let i = (idx as usize).min(N_CAP - 1);
    unsafe {
        if CAP_GRANT[i] && (CAP_PERM[i] & perm) == perm {
            return 0;
        }
        DENIES += 1;
        aud_note(1, i as u64, 1); // action=1 (check), result=1 (deny)
        serial::write_str("cap  : deny #");
        crate::syscall::debug_dec(i as u64);
        serial::write_line("");
        -1
    }
}

/// 0x8103
pub fn fujo_aud_log(action: u64, subject: u64) -> i64 {
    aud_note(action, subject, 0);
    0
}

/// 0x8104: 条目拷贝 (32B 每项: ts, action, subject, result)。
pub fn fujo_aud_read(ptr: u64, cap: u64) -> i64 {
    unsafe {
        let n = ((cap / 32) as usize).min(N_AUD).min(AUD_NUM as usize);
        for i in 0..n {
            let idx = (AUD_POS as usize).wrapping_add(N_AUD - n + i) % N_AUD;
            let (ts, a, s, r) = AUD[idx];
            let w = (ptr as *mut u64).add(i * 4);
            w.write(ts);
            w.add(1).write(a);
            w.add(2).write(s);
            w.add(3).write(r);
        }
        let na = (cap / 32).min(N_AUD as u64).min(AUD_NUM);
        na as i64
    }
}

pub fn denies() -> u64 {
    unsafe { DENIES }
}
