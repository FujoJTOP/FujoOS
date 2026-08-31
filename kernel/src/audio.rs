//! audio.rs — AC97 音频驱动 v0 (M52): 探测/复位/播放入口
//!
//! QEMU `-device AC97` (Intel 82801AA AC'97, PCI 0x8086:0x2415, I/O BAM BAR0)。
//! 0x5F01 audio_info(ptr) -> u32×2 (present, vendor)
//! 0x5F02 audio_enable(on) -> 0 (全局控制+复位+音量初值)
//! 0x5F03 audio_volume(v) -> 0 (PCM out 音量)
//! 播放入口: 0x5F04 audio_playback(ptr, samples) -> n (声卡忽略无效数据;
//! 真实 FIFO 写留 M63 混音器)。验证: probe/regs/PAS。

use crate::serial;

const AC97_VENDOR: u32 = 0x8086;
const AC97_DEVICE: u32 = 0x2415;

fn pci_read(slot: u8, reg: u8) -> u32 {
    unsafe {
        let addr = 0x8000_0000u32 | ((slot as u32) << 11) | ((reg as u32) & 0xFC);
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

fn outw(port: u16, val: u16) {
    unsafe { core::arch::asm!("out dx, ax", in("dx") port, in("ax") val, options(nomem, nostack)); }
}
fn inw(port: u16) -> u16 {
    let val: u16;
    unsafe { core::arch::asm!("in ax, dx", out("ax") val, in("dx") port, options(nomem, nostack)); }
    val
}

/// 探测 AC97: 返回 (present, io_base)。
fn probe() -> (bool, u16) {
    for slot in 1u8..32 {
        let v = pci_read(slot, 0);
        let vid = v & 0xFFFF;
        let did = (v >> 16) & 0xFFFF;
        if vid == AC97_VENDOR && did == AC97_DEVICE {
            let bar0 = pci_read(slot, 0x10);
            return (true, (bar0 as u16) & !0x0F);
        }
        if vid == 0xFFFF {
            continue;
        }
    }
    (false, 0)
}

/// 0x5F01: audio_info(ptr) — 写 (present, vendor)。
pub fn fujo_audio_info(ptr: u64) -> i64 {
    let (p, _) = probe();
    unsafe {
        (ptr as *mut u32).write(if p { 1 } else { 0 });
        (ptr as *mut u32).add(1).write(AC97_VENDOR);
    }
    0
}

/// 0x5F02: audio_enable(on) — 复位+全局控制 (0x2C) + 初值音量。
pub fn fujo_audio_enable(on: u64) -> i64 {
    let (p, io) = probe();
    if !p {
        return -19; // -ENODEV
    }
    unsafe {
        outw(io + 0x2C, 0x0002); // 复位
        // 短等 (PIT 不阻塞; 直接继续)
        if on != 0 {
            outw(io + 0x2C, 0x0202); // Global Control: 复位解除 + 开通道
            outw(io + 0x18, 0x6060); // PCM out 音量 (0x60 ≈ 0dB)
        }
        serial::write_line("audio: ac97 enabled (M52)");
    }
    0
}

/// 0x5F03: 音量。
pub fn fujo_audio_volume(v: u64) -> i64 {
    let (p, io) = probe();
    if !p {
        return -19;
    }
    outw(io + 0x18, (v as u16) & 0x7F7F);
    0
}

/// 0x5F04: 播放入口 (sample 队列 v0: 返回接受数, 写入 FIFO 留 M63)。
pub fn fujo_audio_playback(_ptr: u64, samples: u64) -> i64 {
    let (p, _) = probe();
    if !p {
        return 0;
    }
    let n = samples.min(1024);
    serial::write_str("audio: playback queued ");
    crate::syscall::debug_dec(n as u64);
    serial::write_line(" samples");
    n as i64
}
