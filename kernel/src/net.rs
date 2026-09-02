//! net.rs — W14a: virtio-net legacy 驱动 (复用 W13c3 vring 传输, 2 队列 rx/tx)
//!
//! QEMU: `-netdev user,id=net0 -device virtio-net-pci,netdev=net0,queue-size=32`
//!      (legacy: BAR0 I/O; 队列 0=rx 1=tx; 每槽 [hdr(10B)][data(1514B)] 链)
//! vring (每队列 3 帧; QEMU virtio-net 默认 queue-size=256):
//!   V+0x0000 desc[256]×16B (4KiB) | V+0x1000 avail (flags,idx,ring[256])
//!   | V+0x2000 used (独立页, flags,idx,elems[256]×8B)
//! rx 槽 i: hdr = RXBUF + i*1520 (10B), data = RXBUF + i*1520 + 16 (1514B)
//! tx: hdr = TXBUF+0 (10B), data = TXBUF+0x10 (1514B)
//! 接口: 0x8A04 net_info / 0x8A05 net_tx / 0x8A06 net_rx (轮询, TCG 安全)。

use crate::acpi;
use crate::serial;
use crate::syscall;

const VIRTIO_VENDOR: u16 = 0x1AF4;
const VIRTIO_NET: u16 = 0x1000;

// 0.9.5 寄存器 (W13c3 已验证; 字节偏移)
const VIO_STATUS: usize = 0x12; // 字节
const VIO_QUEUE_PFN: usize = 0x08;
const VIO_QUEUE_SIZE: usize = 0x0C;
const VIO_QUEUE_SEL: usize = 0x0E;
const VIO_QUEUE_NOTIFY: usize = 0x10;
const VIO_CONFIG: usize = 0x14; // 网卡 config: mac[6] @ 0x14

const VQ_SIZE: usize = 256; // 设备默认 (virtio-net-pci 无 queue-size 属性)
const DESC_SIZE: usize = 16;
const SLOTS: usize = 16;
const NET_HDR: usize = 10;
const BUF_SIZE: usize = 1514;
const SLOT_STRIDE: usize = 1520; // hdr(10) @ +0, data @ +16
const OFF_AVAIL: usize = 0x1000;
const OFF_USED: usize = 0x2000;

static mut IO: u16 = 0;
static mut MAC: [u8; 6] = [0; 6];
static mut READY: bool = false;
static mut RX_VQ: u64 = 0;
static mut RX_BUF: u64 = 0;
static mut TX_VQ: u64 = 0;
static mut TX_BUF: u64 = 0;
static mut RX_LAST: u16 = 0;
static mut TX_LAST: u16 = 0;

fn inl(base: u16, off: usize) -> u32 {
    let port = base.wrapping_add(off as u16);
    let v: u32;
    unsafe {
        core::arch::asm!("in eax, dx", in("dx") port as u16, out("eax") v, options(nomem, nostack));
    }
    v
}

fn inw(base: u16, off: usize) -> u16 {
    let port = base.wrapping_add(off as u16);
    let v: u16;
    unsafe {
        core::arch::asm!("in ax, dx", in("dx") port as u16, out("ax") v, options(nomem, nostack));
    }
    v
}

fn inb(base: u16, off: usize) -> u8 {
    let port = base.wrapping_add(off as u16);
    let v: u8;
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port as u16, out("al") v, options(nomem, nostack));
    }
    v
}

fn outl(base: u16, off: usize, val: u32) {
    let port = base.wrapping_add(off as u16);
    unsafe {
        core::arch::asm!("out dx, eax", in("dx") port as u16, in("eax") val, options(nomem, nostack));
    }
}

fn outw(base: u16, off: usize, val: u16) {
    let port = base.wrapping_add(off as u16);
    unsafe {
        core::arch::asm!("out dx, ax", in("dx") port as u16, in("ax") val, options(nomem, nostack));
    }
}

fn outb(base: u16, off: usize, val: u8) {
    let port = base.wrapping_add(off as u16);
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port as u16, in("al") val, options(nomem, nostack));
    }
}

