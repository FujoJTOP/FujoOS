//! modelreg.rs — M94: AI 服务 (模型注册表 + fupm 安装)
//!
//! 注册表: 4 槽 {name[16], size, active, calls}; 权重数据区
//! MODEL_DATA=0xF38000 (4×8KB, backbuffer/页缓存/权重库后, 恒等映射)。
//! 接口: 0x8401 fupm_install(ptr, size, name) /
//!       0x8402 reg_list(ptr) 条目: {name 16B, size, active, calls} 8B×4 /
//!       0x8403 reg_active(idx) 激活 (单槽) / 0x8404 fupm_remove(idx)。

use crate::serial;

const NS: usize = 4;
const MDATA: u64 = 0xF38000;
const MSLOT: u64 = 0x2000; // 8KiB / 模型

static mut ENAME: [[u8; 16]; NS] = [[0; 16]; NS];
static mut ESIZE: [u64; NS] = [0; NS];
static mut EACTIVE: [bool; NS] = [false; NS];
static mut ECALLS: [u64; NS] = [0; NS];

/// 0x8401
pub fn fujo_fupm_install(ptr: u64, size: u64, name: u64) -> i64 {
    unsafe {
        let mut slot = None;
        for i in 0..NS {
            if ESIZE[i] == 0 {
                slot = Some(i);
                break;
            }
        }
        let i = match slot {
            Some(i) => i,
            None => return -12, // -ENOMEM
        };
        let m = size.min(MSLOT);
        for k in 0..m as usize {
            (MDATA as *mut u8).add(i * MSLOT as usize + k).write((ptr as *const u8).add(k).read());
        }
        ESIZE[i] = m;
        for k in 0..16 {
            ENAME[i][k] = (name as *const u8).add(k).read();
        }
        // 名字零终止
        ENAME[i][15] = 0;
        serial::write_str("fupm : installed #");
        crate::syscall::debug_dec(i as u64);
        serial::write_str(" size=");
        crate::syscall::debug_dec(m);
        serial::write_line("");
    }
    0
}

/// 0x8402: 条目表转储: n × 4B×4: [size, active, calls, _]。名称经名字通道读?
/// v0: 直接写 (name 由读 MDATA 区 + 名字表经 0x8405? 简化: 条目:
/// u64×4 (size, active, calls, name_ptr_mdata_base))。
pub fn fujo_reg_list(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        for i in 0..NS {
            w.add(i * 4).write(ESIZE[i]);
            w.add(i * 4 + 1).write(if EACTIVE[i] { 1 } else { 0 });
            w.add(i * 4 + 2).write(ECALLS[i]);
            w.add(i * 4 + 3).write(MDATA + (i as u64) * MSLOT);
        }
    }
    0
}

/// 0x8403
pub fn fujo_reg_active(idx: u64) -> i64 {
    let i = (idx as usize).min(NS - 1);
    unsafe {
        if ESIZE[i] == 0 {
            return -22;
        }
        for k in 0..NS {
            EACTIVE[k] = false;
        }
        EACTIVE[i] = true;
        serial::write_str("fupm : active #");
        crate::syscall::debug_dec(i as u64);
        serial::write_line("");
    }
    0
}

/// 0x8404
pub fn fujo_fupm_remove(idx: u64) -> i64 {
    let i = (idx as usize).min(NS - 1);
    unsafe {
        if ESIZE[i] == 0 {
            return -22;
        }
        ESIZE[i] = 0;
        EACTIVE[i] = false;
        serial::write_str("fupm : removed #");
        crate::syscall::debug_dec(i as u64);
        serial::write_line("");
    }
    0
}
