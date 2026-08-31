//! kobj.rs — M19 内核对象/句柄表 v0 (统一资源抽象)
//!
//! 背景: VFS fd 表 (M15) / IPC 管道 (M18) / 共享窗口 / 信号各自为政。
//! M19 把"内核资源"收敛为类型化对象表: 每个资源注册一条 KObj 记录
//! (kind + epoch + payload), 由统一原语创建/释放/统计 —— 日后句柄制
//! (close_by_handle / 引用计数 / 继承) 的接缝。
//!
//! 原语 (fujo 原生槽):
//!   0x5130 fujo_kobj_create(kind) -> handle (kind: 2=pipe, 3=shm, 4=sig)
//!   0x5131 fujo_kobj_free(handle)  -> 0
//!   0x5132 fujo_kobj_info(ptr, n)  -> 各类型计数写入 ptr[n] (i32 ×4)
//!
//! 说明 v0: 对象表为全局单表 (无每进程隔离, 见 M14b 注);
//! pipe 的 fd 登记与 kobj 记录并行 (fd 供 VFS 路径, kobj 供统计/审计)。

use crate::serial;

pub const KOBJ_MAX: usize = 64;

pub const K_FILE: u8 = 1; // VFS 文件 (M15)
pub const K_PIPE: u8 = 2; // IPC 管道 (M18)
pub const K_SHM: u8 = 3; // 共享窗口 (M18)
pub const K_SIG: u8 = 4; // 信号 (M18)

#[derive(Clone, Copy)]
pub struct KObj {
    pub kind: u8,
    pub epoch: u16, // 一代序数: 复用槽时递增, 防悬垂句柄
    pub payload: u64,
}

static mut KOBJS: [KObj; KOBJ_MAX] = [KObj { kind: 0, epoch: 0, payload: 0 }; KOBJ_MAX];
static mut NEXT_EPOCH: [u16; KOBJ_MAX] = [0; KOBJ_MAX];
static mut ALLOC_LOG: u64 = 0; // M20: 噪音抑制 (仅记录前 16 次)
static mut FREE_LOG: u64 = 0;

fn print_dec(v: u64) {
    let mut buf = [0u8; 24];
    let mut i = 24;
    let mut x = v;
    if x == 0 {
        serial::write_str("0");
        return;
    }
    while x > 0 {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    serial::write_str(core::str::from_utf8(&buf[i..]).unwrap());
}

/// 分配对象: 空槽 -> epoch++ -> kind/payload 写入, 返回槽索引 (句柄)。
/// v0 句柄 = 槽索引 (0..63); epoch 用于 free 后的防悬垂校验。
pub fn alloc(kind: u8, payload: u64) -> Option<usize> {
    unsafe {
        for i in 0..KOBJ_MAX {
            if KOBJS[i].kind == 0 {
                NEXT_EPOCH[i] = NEXT_EPOCH[i].wrapping_add(1);
                KOBJS[i].kind = kind;
                KOBJS[i].epoch = NEXT_EPOCH[i];
                KOBJS[i].payload = payload;
                if ALLOC_LOG < 16 {
                    ALLOC_LOG += 1;
                    serial::write_str("kobj : alloc kind=");
                    print_dec(kind as u64);
                    serial::write_str(" slot=");
                    print_dec(i as u64);
                    serial::write_line("");
                }
                return Some(i);
            }
        }
        None
    }
}

/// 释放对象 (按槽索引), epoch 递增防悬垂。
pub fn free(slot: usize) -> bool {
    unsafe {
        if slot >= KOBJ_MAX || KOBJS[slot].kind == 0 {
            return false;
        }
        if FREE_LOG < 16 {
            FREE_LOG += 1;
            serial::write_str("kobj : free slot=");
            print_dec(slot as u64);
            serial::write_str(" (kind ");
            print_dec(KOBJS[slot].kind as u64);
            serial::write_line(")");
        }
        KOBJS[slot].kind = 0;
        KOBJS[slot].payload = 0;
        true
    }
}

/// 统计各类型对象计数 (用于 fujo_kobj_info)。
pub fn counts() -> [u32; 4] {
    unsafe {
        let mut c = [0u32; 4]; // [file, pipe, shm, sig]
        for i in 0..KOBJ_MAX {
            let k = KOBJS[i].kind;
            if k == K_FILE {
                c[0] += 1;
            } else if k == K_PIPE {
                c[1] += 1;
            } else if k == K_SHM {
                c[2] += 1;
            } else if k == K_SIG {
                c[3] += 1;
            }
        }
        c
    }
}

/// fujo_kobj_create(kind) -> slot | -12 (-ENOMEM)
pub fn fujo_kobj_create(kind: u64) -> i64 {
    let kind = match kind {
        1 => K_FILE,
        2 => K_PIPE,
        3 => K_SHM,
        4 => K_SIG,
        _ => {
            serial::write_str("kobj : create bad kind ");
            print_dec(kind);
            serial::write_line(" -EINVAL");
            return -22;
        }
    };
    match alloc(kind, 0) {
        Some(s) => s as i64,
        None => -12,
    }
}

/// fujo_kobj_free(handle) -> 0 | -22
pub fn fujo_kobj_free(handle: u64) -> i64 {
    if free(handle as usize) {
        0
    } else {
        -22
    }
}

/// fujo_kobj_info(ptr, n): 把各类型计数写入用户 ptr (i32 × min(4,n))。
/// 返回写入个数。
pub fn fujo_kobj_info(ptr: u64, n: u64) -> i64 {
    unsafe {
        if !(0x400000..0xC00000).contains(&ptr) {
            return -14;
        }
        let c = counts();
        let m = (n as usize).min(4);
        for i in 0..m {
            (ptr as *mut i32).add(i).write(c[i] as i32);
        }
        serial::write_str("kobj : info counts file=");
        print_dec(c[0] as u64);
        serial::write_str(" pipe=");
        print_dec(c[1] as u64);
        serial::write_str(" shm=");
        print_dec(c[2] as u64);
        serial::write_str(" sig=");
        print_dec(c[3] as u64);
        serial::write_line("");
        m as i64
    }
}
