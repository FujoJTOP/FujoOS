//! pcache.rs — M66: 页缓存/预读 v0 (内存盘→真盘桥)
//!
//! 槽模型: 16 页槽 (元数据静态 BSS, 数据页常量区 0xD00000, 16×4KiB)。
//! 模拟磁盘: 0xDF0000 (4×4KiB) —— boot 页表 0..64MiB 恒等映射,
//! U=0 只内核访问 (真 ATA 后由块设备层替换磁盘区, 接口不变)。
//!
//! 接口: 0x6C01 alloc(n) / 0x6C02 write(blk,ptr) / 0x6C03 read(blk,ptr) /
//!       0x6C04 prefetch(start,n) 从盘入缓存 / 0x6C05 flush() 脏页回盘 /
//!       0x6C06 evict() / 0x6C07 info(ptr) → (slots, dirty, hits, miss)

use crate::serial;

pub const NSLOTS: usize = 16;
const PAGE: usize = 4096;
// backbuffer = 0xC00000..0xF00000 (1024x768x32, 3MiB) —— 缓存区必须在其后!
const CACHE_DATA: u64 = 0xF10000; // 16 页数据区: 0xF10000..0xF20000
const MEM_DISK: u64 = 0xF24000; // 模拟盘 4 页: 0xF24000..0xF28000
const DISK_PAGES: u64 = 4;

/// 启动格式化: 模拟盘清零 (M66)。
pub fn init() {
    unsafe {
        for i in 0..(DISK_PAGES as usize * PAGE) {
            ((MEM_DISK as *mut u8).add(i)).write(0);
        }
    }
}

#[derive(Clone, Copy)]
struct Slot {
    blk: u64, // 块号 (0=空槽)
    valid: bool,
    dirty: bool,
}

static mut SLOTS: [Slot; NSLOTS] = [
    Slot { blk: 0, valid: false, dirty: false };
    NSLOTS
];
static mut HITS: u64 = 0;
static mut MISS: u64 = 0;

fn find(blk: u64) -> Option<usize> {
    unsafe {
        for i in 0..NSLOTS {
            if SLOTS[i].valid && SLOTS[i].blk == blk {
                return Some(i);
            }
        }
    }
    None
}

fn alloc_slot(blk: u64) -> usize {
    unsafe {
        // 优先空槽, 否则 blk 最大的槽退化替换 (LRU v0)
        let mut free = None;
        let mut oldest = 0usize;
        for i in 0..NSLOTS {
            if !SLOTS[i].valid {
                free = Some(i);
                break;
            }
            if SLOTS[i].blk > SLOTS[oldest].blk {
                oldest = i;
            }
        }
        let s = free.unwrap_or(oldest);
        SLOTS[s] = Slot { blk, valid: true, dirty: false };
        s
    }
}

fn copy_page(dst: u64, src: u64) {
    unsafe {
        for i in 0..PAGE {
            (dst as *mut u8).add(i).write((src as *const u8).add(i).read());
        }
    }
}

/// 0x6C01: 分配 n 页连续块空间 (v0 直接分配 n 个槽)。
pub fn fujo_pc_alloc(n: u64) -> i64 {
    let m = (n as usize).min(NSLOTS);
    // 连续性不做物理保证 (v0: 槽即逻辑页, 磁盘回写按 blk 编码)
    let mut free = 0;
    unsafe {
        for i in 0..m {
            for j in 0..NSLOTS {
                if !SLOTS[j].valid {
                    break;
                }
                free += 1;
            }
        }
    }
    // 简化: 返回首个可用连续空白 (0 起)
    let mut base: i64 = 0;
    unsafe {
        for i in 0..NSLOTS {
            if !SLOTS[i].valid {
                base = i as i64;
                break;
            }
        }
        for k in 0..m {
            let b = (base + k as i64) as u64;
            alloc_slot(b);
        }
    }
    base
}

