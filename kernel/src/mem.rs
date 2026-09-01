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

/// M108: 高地址映射 PT (0x1000000..0x1080000, 恒等 P=1 U=1) —
/// 窗口程序 (user-high.ld) 装载区, 与桌面代理 0x400000 共存。
#[repr(C, align(4096))]
pub static mut PT_HIGH: PtAligned = PtAligned([0; 512]);
static mut HIGH_MAPPED: bool = false;

/// M108: 一次性映射高址 2MiB (物理恒等, U=1)。
pub fn map_high_user() {
    unsafe {
        if HIGH_MAPPED {
            return;
        }
        let pm4 = read_cr3() as *mut u64;
        let pdpt = (pm4.read() & 0x000F_FFFF_FFFF_F000) as *mut u64;
        let pd = (pdpt.read() & 0x000F_FFFF_FFFF_F000) as *mut u64;
        let old = pd.add(8).read(); // 0x1000000>>21 = 8
        pd.add(8).write(core::ptr::addr_of_mut!(PT_HIGH) as u64 | 0x7);
        for i in 0..512usize {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!(PT_HIGH.0[i]),
                (0x100_0000u64 + (i as u64) * 0x1000) | 0x7,
            );
        }
        write_cr3_flush();
        HIGH_MAPPED = true;
        serial::write_str("m108 : high user map PD[8] (old ");
        print_hex(old & 0xFFF);
        serial::write_line(") - 2MiB U=1 armed");
    }
}

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
fn frame_alloc_zero() -> Option<u64> {    unsafe {
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
fn demand_zero(cr2: u64) -> bool {    unsafe {
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

// ---------------------------------------------------------------------------
// M86 · 权重 mmap 对象: 模型权重入资源 (可作 .run 资源节), 内核按需页
// ---------------------------------------------------------------------------

/// 权重库区 (backbuffer 0xF00000 后; M66 缓存 0xF10000..0xF28000 之后)。
const WLIB_BASE: u64 = 0xF30000;
const WLIB_CAP: u64 = 0x2000; // 8KiB 权重库
const WMAP_MAX: usize = 2;

static mut WLIB_LEN: u64 = 0;
static mut WMAP_VA: [u64; WMAP_MAX] = [0; WMAP_MAX];
static mut WMAP_LEN: [u64; WMAP_MAX] = [0; WMAP_MAX];
static mut WMAP_ON: [bool; WMAP_MAX] = [false; WMAP_MAX];
static mut WPF: u64 = 0; // 权重页按需装入次数
static mut W_PAGES: u64 = 0;

/// 0x7C01: 载入权重库 (拷贝到 WLIB)。
pub fn fujo_wmap_load(ptr: u64, len: u64) -> i64 {
    unsafe {
        let m = len.min(WLIB_CAP);
        for i in 0..m {
            // 恒等映射区: 虚拟=物理
            ((WLIB_BASE as *mut u8).add(i as usize)).write((ptr as *const u8).add(i as usize).read());
        }
        WLIB_LEN = m;
    }
    0
}

/// 0x7C02: 登记权重映射区 (va 须在需求段 0x800000..0xC00000, 页对齐)。
pub fn fujo_wmap_res(va: u64, len: u64) -> i64 {
    unsafe {
        if va < USER_HEAP_BASE || va + len > USER_HEAP_BASE + USER_HEAP_LEN {
            return -22; // -EINVAL
        }
        if len > WLIB_LEN || len > WLIB_CAP {
            return -12; // -ENOMEM
        }
        let mut slot = None;
        for i in 0..WMAP_MAX {
            if !WMAP_ON[i] {
                slot = Some(i);
                break;
            }
        }
        match slot {
            Some(i) => {
                WMAP_VA[i] = va;
                WMAP_LEN[i] = len;
                WMAP_ON[i] = true;
                serial::write_str("m86  : weight-map va=0x");
                print_hex(va);
                serial::write_str(" len=");
                print_dec(len);
                serial::write_line(" (demand pages)");
            }
            None => return -12,
        }
    }
    0
}

/// 0x7C03: (pfa, pages, wlen, maps)。
pub fn fujo_wmap_stats(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(WPF);
        w.add(1).write(W_PAGES);
        w.add(2).write(WLIB_LEN);
        let mut maps = 0u64;
        for i in 0..WMAP_MAX {
            if WMAP_ON[i] {
                maps += 1;
            }
        }
        w.add(3).write(maps);
    }
    0
}

/// #PF 钩子 (demand_zero 之前): 权重映射区 → 从 WLIB 拷贝页。
fn wmap_fault(cr2: u64) -> bool {
    unsafe {
        for i in 0..WMAP_MAX {
            if !WMAP_ON[i] {
                continue;
            }
            let va = WMAP_VA[i];
            let len = WMAP_LEN[i];
            if cr2 >= va && cr2 < va + len {
                let off = (cr2 - va) as usize;
                let pt_idx = off >> 12; // 逐页
                let page_no = ((cr2 & !0xFFF) - va) as usize; // 页序
                // PTE 表定位 (同 demand_zero: 0x800000..0xC00000=PT_HEAP0/1)
                let base_idx = (cr2 - USER_HEAP_BASE) as usize >> 12;
                let (table, idx) = if base_idx < 512 {
                    (core::ptr::addr_of_mut!(PT_HEAP0).cast::<u64>(), base_idx)
                } else {
                    (core::ptr::addr_of_mut!(PT_HEAP1).cast::<u64>(), base_idx - 512)
                };
                let pte_addr = (table as *mut u64).add(idx);
                if core::ptr::read_volatile(pte_addr) & 1 != 0 {
                    return false;
                }
                let phys = frame_alloc_zero();
                match phys {
                    Some(p) => {
                        // 从 WLIB 拷贝该页 (src = WLIB_BASE + page_no*4096)
                        for k in 0..512usize {
                            ((p as *mut u64).add(k)).write(
                                ((WLIB_BASE + (page_no as u64) * 0x1000) as *const u64).add(k).read(),
                            );
                        }
                        core::ptr::write_volatile(pte_addr, p | 0x7);
                        core::arch::asm!(
                            "invlpg [{0}]",
                            in(reg) cr2,
                            options(nomem, nostack, preserves_flags)
                        );
                        WPF += 1;
                        W_PAGES += 1;
                        if WPF <= 8 {
                            serial::write_str("m86  : wmap page va=0x");
                            print_hex(cr2 & !0xFFF);
                            serial::write_line(" (from weight lib)");
                        }
                        let _ = pt_idx;
                        return true;
                    }
                    None => return false,
                }
            }
        }
    }
    false
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
        // M20 fix: demand-zero 也处理"读未分配页" (err=present=0, user, read)
        // —— Linux 语义: 匿名映射未写页被读也应返回零页; 原实现仅覆盖 write,
        // 导致 B 在 A 写共享页之前读 -> 误判致命崩溃 (M18 回归实证)。
        let _ = write;
        // M86: 权重映射区按需页 (demand-zero 前)
        if user && !present && wmap_fault(cr2) {
            return;
        }
        if user && !present && demand_zero(cr2) {
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
