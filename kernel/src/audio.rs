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

// ---------------------------------------------------------------------------
// M63: 混音器/效果链 v0 (CPU 侧, 采样级)
//
// 通道: 4 路 i16 单声道, 每路 128 样本存储 (M63 范围), 效果链:
//   输入 x -> 单极低通 y += k/256*(x-y) -> 增益 g/256 -> 混音累加饱和。
// 接口:
//   0x5F05 mix_open(ch)          重置通道
//   0x5F06 mix_push(ch, ptr, n)  追加样本 (n<=128)
//   0x5F07 mix_render(ptr, n, g) 混音全部通道 -> 用户缓冲 (i16)
//   0x5F08 mix_effect(ch, k, p)  k=1:低通系数(0..256) k=2:增益(0..256)
//   0x5F09 mix_status(ptr)       写 u32×2: (NCH, len0..3 打包)
// ---------------------------------------------------------------------------

const NCH: usize = 4;
const CBUF: usize = 128;

static mut CH_BUF: [[i16; CBUF]; NCH] = [[0; CBUF]; NCH];
static mut CH_LEN: [u16; NCH] = [0; NCH];
static mut CH_K: [u16; NCH] = [256; NCH]; // 低通系数 (x/256, 256=直通)
static mut CH_Y: [i32; NCH] = [0; NCH];   // 滤波状态
static mut CH_GAIN: [u16; NCH] = [256; NCH];

fn uget(ptr: u64) -> u16 {
    unsafe { (ptr as *const u16).read() }
}

/// 0x5F05
pub fn fujo_mix_open(ch: u64) -> i64 {
    let c = (ch as usize).min(NCH - 1);
    unsafe {
        CH_LEN[c] = 0;
        CH_K[c] = 256;
        CH_Y[c] = 0;
        CH_GAIN[c] = 256;
    }
    0
}

/// 0x5F06
pub fn fujo_mix_push(ch: u64, ptr: u64, n: u64) -> i64 {
    let c = (ch as usize).min(NCH - 1);
    let m = (n as usize).min(CBUF);
    unsafe {
        for i in 0..m {
            CH_BUF[c][i] = uget(ptr + (i as u64) * 2) as i16;
        }
        CH_LEN[c] = m as u16;
    }
    m as i64
}

/// 0x5F07
pub fn fujo_mix_render(ptr: u64, n: u64, gain: u64) -> i64 {
    let m = (n as usize).min(256);
    let g = (gain as u16).min(256);
    unsafe {
        for i in 0..m {
            let mut acc: i64 = 0;
            for c in 0..NCH {
                if i >= CH_LEN[c] as usize {
                    continue;
                }
                let x = CH_BUF[c][i] as i32;
                let k = CH_K[c] as i32;
                // 单极低通: y += k/256*(x - y)
                let dy = ((x - CH_Y[c]) * k) / 256;
                CH_Y[c] += dy;
                let xn = CH_Y[c] * (CH_GAIN[c] as i32) / 256;
                acc = acc.saturating_add(xn as i64);
            }
            let s = acc * (g as i64) / 256;
            let s = (s as i64).clamp(-32768, 32767) as i16;
            (ptr as *mut i16).add(i).write(s);
        }
    }
    m as i64
}

/// 0x5F08
pub fn fujo_mix_effect(ch: u64, kind: u64, param: u64) -> i64 {
    let c = (ch as usize).min(NCH - 1);
    unsafe {
        match kind {
            1 => CH_K[c] = (param as u16).min(256),
            2 => CH_GAIN[c] = (param as u16).min(256),
            _ => return -22, // -EINVAL
        }
    }
    0
}

/// 0x5F09
pub fn fujo_mix_status(ptr: u64) -> i64 {
    unsafe {
        (ptr as *mut u32).write(NCH as u32);
        let lens = (CH_LEN[0] as u32) | ((CH_LEN[1] as u32) << 8) | ((CH_LEN[2] as u32) << 16)
            | ((CH_LEN[3] as u32) << 24);
        (ptr as *mut u32).add(1).write(lens);
    }
    0
}
