//! installer.rs — M98: live 镜像 + 安装器 v0
//!
//! 安装 = boot 模块 (initrd ELF) 拷入 FJFS 卷 /system/fujo-kernel.bin
//! + 引导计数 /system/bootcount (每次 install 递增) —— 盘上持久。
//! 接口: 0x8701 inst_install() / 0x8702 inst_status(ptr) →
//!       (installed, kernel_size, volume_ok, boot_count)。

use crate::ata;
use crate::fjfs;
use crate::syscall;
use crate::serial;

static mut INSTALLED: bool = false;
static mut KERNEL_SIZE: u64 = 0;
static mut BOOT_COUNT: u64 = 0;

/// 0x8701: 安装 (拷贝 boot 模块 → /system/fujo-kernel.bin + 计数)。
pub fn fujo_inst_install() -> i64 {
    if !unsafe { ata::ATA_PRESENT } {
        return -19; // -ENODEV (无参考盘)
    }
    if !fjfs::superblock_ok() {
        return -30; // -EROFS
    }
    // 取 boot 模块 (M15 remember_module 保存)
    let (addr, len) = syscall::boot_module_vals();
    if len == 0 {
        return -2; // -ENOENT
    }
    if !fjfs::write_file(b"fujo-kernel.bin", addr as *const u8, len as usize) {
        return -5; // -EIO
    }
    // bootcount: 读盘上现有值 +1 (阶段2 起递增; 证明盘上持久)
    let mut bc = [0u8; 8];
    let n = fjfs::read_file(b"bootcount", bc.as_mut_ptr(), 8);
    let mut prev = 0u64;
    if n == 8 {
        for i in 0..8 {
            prev |= (bc[i] as u64) << (8 * i);
        }
    }
    unsafe { BOOT_COUNT = prev + 1; }
    for i in 0..8 {
        bc[i] = (unsafe { BOOT_COUNT } >> (8 * i)) as u8;
    }
    let _ = fjfs::write_file(b"bootcount", bc.as_ptr(), 8);
    unsafe {
        INSTALLED = true;
        KERNEL_SIZE = len;
    }
    serial::write_str("inst : kernel -> /system/fujo-kernel.bin (");
    crate::syscall::debug_dec(len);
    serial::write_line(" bytes) installed");
    0
}

/// 0x8702: (installed, kernel_size, volume_ok, boot_count)。
pub fn fujo_inst_status(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(if INSTALLED { 1 } else { 0 });
        w.add(1).write(KERNEL_SIZE);
        w.add(2).write(if fjfs::superblock_ok() { 1 } else { 0 });
        w.add(3).write(BOOT_COUNT);
    }
    0
}