/// 队列注册: sel -> 尺寸 (设备 256) -> 3 帧 (desc/avail + used 独立页) -> pfn。
fn vq_register(qsel: u16) -> u64 {
    unsafe {
        outw(IO, VIO_QUEUE_SEL, qsel);
        let qsz = inw(IO, VIO_QUEUE_SIZE).min(VQ_SIZE as u16);
        let phys = crate::mem::alloc_frames_kernel(3).expect("net vring oom");
        for k in 0..3072usize {
            ((phys as *mut u64).add(k)).write(0);
        }
        outl(IO, VIO_QUEUE_PFN, (phys >> 12) as u32);
        let _ = qsz;
        phys
    }
}

/// 启动: 找 1AF4:1000 -> BAR0 -> 状态 -> 两队列 + rx 预投 -> DRIVER_OK -> MAC。
pub fn init() -> bool {
    unsafe {
        match acpi::pci_find(VIRTIO_VENDOR, VIRTIO_NET) {
            Some((bus, slot, func, bar)) => {
                acpi::pci_write_cfg(bus, slot, func, 0x04, 0x7);
                let io = (bar & !0x3) as u16;
                if io == 0 {
                    return false;
                }
                IO = io;
                let magic = inl(io, 0);
                outb(io, VIO_STATUS, 0);
                outb(io, VIO_STATUS, 1 | 2);
                outl(io, 0x04, 0); // GUEST_FEATURES = 0
                RX_VQ = vq_register(0); // rx
                TX_VQ = vq_register(1); // tx
                RX_BUF = crate::mem::alloc_frames_kernel(6).expect("net rxbuf oom");
                TX_BUF = crate::mem::alloc_frames_kernel(1).expect("net txbuf oom");
                for k in 0..(6 * 512) {
                    ((RX_BUF as *mut u64).add(k)).write(0);
                }
                for k in 0..512 {
                    ((TX_BUF as *mut u64).add(k)).write(0);
                }
                // rx 预投 16 槽: desc[2i]=hdr(WRITE|NEXT) desc[2i+1]=data(WRITE)
                let rxq = RX_VQ as *mut u8;
                let mut ai = 0u16;
                for i in 0..SLOTS {
                    let d = rxq.add(i * 32) as *mut u8;
                    let hdr_a = RX_BUF + (i * SLOT_STRIDE) as u64;
                    let data_a = RX_BUF + (i * SLOT_STRIDE + 16) as u64;
                    (d.add(0) as *mut u64).write(hdr_a);
                    (d.add(8) as *mut u32).write(NET_HDR as u32);
                    (d.add(12) as *mut u16).write(0x3); // NEXT|WRITE
                    (d.add(14) as *mut u16).write((2 * i + 1) as u16);
                    let d2 = rxq.add(i * 32 + 16) as *mut u8;
                    (d2.add(0) as *mut u64).write(data_a);
                    (d2.add(8) as *mut u32).write(BUF_SIZE as u32);
                    (d2.add(12) as *mut u16).write(0x2); // WRITE
                    (d2.add(14) as *mut u16).write(0);
                    (rxq.add(OFF_AVAIL + 4 + (i * 2)) as *mut u16).write((2 * i) as u16);
                    ai = ai.wrapping_add(1);
                }
                (rxq.add(OFF_AVAIL + 2) as *mut u16).write(ai);
                outl(io, VIO_QUEUE_NOTIFY, 0); // rx kick (guest->device 通知)
                outb(io, VIO_STATUS, 1 | 2 | 4);
                let st_ok = inb(io, VIO_STATUS);
                for m in 0..6 {
                    MAC[m] = inb(io, VIO_CONFIG + m);
                }
                serial::write_str("net  : virtio-net ready io=0x");
                syscall::debug_hex(io as u64);
                serial::write_str(" status=0x");
                syscall::debug_hex(st_ok as u64);
                serial::write_str(" mac=");
                for m in 0..6 {
                    syscall::debug_hex(MAC[m] as u64);
                    if m < 5 {
                        serial::write_str(":");
                    }
                }
                serial::write_line("");
                READY = true;
                true
            }
            None => false,
        }
    }
}

pub fn ready() -> bool {
    unsafe { READY }
}

/// 0x8A04: 状态 (out[0]=ready, out[1]=mac0, ..*4=mac5, out[5]=rx_last, out[6]=tx_last)。
#[no_mangle]
pub extern "C" fn fujo_net_info(out: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&out) {
        return -14;
    }
    unsafe {
        let o = out as *mut u64;
        o.write(if READY { 1 } else { 0 });
        for m in 0..6 {
            o.add(1 + m).write(MAC[m] as u64);
        }
        o.add(7).write(RX_LAST as u64);
        o.add(8).write(TX_LAST as u64);
    }
    0
}

