//! platform.rs — W20: 平台检测原语 (QEMU 仿真 vs 真机)
//!
//! 动机 (用户指示): QEMU 专属行为严重拖累可靠性 —— 参考机 TCG 对多种硬件
//! 设备采用"宽松语义" (本内核已实证 LAPIC ICR 投递与 Intel SDM 相反, 详见
//! docs/74-platform-audit.md)。检测出 QEMU 后, 驱动按"QEMU 适配"或
//! "Intel/规范严格"两条路径运行, 真机信任规范语义。
//!
//! 证据链 (全部只读探测, 无副作用):
//!   ① Bochs VBE 特征 ID 0xB0C5 —— QEMU std-vga 专属 (真机 GPU/BIOS VBE
//!      返回厂商真实 ID, 不会回 0xB0C5); graphics::init 早期获得;
//!   ② PCI vendor 0x1234 (QEMU 出品) —— acpi::scan_all 后可用, 副证据。
//!
//! 保守默认: 无证据时按 QEMU 适配 (当前参考机全部回归在 QEMU 上; 真机
//! 首个 VBE 探测即给出定论)。

use crate::serial;

static mut IS_QEMU: bool = true;
static mut VBE_ID: u16 = 0;

/// graphics::init 早期调用 (VBE 特征 ID 读出后)。
pub fn note_vbe_id(id: u16) {
    unsafe {
        VBE_ID = id;
        IS_QEMU = id == 0xB0C5;
    }
    serial::write_str("plat : vbe_id=0x");
    crate::syscall::log_hex(id as u64);
    serial::write_str(" qemu=");
    serial::write_str(unsafe { if IS_QEMU { "1" } else { "0" } });
    serial::write_line(
        unsafe { if IS_QEMU { " (QEMU adapter default)" } else { " (real hw path)" } },
    );
}

pub fn is_qemu() -> bool {
    unsafe { IS_QEMU }
}

pub fn vbe_id() -> u16 {
    unsafe { VBE_ID }
}

/// 0x8D01: platform_info(ptr) — u64×3 = (is_qemu, vbe_id, icr_mode)。
/// icr_mode: 0 = QEMU 适配 (写低触发), 1 = Intel SDM (写高触发)。
#[no_mangle]
pub extern "C" fn fujo_platform_info(ptr: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&ptr) {
        return -14;
    }
    unsafe {
        let w = ptr as *mut u64;
        w.write(if IS_QEMU { 1 } else { 0 });
        w.add(1).write(VBE_ID as u64);
        w.add(2).write(crate::smp::icr_mode() as u64);
    }
    0
}
