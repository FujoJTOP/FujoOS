//! virtio.rs — W13: PCI 总线模型上的 virtio-blk (legacy 轮询) 驱动
//!
//! 参考机 100% 可复现: QEMU `-drive if=virtio,file=...,format=raw` -> PCI
//! vendor=0x1AF4 device=0x1001;BAR0 = I/O 端口区 (legacy);
//! 设备区头部: magic "virt", ver, device_id(=2), vendor_id。
//!
//! vring (单队列, 16 描述符) 在帧分配器 4KiB 帧内 (恒等映射, guest-physical):
//!   0x000 desc[16]×16B | 0x200 avail (idx+ring[16]) | 0x300 used (idx+ring[16])
//!   0x400 req header 16B | 0x500 data 512B | 0x900 status 1B
//! 提交: desc0=header(IN) desc1=data(WRITE) desc2=status(WRITE) -> notify ->
//! 轮询 used.idx (无中断, TCG 安全), status==0 成功。
//!
//! 接口: 0x8A01 vblk_read(lba, out, cap) = 0 成功 / -1 失败 (读入用户缓冲)。

use crate::acpi;
use crate::serial;
use crate::syscall;

const VIRTIO_VENDOR: u16 = 0x1AF4;
const VIRTIO_BLK: u16 = 0x1001;

// legacy I/O 寄存器 (BAR 基址 + 偏移)
const VIO_STATUS: usize = 0x14;
const VIO_QUEUE_PFN: usize = 0x08;
const VIO_QUEUE_SIZE: usize = 0x0C;
const VIO_QUEUE_SEL: usize = 0x0E;
const VIO_QUEUE_NOTIFY: usize = 0x10;

const VQ_N: usize = 16;
const VQ_PAGE: u64 = 0x4000; // vring + 缓冲 一帧
const OFF_REQ: usize = 0x400;
const OFF_DATA: usize = 0x500;
const OFF_STATUS: usize = 0x900;

static mut IO_BASE: u16 = 0;
static mut VQ_PHYS: u64 = 0;
static mut VQ_READY: bool = false;
static mut LAST_USED: u16 = 0;
static mut SECTOR_DATA: u64 = 0; // 内核侧直接地址 (恒等 = phys)

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

fn inb(base: u16, off: usize) -> u8 {
    let port = base.wrapping_add(off as u16);
    let v: u8;
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port as u16, out("al") v, options(nomem, nostack));
    }
    v
}

fn outb(base: u16, off: usize, val: u8) {
    let port = base.wrapping_add(off as u16);
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port as u16, in("al") val, options(nomem, nostack));
    }
}

/// 启动初始化: 找设备 -> BAR0 (I/O) -> 置 ACK/DRIVER -> vring 帧 -> QUEUE_PFN -> DRIVER_OK。
pub fn init() -> bool {
    unsafe {
        match crate::acpi::pci_find(VIRTIO_VENDOR, VIRTIO_BLK) {
            Some((bus, slot, func, bar)) => {
                // PCI 命令寄存器: 使能 I/O 空间 + 总线主控 (复位默认 0!)
                crate::acpi::pci_write_cfg(bus, slot, func, 0x04, 0x7);
                let io = (bar & !0x3) as u16;
                if io == 0 {
                    return false;
                }
                IO_BASE = io;
                // 设备区头部校验 (virtio_magic 在 I/O 空间 traps: 0x00..0x07 只读)
                let magic = inl(io, 0);
                let did = inl(io, 0x08);
                serial::write_str("vblk : bar0=0x");
                syscall::debug_hex(bar as u64);
                serial::write_str(" io=0x");
                syscall::debug_hex(io as u64);
                serial::write_str(" magic=0x");
                syscall::debug_hex(magic as u64);
                serial::write_str(" dev=");
                syscall::debug_dec((did >> 16) as u64);
                serial::write_line("");
                // 状态: RESET(0) -> ACK(1) -> DRIVER(2)
                outl(io, VIO_STATUS, 0);
                outl(io, VIO_STATUS, 1 | 2);
                // vring 帧 (4KiB 清零; 恒等映射, guest-physical 直用)
                let phys = match crate::mem::alloc_frame_kernel() {
                    Some(p) => p,
                    None => return false,
                };
                VQ_PHYS = phys;
                let vq = phys as *mut u8;
                for i in 0..VQ_PAGE as usize {
                    vq.add(i).write(0);
                }
                // 队列 0: size 声明 + 页帧
                outw(io, VIO_QUEUE_SEL, 0);
                let qsz = inw(io, VIO_QUEUE_SIZE).min(VQ_N as u16) as usize;
                outl(io, VIO_QUEUE_PFN, (phys >> 12) as u32);
                // 读回校验
                let pfn_ro = inl(io, VIO_QUEUE_PFN);
                let st_ro = inb(io, VIO_STATUS);
                serial::write_str("vblk : vring phys=0x");
                syscall::debug_hex(phys);
                serial::write_str(" qsz=");
                syscall::debug_dec(qsz as u64);
                serial::write_str(" pfn_ro=0x");
                syscall::debug_hex(pfn_ro as u64);
                serial::write_str(" status_ro=0x");
                syscall::debug_hex(st_ro as u64);
                serial::write_line("");
                let _ = qsz;
                // DRIVER_OK
                outl(io, VIO_STATUS, 1 | 2 | 4);
                SECTOR_DATA = phys + OFF_DATA as u64;
                VQ_READY = true;
                true
            }
            None => false,
        }
    }
}

