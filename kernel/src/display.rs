//! display.rs — 显示驱动抽象 v0 (M51): VBE(std-vga) 后端 + virtio-gpu 探测
//!
//! 后端标识: 0=Bochs VBE (QEMU std-vga 0x1234:0x1111, games/UI 默认)
//!            1=virtio-gpu-pci (0x1AF4:0x1050, M61 起图形加速接入)
//! 0x5E01 disp_info(ptr) -> 写 u32×5 (backend, vendor_id, device_id, w, h)
//! 0x5E02 disp_set_backend(which) -> (v1: 返回当前后端, 切换留 M61)

use crate::font;
use crate::graphics;

fn pci_read(bus: u8, slot: u8, reg: u8) -> u32 {
    unsafe {
        let addr = 0x8000_0000u32
            | ((bus as u32) << 16)
            | ((slot as u32) << 11)
            | ((reg as u32) & 0xFC);
        let mut lo: u32 = addr;
        core::arch::asm!(
            "mov edx, 0xCF8",
            "out dx, eax",
            "mov edx, 0xCFC",
            "in eax, dx",
            inlateout("eax") lo,
            lateout("edx") _,
            options(nomem, nostack)
        );
        lo
    }
}

/// 枚举显示设备: 返回 (backend, vendor, device)。
pub fn detect() -> (u32, u32, u32) {
    for slot in 1u8..32 {
        let v = pci_read(0, slot, 0);
        let vid = v & 0xFFFF;
        let did = (v >> 16) & 0xFFFF;
        if vid == 0x1234 && did == 0x1111 {
            return (0, vid, did); // bochs/std-vga (VBE)
        }
        if vid == 0x1AF4 && did == 0x1050 {
            return (1, vid, did); // virtio-gpu-pci
        }
        if vid == 0xFFFF {
            continue;
        }
    }
    (0, 0, 0)
}

/// 0x5E01: disp_info(ptr) — 写 (backend, vendor, device, w, h)。
pub fn fujo_disp_info(ptr: u64) -> i64 {
    let (b, vid, did) = detect();
    unsafe {
        let p = ptr as *mut u32;
        p.add(0).write(b);
        p.add(1).write(vid);
        p.add(2).write(did);
        p.add(3).write(font::fb_w());
        p.add(4).write(font::fb_h());
    }
    0
}

/// 0x5E02: set_backend (v1: 记录偏好; 实际切换 M61)。
pub fn fujo_disp_set_backend(w: u64) -> i64 {
    let _ = unsafe { graphics::vbe_get(0x01) }; // 触达 (保持引用)
    w as i64
}
