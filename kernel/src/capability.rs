//! capability.rs — M91: 权限与审计 (四件套④): 能力表 + 审计日志
//!
//! 能力表: 8 槽 {perm u64, granted bool}; 审计环 32 项
//! {ts, action, subject, result}; cap_check 拒绝自动入审计。
//! 接口: 0x8101 cap_grant(idx,perm) / 0x8102 cap_check(idx,perm) /
//!       0x8103 aud_log(action,subject) / 0x8104 aud_read(ptr,cap)。
//!
//! M112 (AI For Next ③ 动作通道): 0x8105 cap_exec(act,arg0,arg1) ——
//! 模型输出 → 内核执行, 经 cap_check 授权 (槽 6 = exec 槽, perm bit=act-1)
//! + aud_log(action=2)。动作: KILL/ISOLATE/RESUME/SET_CFG/ACK;
//! LAUNCH 需 syscall 现场 (在 syscall.rs 分发内实现)。
//! 0x8106 cfg_get(key): 配置读取 (anom 阈值/自动隔离等, SET 经 cap_exec)。

use crate::serial;

const N_CAP: usize = 8;
const N_AUD: usize = 32;

static mut CAP_PERM: [u64; N_CAP] = [0; N_CAP];
static mut CAP_GRANT: [bool; N_CAP] = [false; N_CAP];

static mut AUD: [(u64, u64, u64, u64); N_AUD] = [(0, 0, 0, 0); N_AUD];
static mut AUD_POS: u64 = 0;
static mut AUD_NUM: u64 = 0;
static mut DENIES: u64 = 0;

// ---- M112: exec 动作集 (act 1..=6, perm bit = act-1) ----
pub const EXEC_SLOT: usize = 6;
pub const ACT_KILL: u64 = 1;
pub const ACT_ISOLATE: u64 = 2;
pub const ACT_LAUNCH: u64 = 3;
pub const ACT_SET_CFG: u64 = 4;
pub const ACT_RESUME: u64 = 5;
pub const ACT_ACK: u64 = 6;
pub const ALL_ACTS: u64 = 0x3F;

/// M112: 配置槽 (key 1..=8): 1=anom 置信阈值 (默认 50), 2=自动隔离 (默认 0)。
static mut CFG: [(u64, u64); 8] = [(0, 0); 8];

pub fn cfg_set(key: u64, val: u64) -> i64 {
    if key == 0 || key > 8 {
        return -22;
    }
    unsafe {
        CFG[(key - 1) as usize] = (key, val);
        serial::write_str("cfg  : set #");
        crate::syscall::debug_dec(key);
        serial::write_str(" = ");
        crate::syscall::debug_dec(val);
        serial::write_line("");
    }
    0
}

/// 0x8106: 配置读 (未设置返回默认)。
pub fn fujo_cfg_get(key: u64) -> i64 {
    let def = match key {
        1 => 50,  // anom 阈值
        2 => 0,   // 自动隔离 off
        _ => 0,
    };
    if key == 0 || key > 8 {
        return -22;
    }
    unsafe {
        let (k, v) = CFG[(key - 1) as usize];
        if k == key {
            v as i64
        } else {
            def
        }
    }
}

/// M112: exec 动作授权检查 (槽 6 + perm bit)。
pub fn exec_authorized(act: u64) -> bool {
    if act == 0 || act > 6 {
        return false;
    }
    unsafe {
        CAP_GRANT[EXEC_SLOT] && (CAP_PERM[EXEC_SLOT] & (1u64 << (act - 1))) != 0
    }
}

/// M112: exec 审计落笔。
pub fn aud_exec(act: u64, result: u64) {
    aud_note(2, act, result); // action=2 (exec)
    unsafe {
        if result == 1 {
            DENIES += 1;
        }
    }
}

/// M112: 0x8105 主体 —— 非 LAUNCH 动作 (LAUNCH 在 syscall.rs 分发内, 需现场)。
pub fn fujo_cap_exec(act: u64, a0: u64, a1: u64) -> i64 {
    if !exec_authorized(act) {
        aud_exec(act, 1);
        serial::write_str("cap  : deny exec #");
        crate::syscall::debug_dec(act);
        serial::write_line("");
        return -1;
    }
    let rc = match act {
        ACT_KILL => crate::sched::kill_task(a0 as usize),
        ACT_ISOLATE => crate::sched::task_suspend(a0 as usize),
        ACT_RESUME => crate::sched::task_resume(a0 as usize),
        ACT_SET_CFG => cfg_set(a0, a1),
        ACT_ACK => crate::ai::anom_ack(),
        _ => -22,
    };
    aud_exec(act, if rc == 0 { 0 } else { 1 });
    serial::write_str("cap  : exec #");
    crate::syscall::debug_dec(act);
    serial::write_line(if rc == 0 { " ok" } else { " rc!=0" });
    rc
}

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

/// M118 (R1): 审计条目总数 (公理化自检)。
pub fn aud_num() -> u64 {
    unsafe { AUD_NUM }
}

/// M118 (R1): 最近一条审计 (ts, action, subject, result)。
pub fn aud_tail() -> (u64, u64, u64, u64) {
    unsafe {
        if AUD_NUM == 0 {
            return (0, 0, 0, 0);
        }
        AUD[(AUD_POS.wrapping_sub(1) % N_AUD as u64) as usize]
    }
}
