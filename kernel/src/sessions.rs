//! sessions.rs — M88: agent 会话 v0 (检查点/恢复, 一等进程面)
//!
//! 会话表 (4 槽): active / gen / tokens / ck[128] (检查点 blob)。
//! 检查点: demo 工作区快照 (保存/恢复往返); gen 每次恢复递增。
//! 接口: 0x7E01 sess_create(id) / 0x7E02 sess_save(id, ptr, len≤128) /
//!       0x7E03 sess_load(id, ptr) → len | -19 / 0x7E04 sess_info(ptr) →
//!       (active, ck_len, gen, tokens) / 0x7E05 sess_tick(id, tokens)。

use crate::serial;

const NS: usize = 4;
const CK: usize = 128;

static mut ACTIVE: [bool; NS] = [false; NS];
static mut GEN: [u64; NS] = [0; NS];
static mut TOKENS: [u64; NS] = [0; NS];
static mut CK_LEN: [u64; NS] = [0; NS];
static mut CK_DATA: [[u8; CK]; NS] = [[0; CK]; NS];

/// 0x7E01
pub fn fujo_sess_create(id: u64) -> i64 {
    let i = (id as usize).min(NS - 1);
    unsafe {
        if ACTIVE[i] {
            return -16; // -EBUSY
        }
        ACTIVE[i] = true;
        GEN[i] = 0;
        TOKENS[i] = 0;
        CK_LEN[i] = 0;
        serial::write_str("sess : create #");
        crate::syscall::debug_dec(i as u64);
        serial::write_line("");
    }
    0
}

/// 0x7E02
pub fn fujo_sess_save(id: u64, ptr: u64, len: u64) -> i64 {
    let i = (id as usize).min(NS - 1);
    unsafe {
        if !ACTIVE[i] {
            return -19; // -ENODEV
        }
        let m = (len as usize).min(CK);
        for k in 0..m {
            CK_DATA[i][k] = (ptr as *const u8).add(k).read();
        }
        CK_LEN[i] = m as u64;
    }
    0
}

/// 0x7E03
pub fn fujo_sess_load(id: u64, ptr: u64) -> i64 {
    let i = (id as usize).min(NS - 1);
    unsafe {
        if !ACTIVE[i] {
            return -19;
        }
        for k in 0..CK_LEN[i] as usize {
            (ptr as *mut u8).add(k).write(CK_DATA[i][k]);
        }
        GEN[i] += 1;
        serial::write_str("sess : load #");
        crate::syscall::debug_dec(i as u64);
        serial::write_str(" gen=");
        crate::syscall::debug_dec(GEN[i]);
        serial::write_line("");
    }
    unsafe { CK_LEN[i] as i64 }
}

/// 0x7E04
pub fn fujo_sess_info(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        let mut active = 0u64;
        let mut max_ck = 0u64;
        let mut max_gen = 0u64;
        let mut max_tok = 0u64;
        for i in 0..NS {
            if ACTIVE[i] {
                active += 1;
                if CK_LEN[i] > max_ck {
                    max_ck = CK_LEN[i];
                }
                if GEN[i] > max_gen {
                    max_gen = GEN[i];
                }
                if TOKENS[i] > max_tok {
                    max_tok = TOKENS[i];
                }
            }
        }
        w.write(active);
        w.add(1).write(max_ck);
        w.add(2).write(max_gen);
        w.add(3).write(max_tok);
    }
    0
}

/// 0x7E05
pub fn fujo_sess_tick(id: u64, tokens: u64) -> i64 {
    let i = (id as usize).min(NS - 1);
    unsafe {
        if !ACTIVE[i] {
            return -19;
        }
        TOKENS[i] += tokens;
    }
    0
}