/// 0x8A05: 发送一帧 (完整 ethernet frame, ≤1514B)。0 成功 / -1 超时 / -2 长度错。
#[no_mangle]
pub extern "C" fn fujo_net_tx(buf: u64, len: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&buf) || len == 0 || len > BUF_SIZE as u64 {
        return -2;
    }
    unsafe {
        if !READY {
            return -3;
        }
        // 拷数据 (hdr 10B 全零: 无 offload)
        for k in 0..NET_HDR as usize {
            ((TX_BUF as *mut u8).add(k)).write(0);
        }
        for k in 0..len as usize {
            ((TX_BUF as *mut u8).add(0x10 + k)).write((buf as *const u8).add(k).read_volatile());
        }
        let txq = TX_VQ as *mut u8;
        let d = txq as *mut u8;
        (d.add(0) as *mut u64).write(TX_BUF);
        (d.add(8) as *mut u32).write(NET_HDR as u32);
        (d.add(12) as *mut u16).write(0x1); // NEXT
        (d.add(14) as *mut u16).write(1);
        let d2 = txq.add(16) as *mut u8;
        (d2.add(0) as *mut u64).write(TX_BUF + 0x10);
        (d2.add(8) as *mut u32).write(len as u32);
        (d2.add(12) as *mut u16).write(0x0); // 设备读 (host->device)
        (d2.add(14) as *mut u16).write(0);
        let ai = (txq.add(OFF_AVAIL + 2) as *mut u16).read_volatile();
        (txq.add(OFF_AVAIL + 4) as *mut u16).write_volatile(0);
        (txq.add(OFF_AVAIL + 2) as *mut u16).write_volatile(ai.wrapping_add(1));
        outl(IO, VIO_QUEUE_NOTIFY, 1); // tx 队列 kick
        let mut spin: u64 = 0;
        loop {
            spin += 1;
            let ui = (txq.add(OFF_USED + 2) as *mut u16).read_volatile();
            if ui != TX_LAST {
                TX_LAST = ui;
                return 0;
            }
            if spin > 60_000_000 {
                return -1;
            }
        }
    }
}

/// 0x8A06: 收帧 (有包 -> 拷贝 eth frame 到 buf 并返回长度; 无包 -> 0; cap 小 -> -4)。
#[no_mangle]
pub extern "C" fn fujo_net_rx(buf: u64, cap: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&buf) {
        return -14;
    }
    unsafe {
        if !READY {
            return -3;
        }
        let rxq = RX_VQ as *mut u8;
        let ui = (rxq.add(OFF_USED + 2) as *mut u16).read_volatile();
        if ui == RX_LAST {
            return 0;
        }
        // used elem: {id u32, len u32} @ used+4 (ring 起点)
        let pos = (OFF_USED + 4 + (RX_LAST as usize % SLOTS) * 8) as usize;
        let elem_id = (rxq.add(pos) as *mut u32).read_volatile();
        let elem_len = (rxq.add(pos + 4) as *mut u32).read_volatile();
        RX_LAST = RX_LAST.wrapping_add(1);
        let slot = (elem_id as usize) / 2;
        let n = (elem_len as usize).saturating_sub(NET_HDR);
        if n == 0 || n > BUF_SIZE || slot >= SLOTS {
            return 0;
        }
        if cap < n as u64 {
            return -4;
        }
        let src = (RX_BUF + (slot * SLOT_STRIDE + 16) as u64) as *const u8;
        for k in 0..n {
            (buf as *mut u8).add(k).write_volatile(src.add(k).read_volatile());
        }
        // 重投该槽 (desc 不变, 只需 avail 再登记链头 + idx++)
        let ai = (rxq.add(OFF_AVAIL + 2) as *mut u16).read_volatile();
        (rxq.add(OFF_AVAIL + 4 + ((ai as usize % VQ_SIZE) * 2)) as *mut u16).write_volatile(elem_id as u16);
        (rxq.add(OFF_AVAIL + 2) as *mut u16).write_volatile(ai.wrapping_add(1));
        n as i64
    }
}
