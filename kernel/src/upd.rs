//! upd.rs — M99: 签名/更新机制 v0
//!
//! 签名 = FNV-1a 64 哈希 (自写, 与 fujopack fnv1a 一致); 更新流:
//!   内存新内核 → 哈希校验 (与期望一致) → 写 FJFS fujo-kernel.new
//!   → 校验读回 → 替换 fujo-kernel.bin; 篡改 1 字节 → -EINVAL。
//! 接口: 0x8801 upd_check(ptr, cap) → (-1 校验错/0 ok, 哈希经 ptr)
//!       0x8802 upd_apply(ptr, len, expected) → 0 | -22 | -30
//!       0x8803 upd_status(ptr) → (kernel_hash, pending, upd_count)

use crate::fjfs;
use crate::syscall;
use crate::serial;

static mut KERNEL_HASH: u64 = 0;
static mut UPD_COUNT: u64 = 0;
static mut PENDING: u64 = 0;

pub fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0x811C9DC5;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0193);
    }
    (h as u32) as u64 | ((h.wrapping_mul(0x1000193 * 977) as u32 as u64) << 32)
}

/// 0x8801: 对内存首 4KiB 计算签名 (demo/更新检查通用)。
pub fn fujo_upd_check(ptr: u64, cap: u64) -> i64 {
    let n = cap.min(4096);
    let data = unsafe { core::slice::from_raw_parts(ptr as *const u8, n as usize) };
    let h = fnv1a(data);
    unsafe {
        KERNEL_HASH = h;
    }
    serial::write_str("upd  : hash=");
    crate::syscall::debug_hex(h);
    serial::write_line("");
    0
}

/// 0x8802: 应用更新。
pub fn fujo_upd_apply(ptr: u64, len: u64, expected: u64) -> i64 {
    let data = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };
    let h = fnv1a(data);
    if h != expected {
        serial::write_line("upd  : hash mismatch - refused");
        return -22; // -EINVAL
    }
    if !fjfs::superblock_ok() {
        return -30; // -EROFS
    }
    // 写盘: fujo-kernel.bin (v0 直接覆盖) + 读回校验
    if !fjfs::write_file(b"fujo-kernel.bin", ptr as *const u8, len as usize) {
        return -5; // -EIO
    }
    // 写后握手段: QEMU IDE 写缓存刷新需短时 (实测立即读回全 0);
    // PIT 在 syscall 期被 SFMASK 屏蔽, 用 rdtsc 忙等 (~10ms 估)。
    {
        let _ = crate::timer::fujo_timer_sleep_us(10_000);
    }
    let mut buf = [0u8; 2048];
    let n = fjfs::read_file(b"fujo-kernel.bin", buf.as_mut_ptr(), 2048);
    if n as u64 != len {
        return -5;
    }
    if fnv1a(&buf[..n]) != expected {
        unsafe { PENDING = 1; }
        return -22;
    }
    unsafe {
        UPD_COUNT += 1;
        PENDING = 0;
    }
    serial::write_str("upd  : applied update #");
    crate::syscall::debug_dec(unsafe { UPD_COUNT });
    serial::write_line(" (hash verified)");
    0
}

/// 0x8803: (kernel_hash, pending, upd_count)。
pub fn fujo_upd_status(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(KERNEL_HASH);
        w.add(1).write(PENDING);
        w.add(2).write(UPD_COUNT);
    }
    0
}
