//! dump.rs — M84: 崩溃转储 (minidump 雏形)
//!
//! 用户态异常 (#GP/#UD/#PF...) 时记录快照:
//!   [0..8)   "FUJDUMP\0"
//!   [8]  vec  [16] rip  [24] cr2  [32] rsp  [40] cs
//!   [48..8+64] regs8 (r11 r10 r9 r8 rdi rsi rdx rcx)
//!   [112]     count (u64)   → 120 字节
//! 接口: 0x7B01 dump_arm(on) / 0x7B02 dump_read(ptr, cap) →
//!       拷贝字节数 / 0x7B03 dump_info(ptr) → (count, vec, rip, cr2)。

use crate::serial;

const MAGIC: &[u8; 8] = b"FUJDUMP\0";
static mut DUMP_ON: bool = false;
static mut D_COUNT: u64 = 0;
static mut D_VEC: u64 = 0;
static mut D_RIP: u64 = 0;
static mut D_CR2: u64 = 0;
static mut D_RSP: u64 = 0;
static mut D_CS: u64 = 0;
static mut D_REGS: [u64; 8] = [0; 8];

/// exc2 挂接: 用户异常 (含隔离转场前) 记录。
pub fn note_exc(vec: u64, regs: *mut u64, e: u64) {
    unsafe {
        if !DUMP_ON {
            return;
        }
        let rip = regs.add(10 + e as usize).read();
        D_VEC = vec;
        D_RIP = rip;
        D_RSP = regs.add(12 + e as usize).read();
        D_CS = regs.add(11 + e as usize).read() & 0xFF;
        for k in 0..8 {
            D_REGS[k] = regs.add(k).read();
        }
        if vec == 14 {
            let mut cr2: u64;
            core::arch::asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags));
            D_CR2 = cr2;
        } else {
            D_CR2 = 0;
        }
        D_COUNT += 1;
        serial::write_str("dump : captured minidump #");
        crate::syscall::debug_dec(D_COUNT);
        serial::write_str(" vec=");
        crate::syscall::debug_dec(vec);
        serial::write_str(" rip=");
        crate::syscall::debug_hex(rip);
        serial::write_line("");
    }
}

/// 0x7B01
pub fn fujo_dump_arm(on: u64) -> i64 {
    unsafe {
        DUMP_ON = on != 0;
    }
    0
}

/// 0x7B02: 拷贝 minidump (120B) 到用户缓冲。
pub fn fujo_dump_read(ptr: u64, cap: u64) -> i64 {
    unsafe {
        if (cap as usize) < 120 {
            return -14; // -EFAULT
        }
        let b = ptr as *mut u8;
        for i in 0..8 {
            b.add(i).write(MAGIC[i]);
        }
        (ptr as *mut u64).add(1).write(D_VEC);
        (ptr as *mut u64).add(2).write(D_RIP);
        (ptr as *mut u64).add(3).write(D_CR2);
        (ptr as *mut u64).add(4).write(D_RSP);
        (ptr as *mut u64).add(5).write(D_CS);
        for k in 0..8 {
            (ptr as *mut u64).add(6 + k).write(D_REGS[k]);
        }
        (ptr as *mut u64).add(14).write(D_COUNT);
    }
    120
}

/// 0x7B03
pub fn fujo_dump_info(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(D_COUNT);
        w.add(1).write(D_VEC);
        w.add(2).write(D_RIP);
        w.add(3).write(D_CR2);
    }
    0
}