pub fn ready() -> bool {
    unsafe { VQ_READY }
}

/// 0x8A02: 驱动状态 (out[0]=ready, out[1]=io_base, out[2]=qsz, out[3]=vring_phys)。
#[no_mangle]
pub extern "C" fn fujo_vblk_info(out: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&out) {
        return -14;
    }
    unsafe {
        let o = out as *mut u64;
        o.write(if VQ_READY { 1 } else { 0 });
        o.add(1).write(IO_BASE as u64);
        o.add(2).write(VQ_N as u64);
        o.add(3).write(VQ_PHYS);
    }
    0
}

/// 0x8A01: 读扇区 (LBA) -> 用户缓冲 (512B)。0 成功 / -1 失败。
#[no_mangle]
pub extern "C" fn fujo_vblk_read(lba: u64, out: u64, cap: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&out) || cap < 512 {
        return -14;
    }
    unsafe {
        if !VQ_READY {
            return -1;
        }
        let vq = VQ_PHYS as *mut u8;
        // 请求头: type=0 (read), reserved=0, sector=lba
        (vq.add(OFF_REQ) as *mut u32).write_volatile(0);
        (vq.add(OFF_REQ + 4) as *mut u32).write_volatile(0);
        (vq.add(OFF_REQ + 8) as *mut u64).write_volatile(lba);
        // 描述符 0/1/2
        let d = vq.add(0x000) as *mut u32;
        d.add(0).write_volatile((VQ_PHYS + OFF_REQ as u64) as u32); // addr
        d.add(1).write_volatile(16); // len
        d.add(2).write_volatile(0x1); // NEXT
        d.add(3).write_volatile(1); // next desc
        d.add(4).write_volatile(SECTOR_DATA as u32);
        d.add(5).write_volatile(512);
        d.add(6).write_volatile(0x3); // NEXT|WRITE
        d.add(7).write_volatile(2);
        d.add(8).write_volatile((VQ_PHYS + OFF_STATUS as u64) as u32);
        d.add(9).write_volatile(1);
        d.add(10).write_volatile(0x2); // WRITE (last)
        d.add(11).write_volatile(0);
        // avail 投递 (紧随 desc 表: flags@0x100, idx@0x102, ring@0x104)
        let avail_idx = (vq.add(0x100 + 2) as *mut u16).read_volatile();
        (vq.add(0x100 + 4 + ((avail_idx as usize % VQ_N) * 2)) as *mut u16).write_volatile(0);
        (vq.add(0x100 + 2) as *mut u16).write_volatile(avail_idx.wrapping_add(1));
        // notify (queue 0)
        outw(IO_BASE, VIO_QUEUE_NOTIFY, 0);
        // 轮询 used.idx (返回 -2 = 提交/等待超时, 数据路径未到)
        let mut spin: u64 = 0;
        loop {
            let used_idx = (vq.add(0x128 + 2) as *mut u16).read_volatile();
            if used_idx != LAST_USED {
                LAST_USED = used_idx;
                let status = (vq.add(OFF_STATUS) as *mut u8).read_volatile();
                if status == 0 {
                    // 512B 拷给用户
                    for k in 0..512usize {
                        (out as *mut u8).add(k).write_volatile(vq.add(OFF_DATA + k).read_volatile());
                    }
                    return 0;
                }
                return -1;
            }
            spin += 1;
            if spin > 200_000_000 {
                return -2;
            }
        }
    }
}
