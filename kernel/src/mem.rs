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
            // M12: 不再内核清零 —— 堆区 P=0, 用户首写时按需零页 (demand_zero)
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
        // M12: 不再内核清零 —— 按需零页由 #PF 处理器分发
        HEAP_BRK = old + need;
        serial::write_str("mem  : mmap anon ");
        print_hex(old);
        serial::write_str(" + ");
        print_dec(need);
        serial::write_line(" bytes");
        old as i64
    }
}

/// 系统调用 munmap(nr11) v0: no-op (回收在帧分配器/调度器后实现)。
#[no_mangle]
pub extern "C" fn fujo_munmap(_addr: u64, _len: u64) -> i64 {
    0
}

// ---------------------------------------------------------------------------
// M12 · 缺页处理: 按需零页 (需求段 0x800000..0xC00000 用内核 PT 替换恒等映射)
// ---------------------------------------------------------------------------

/// 帧分配区域 (恒等映射: 物理=虚拟, 16MiB..63MiB, 全部在 64MiB 恒等内)。
const FRAME_BASE: u64 = 0x1000000;
const FRAME_END: u64 = 0x3F00000;
const FRAME_PAGES: usize = ((FRAME_END - FRAME_BASE) / 0x1000) as usize;
const BITMAP_LEN: usize = (FRAME_PAGES + 7) / 8;

static mut FRAME_BITMAP: [u8; BITMAP_LEN] = [0; BITMAP_LEN];
static mut PF_COUNT: u64 = 0;

/// 页码表容器: 必须 4096 对齐 (页表基址低 12 位=标志位, 未对齐会报保留位 #PF)。
#[repr(C, align(4096))]
struct PtAligned([u64; 512]);

/// 堆区替换 PT (每张 512 项, 初始 P=0 -> 首写触发按需零页)。
static mut PT_HEAP0: PtAligned = PtAligned([0; 512]); // 0x800000..0xA00000
static mut PT_HEAP1: PtAligned = PtAligned([0; 512]); // 0xA00000..0xC00000

unsafe fn write_cr3_flush() {
    let cr3 = read_cr3();
    core::arch::asm!(
        "mov {}, cr3",
        in(reg) cr3,
        options(nomem, nostack, preserves_flags)
    );
}

/// M12 初始化: 把恒等映射的 PD[4]/PD[5] 替换为内核 PT (P=0), 使堆区按需物理帧。
pub fn demand_zero_init() {
    unsafe {
        let pm4 = read_cr3() as *mut u64;
        let pdpt = (pm4.read() & 0x000F_FFFF_FFFF_F000) as *mut u64;
        let pd = (pdpt.read() & 0x000F_FFFF_FFFF_F000) as *mut u64;
        // PD 索引: VA 0x800000>>21 = 4 (PT_HEAP0), 0xA00000>>21 = 5 (PT_HEAP1)
        let old0 = pd.add(4).read();
        let old1 = pd.add(5).read();
        pd.add(4).write(core::ptr::addr_of_mut!(PT_HEAP0) as u64 | 0x7); // P|W|U
        pd.add(5).write(core::ptr::addr_of_mut!(PT_HEAP1) as u64 | 0x7);
        write_cr3_flush();
        serial::write_str("m12  : demand-zero heap PD[4/5] replaced (old ");
        print_hex(old0 & 0xFFF);
        serial::write_str("/");
        print_hex(old1 & 0xFFF);
        serial::write_line(") - zero-on-first-write armed");
    }
}

/// 分配并清零一页物理帧 (返回物理地址; 恒等映射内核可写)。
fn frame_alloc_zero() -> Option<u64> {
    unsafe {
        for i in 0..FRAME_PAGES {
            let byte = i / 8;
            let bit = i % 8;
            let cur = core::ptr::read_volatile(core::ptr::addr_of!(FRAME_BITMAP[byte]));
            if cur & (1 << bit) == 0 {
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!(FRAME_BITMAP[byte]),
                    cur | (1 << bit),
                );
                let phys = FRAME_BASE + (i as u64) * 0x1000;
                // 清零 (内核直写)
                for k in 0..512usize {
                    ((phys as *mut u64).add(k)).write(0);
                }
                return Some(phys);
            }
        }
    }
    None
}

/// 按需零页: 用户堆区首写 -> 分配零帧 -> 置 PTE -> invlpg -> true (iretq 重试)。
fn demand_zero(cr2: u64) -> bool {
    unsafe {
        if cr2 < USER_HEAP_BASE || cr2 >= USER_HEAP_BASE + USER_HEAP_LEN {
            return false;
        }
        let off = (cr2 - USER_HEAP_BASE) as usize;
        let pt_idx = off >> 12; // 0..1023
        let (table, idx) = if pt_idx < 512 {
            (core::ptr::addr_of_mut!(PT_HEAP0).cast::<u64>(), pt_idx)
        } else {
            (core::ptr::addr_of_mut!(PT_HEAP1).cast::<u64>(), pt_idx - 512)
        };
        let pte_addr = (table as *mut u64).add(idx);
        if core::ptr::read_volatile(pte_addr) & 1 != 0 {
            return false; // 已映射 (不应发生)
        }
        let phys = match frame_alloc_zero() {
            Some(p) => p,
            None => {
                serial::write_line("mem  : out of frames (demand-zero)");
                return false;
            }
        };
        core::ptr::write_volatile(pte_addr, phys | 0x7); // P|W|U
        core::arch::asm!(
            "invlpg [{0}]",
            in(reg) cr2,
            options(nomem, nostack, preserves_flags)
        );
        PF_COUNT += 1;
        if PF_COUNT <= 8 || PF_COUNT % 1024 == 0 {
            serial::write_str("m12  : demand-zero va=0x");
            print_hex(cr2);
            serial::write_str(" -> phys=0x");
            print_hex(phys);
            serial::write_str(" (frames=");
            print_dec(PF_COUNT);
            serial::write_line(")");
        }
        true
    }
}

/// #PF 处理 (asm 桩 fujo_pf_stub 调用; regs 布局见桩注释)。
/// 用户堆区首写 -> 按需零页 -> 返回 (桩 iretq 重试原指令, 进程继续);
/// 未处理 -> 诊断 + 停机 (与旧行为一致, 但带 cr2/err/rip)。
#[no_mangle]
pub extern "C" fn fujo_pf_handler(_vec: u64, regs: *const u64) {
    unsafe {
        let err = regs.add(9).read();
        let rip = regs.add(10).read();
        let mut cr2: u64;
        core::arch::asm!(
            "mov {}, cr2",
            out(reg) cr2,
            options(nomem, nostack, preserves_flags)
        );
        let user = (err >> 2) & 1 == 1;
        let write = (err >> 1) & 1 == 1;
        let present = err & 1 == 1;
        if user && write && !present && demand_zero(cr2) {
            return; // 桩: pop 寄存器 + iretq -> 重试原指令
        }
        // M14: 用户态进程级崩溃隔离 —— 多任务时终止当前任务并切换幸存者
        // (需求页之外的用户 #PF, 如空指针写); 单任务保持停机诊断。
        if user && crate::sched::terminate_current_and_next() {
            return; // 桩: 检测 pf_must_switch -> 转场到幸存任务帧
        }
        serial::write_str("m12  : UNHANDLED #PF cr2=0x");
        print_hex(cr2);
        serial::write_str(" err=0x");
        print_hex(err);
        serial::write_str(" rip=0x");
        print_hex(rip);
        serial::write_line(" - kernel halted");
        loop {
            crate::hlt();
        }
    }
}