/// 0x6C02: 写用户页到缓存 (脏页)。
pub fn fujo_pc_write(blk: u64, ptr: u64) -> i64 {
    let s = match find(blk) {
        Some(s) => s,
        None => alloc_slot(blk),
    };
    copy_page(CACHE_DATA + (s as u64) * PAGE as u64, ptr);
    unsafe {
        SLOTS[s].dirty = true;
    }
    0
}

/// 0x6C03: 读缓存页到用户 (未命中 → 从盘同步预读)。
pub fn fujo_pc_read(blk: u64, ptr: u64) -> i64 {
    let s = match find(blk) {
        Some(s) => {
            unsafe { HITS += 1; }
            s
        }
        None => {
            unsafe { MISS += 1; }
            // 从模拟盘同步 (块号 < 4 有效)
            if blk < 4 {
                let s = alloc_slot(blk);
                let src = MEM_DISK + (blk as u64) * PAGE as u64;
                serial::write_str("pcache: miss blk=");
                crate::syscall::debug_dec(blk);
                serial::write_str(" src=");
                crate::syscall::debug_hex(src);
                serial::write_str(" v=");
                crate::syscall::debug_hex(unsafe { (src as *const u8).read() as u64 });
                serial::write_line("");
                copy_page(CACHE_DATA + (s as u64) * PAGE as u64, src);
                s
            } else {
                // 盘上无此块: 返回空页
                let s = alloc_slot(blk);
                unsafe {
                    for i in 0..PAGE {
                        ((CACHE_DATA + (s as u64) * PAGE as u64) as *mut u8).add(i).write(0);
                    }
                }
                s
            }
        }
    };
    copy_page(ptr, CACHE_DATA + (s as u64) * PAGE as u64);
    0
}

/// 0x6C04: 预读: 从盘读 n 页到缓存 (顺序窗口; 脏页不清)。
pub fn fujo_pc_prefetch(start: u64, n: u64) -> i64 {
    for k in 0..n.min(4) {
        let blk = start + k;
        if blk >= 4 {
            break;
        }
        let s = alloc_slot(blk);
        copy_page(CACHE_DATA + (s as u64) * PAGE as u64, MEM_DISK + (blk as u64) * PAGE as u64);
    }
    n.min(4) as i64
}

/// 0x6C05: 脏页回写磁盘 (blk 编码偏移)。
pub fn fujo_pc_flush() -> i64 {
    let mut n = 0;
    unsafe {
        for i in 0..NSLOTS {
            if SLOTS[i].valid && SLOTS[i].dirty && SLOTS[i].blk < 4 {
                copy_page(
                    MEM_DISK + (SLOTS[i].blk as u64) * PAGE as u64,
                    CACHE_DATA + (i as u64) * PAGE as u64,
                );
                SLOTS[i].dirty = false;
                n += 1;
            }
        }
    }
    serial::write_str("pcache: flushed ");
    crate::syscall::debug_dec(n as u64);
    serial::write_line(" dirty pages -> mem-disk");
    n as i64
}

/// 0x6C06: 全部槽失效 (重载测试)。
pub fn fujo_pc_evict() -> i64 {
    unsafe {
        for i in 0..NSLOTS {
            SLOTS[i].valid = false;
            SLOTS[i].dirty = false;
        }
    }
    0
}

/// 0x6C07
pub fn fujo_pc_info(ptr: u64) -> i64 {
    let mut slots = 0u32;
    let mut dirty = 0u32;
    unsafe {
        for i in 0..NSLOTS {
            if SLOTS[i].valid {
                slots += 1;
            }
            if SLOTS[i].dirty {
                dirty += 1;
            }
        }
        (ptr as *mut u32).write(slots);
        (ptr as *mut u32).add(1).write(dirty);
        (ptr as *mut u32).add(2).write(HITS as u32);
        (ptr as *mut u32).add(3).write(MISS as u32);
    }
    0
}
