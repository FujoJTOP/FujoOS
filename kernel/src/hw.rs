//! hw.rs — M97: 真机显示/键盘/存储适配 (参考机: QEMU 参考平台)
//!
//! 汇总面: 显示 (VBE 当前分辨率 + LFB), 键盘 (IRQ 计数),
//! 存储 (ATA 参考盘 + FJFS 卷状态)。
//! 接口: 0x8601 hw_disp(ptr) → (fbw, fbh, lfb_ok, kbd_irqs) /
//!       0x8602 hw_storage(ptr) → (ata, lba48, fs_ok, files)。

use crate::font;

pub static mut KBD_IRQS: u64 = 0;

/// 键盘 IRQ 挂点 (kbd.rs 每 IRQ 调)。
pub fn kbd_note() {
    unsafe { KBD_IRQS += 1; }
}

/// 0x8601
pub fn fujo_hw_disp(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(font::fb_w() as u64);
        w.add(1).write(font::fb_h() as u64);
        w.add(2).write(if font::fb_w() > 0 { 1 } else { 0 });
        w.add(3).write(KBD_IRQS);
    }
    0
}

/// 0x8602: (ata, lba48, fs_ok, files)。
pub fn fujo_hw_storage(ptr: u64) -> i64 {
    let ata = unsafe { crate::ata::ATA_PRESENT };
    let lba48 = ata && crate::ata::lba48_capable();
    let fs_ok = crate::fjfs::superblock_ok();
    let files = crate::fjfs::file_count();
    unsafe {
        let w = ptr as *mut u64;
        w.write(if ata { 1 } else { 0 });
        w.add(1).write(if lba48 { 1 } else { 0 });
        w.add(2).write(if fs_ok { 1 } else { 0 });
        w.add(3).write(files);
    }
    0
}
