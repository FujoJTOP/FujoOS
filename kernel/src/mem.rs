//! mem.rs — M11 虚拟内存/堆 v0 (地基第一件)
//!
//! 现状: 引导桩建立 0..1GiB 恒等映射 (全级 U=1, 内核页用户可及 —— M11 修这个)。
//! M11 两步:
//!   ① harden_user_guard: 按物理地址重标身份映射的 U 位 —— 用户区
//!      (0x400000..0xC00000: 程序+栈+PE蹦床+堆) U=1, 其余 U=0 (内核镜像/内核栈/
//!      页表/显卡/VRAM 等), LFB 高区 (PML4[3]) 不动。
//!   ② 分配: brk/mmap 在预留常量映射堆 0x800000..0xC00000 (4MiB) 上 bump 分配。
//!      物理=虚拟 (恒等), M11 v0 不做按需分配 —— M12 缺页处理后升级。
//!
//! 原语 (linux ABI 直通): brk(nr12) / mmap(nr9: MAP_PRIVATE|MAP_ANON|MAP_FIXED 子集) /
//! munmap(nr11: v0 no-op)。验收: ring3 alloc_test brk 1MiB + mmap 2MiB 模式回读零错。

use crate::serial;

/// 用户堆区 (恒等映射, 引导桩已建, M11 硬化后 U=1)。
pub const USER_HEAP_BASE: u64 = 0x800000;
pub const USER_HEAP_LEN: u64 = 0x400000; // 4MiB
/// 用户可及范围 (程序 0x400000..0x600000 + PE 蹦床 0x7F0000 + 堆) + 用户栈。
const USER_RANGE_LO: u64 = 0x400000;
const USER_RANGE_HI: u64 = 0xC00000;

static mut HEAP_BRK: u64 = USER_HEAP_BASE;

pub fn init() {
    unsafe {
        HEAP_BRK = USER_HEAP_BASE;
    }
}

#[inline]
fn page_round(x: u64) -> u64 {
    (x + 0xFFF) & !0xFFF
}

unsafe fn read_cr3() -> u64 {
    let v: u64;
    core::arch::asm!("mov {}, cr3", out(reg) v, options(nomem, nostack, preserves_flags));
    v
}

/// ① U 位硬化: 遍历恒等低 1GiB (PML4[0]) 全部 4KiB 页, 用户区设 U, 其余清 U。
/// 返回处理过的页数。
pub fn harden_user_guard() -> u64 {
    unsafe {
        let pm4 = read_cr3() as *mut u64;
        let pdpt = (pm4.read() & 0x000F_FFFF_FFFF_F000) as *mut u64;
        let pd = (pdpt.read() & 0x000F_FFFF_FFFF_F000) as *mut u64;
        let mut touched: u64 = 0;
        let mut flipped: u64 = 0;
        for pdi in 0..512usize {
            let pt_raw = pd.add(pdi).read();
            if pt_raw & 0x1 == 0 {
                continue; // 未映射
            }
            // 2MiB 大页 (PS) 存在: 拆不了, v0 跳过 (本栈全部 4KiB, 不会走到)
            if pt_raw & 0x80 != 0 {
                continue;
            }
            let pt = (pt_raw & 0x000F_FFFF_FFFF_F000) as *mut u64;
            for pti in 0..512usize {
                let paddr = (pdi * 512 + pti) as u64 * 0x1000;
                let mut e = pt.add(pti).read();
                if e & 0x1 == 0 {
                    continue;
                }
                let is_user = paddr >= USER_RANGE_LO && paddr < USER_RANGE_HI;
                let want_u = if is_user { 4u64 } else { 0u64 };
                if e & 0x4 != want_u {
                    e = (e & !0x4) | want_u;
                    pt.add(pti).write(e);
                    flipped += 1;
                }
                touched += 1;
            }
        }
        serial::write_str("mem  : U-guard applied (pages=");
        print_dec(touched);
        serial::write_str(", flipped=");
        print_dec(flipped);
        serial::write_line(") - kernel pages now user-inaccessible");
        touched
    }
}

fn print_dec(v: u64) {
    let mut buf = [0u8; 24];
    let mut i = 24;
    let mut x = v;
    if x == 0 {
        serial::write_str("0");
        return;
    }
    while x > 0 {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    serial::write_str(core::str::from_utf8(&buf[i..]).unwrap());
}

fn print_hex(v: u64) {
    const HX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..16 {
        let d = ((v >> (4 * (15 - i))) & 0xF) as u8;
        buf[2 + i] = HX[d as usize];
    }
    serial::write_str(core::str::from_utf8(&buf).unwrap());
}

/// 系统调用 brk(nr12): 设置堆尾; 返回旧堆尾。
/// v0: 只增不缩 (缩请求返回旧值); 越界返回 -ENOMEM。
#[no_mangle]
pub extern "C" fn fujo_brk(ptr: u64) -> i64 {
    unsafe {
        let old = HEAP_BRK;
        if ptr == 0 {
            return old as i64;
        }
        if ptr < USER_HEAP_BASE {
            return -22; // -EINVAL
        }
        if ptr > USER_HEAP_BASE + USER_HEAP_LEN {
            serial::write_line("mem  : brk beyond heap region -ENOMEM");
            return -12; // -ENOMEM
        }
        if ptr > old {
            // 扩展: 新内存清零 (零页语义由恒等映射+写零满足)
            for a in old..ptr {
                (a as *mut u8).write(0);
            }
            HEAP_BRK = ptr;
        }
        // Linux 语义: 成功返回新堆尾 (v0 只增不缩, 缩请求返回旧值=当前值)
        ptr as i64
    }
}

/// 系统调用 mmap(nr9) v0: 匿名私有映射, bump 分配于堆区内。
/// 参数: (addr, len, prot, flags, fd, off) —— addr 仅作提示, v0 忽略;
/// flags 支持 MAP_PRIVATE(2)|MAP_ANONYMOUS(0x20); 其余 => -EINVAL。
#[no_mangle]
pub extern "C" fn fujo_mmap(addr: u64, len: u64, prot: u64, flags: u64, fd: u64, off: u64) -> i64 {
    let _ = addr;
    let _ = prot;
    let _ = fd;
    let _ = off;
    let need = if flags & (2 | 0x20) == (2 | 0x20) {
        page_round(len)
    } else {
        serial::write_line("mem  : mmap flags unsupported (anon|private only) -EINVAL");
        return -22; // -EINVAL
    };
    unsafe {
        let old = HEAP_BRK;
        if old + need > USER_HEAP_BASE + USER_HEAP_LEN {
            serial::write_line("mem  : mmap out of heap region -ENOMEM");
            return -12; // -ENOMEM
        }
        for a in old..old + need {
            (a as *mut u8).write(0);
        }
        HEAP_BRK = old + need;
        serial::write_str("mem  : mmap anon ");
        print_hex(old);
        serial::write_str(" + ");
        print_dec(need);
        serial::write_line(" bytes");
        old as i64
    }
}

/// 系统调用 munmap(nr11) v0: no-op (回收在 M12/M13 帧分配器后实现)。
#[no_mangle]
pub extern "C" fn fujo_munmap(_addr: u64, _len: u64) -> i64 {
    0
}
