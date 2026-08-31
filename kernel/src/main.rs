//! fujo-kernel 主模块（M0）
//!
//! 引导路径:  QEMU (multiboot v1) -> boot_blob.bin 32 位桩
//!            -> 建 1 GiB 恒等页表 -> long mode -> rust64_entry
//! 本文件:    VGA/串口双通道输出启动日志、多引导信息解析、ABI 表汇报。
//!
//! M1 升级点: 高位半区 + 5 级页表、IDT/PIT/SMP、syscall gate、
//!            IPC/内存服务、VFS/图形服务用户态化。

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

mod gdt;
mod interrupts;
mod serial;
mod syscall;
mod vga;

use core::arch::asm;

const MB_MAGIC: u32 = 0x2BAD_B002;

/// 停机等待中断。
/// 注意: 不能声明 nomem —— 否则 LLVM 会把循环内的静态读提升出循环,
/// 导致 "while ticks==0 {hlt}" 变成永不重读的死循环 (M1 实际踩过的坑)。
#[inline]
pub fn hlt() {
    unsafe { asm!("hlt", options(nostack, preserves_flags)) }
}

// ---------------------------------------------------------------------------
// 多引导 v1 头：必须位于镜像文件前 8 KiB。
// flags = 0x10003: 显式地址(meminfo + 模块对齐)，不请求视频 -> QEMU 保持 VGA 文本模式。
// 加载范围 0x100000..0x207000 (0x107000 字节)，入口 0x101000 (引导桩)。
// ---------------------------------------------------------------------------
#[repr(C, align(4))]
struct MultibootHeader {
    magic: u32,
    flags: u32,
    checksum: u32,
    header_addr: u32,
    load_addr: u32,
    load_end_addr: u32,
    bss_end_addr: u32,
    entry_addr: u32,
}

#[used]
#[link_section = ".multiboot"]
static MB_HEADER: MultibootHeader = MultibootHeader {
    magic: 0x1BAD_B002,
    flags: 0x0001_0003,
    checksum: 0xE451_4FFB,
    header_addr: 0x0010_0000,
    load_addr: 0x0010_0000,
    // 注意: 必须覆盖整个镜像(rodata/data/bss)。内核增长时要同步扩大,
    // 否则尾部段不被加载 -> 字符串/内嵌二进制为垃圾 RAM (M1 踩坑实录)。
    load_end_addr: 0x0021_0000,
    bss_end_addr: 0x0021_0000,
    entry_addr: 0x0010_1000,
};

// ---------------------------------------------------------------------------
// 引导桩 + 页表（boot/gen_stub32.py 生成）。位于物理 0x101000，与 kernel.ld 一致。
// ---------------------------------------------------------------------------
#[used]
#[link_section = ".boot_blob"]
static BOOT_BLOB: [u8; 0xF030] = *include_bytes!("../boot_blob.bin");

/// ELF 入口占位（真正入口是引导桩 far-jump 的 rust64_entry）。
#[no_mangle]
pub extern "C" fn _start() {}

/// cpuid leaf 0（global_asm: x86-64 的 rbx 内联操作数受 LLVM 限制，绕开）。
// NOTE: 用普通注释而非文档注释（宏调用语句不能带文档注释）
core::arch::global_asm!(r#"
    .text
    .global fujo_cpuid_leaf0
    .p2align 4
    .type fujo_cpuid_leaf0, @function
fujo_cpuid_leaf0:
    push rbx
    push rcx
    push rdx
    mov rax, 0
    xor rcx, rcx
    cpuid
    mov [rdi + 0], eax
    mov [rdi + 4], ebx
    mov [rdi + 8], edx
    mov [rdi + 12], ecx
    pop rdx
    pop rcx
    pop rbx
    ret
    .size fujo_cpuid_leaf0, . - fujo_cpuid_leaf0
"#);

extern "C" {
    fn fujo_cpuid_leaf0(buf: *mut u32);
}

struct SerialWriter;

impl core::fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        serial::write_str(s);
        Ok(())
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write as _;
    serial::write_line("[PANIC] fujo-kernel");
    let msg = info.message();
    let mut w = SerialWriter;
    let _ = core::write!(w, "        message: {msg}\n");
    loop {
        crate::hlt();
    }
}

// ---------------------------------------------------------------------------
// 长模式 Rust 入口（0x08:0x200000）
// ---------------------------------------------------------------------------
#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn rust64_entry(magic: u32, mbi: u32) -> ! {
    vga::init();
    serial::init();
    banner(magic, mbi);

    // ---- M1: 内核芯 ----
    gdt::init();
    interrupts::init();
    syscall::setup();
    out_line("m1   : gdt(user segs+tss) / idt(15 exc + irq0) / syscall gate armed");
    out_raw("m1   : sti, waiting first PIT tick...");
    unsafe { asm!("sti", options(nomem, nostack, preserves_flags)); }
    while interrupts::ticks() == 0 {
        hlt();
    }
    out_line(" timer IRQ0 alive (tick=1)");

    // 证明定时器稳定: 再等 100 tick (~1s)
    let t0 = interrupts::ticks();
    while interrupts::ticks() - t0 < 100 {
        hlt();
    }
    out_line("timer : 100 ticks = 1.0 s elapsed (PIT @100 Hz)");

    // ---- M1: 用户态测试程序 (linux-x64 ABI, 原生 syscall) ----
    unsafe {
        out_raw("test : PD[2]=");
        out_hex_u32((core::ptr::read((0x104000usize as *const u64).add(2)) as u32));
        out_raw(" PT2[0]=");
        out_hex_u32((core::ptr::read(0x10A000usize as *const u64)) as u32);
        out_raw(" tss.rsp0=");
        out_hex_u32(gdt::debug_tss_rsp0() as u32);
        out_line("");
    }
    syscall::enter_user_test();
}

// ---------------------------------------------------------------------------
// 启动日志
// ---------------------------------------------------------------------------

fn banner(magic: u32, mbi: u32) {
    // —— 标题（绿色） ——
    vga::set_color(0x0A);
    out_line("FujoOS 0.1.0-dev");
    vga::set_color(0x07);

    // —— 引导信息 ——
    out_raw("boot  : multiboot-v1 magic=");
    out_hex_u32(magic);
    out_line(if magic == MB_MAGIC { " [ok]" } else { " [BAD]" });

    // CPU 厂商
    let vendor = cpuid_vendor();
    out_raw("cpu   : vendor=");
    out_raw(core::str::from_utf8(&vendor).unwrap_or("unknown"));
    out_raw("\n");

    // 内存（BIOS 上报）
    let info = unsafe { parse_mbi(mbi) };
    match info {
        Some(m) => {
            out_raw("mem   : lower=");
            out_dec_u32(m.mem_lower_kb);
            out_raw(" KiB, upper=");
            out_dec_u32(m.mem_upper_kb);
            out_raw(" KiB");
            if m.flags & 0x40 != 0 && m.mmap_addr != 0 {
                let (count, total) = unsafe { sum_mmap(m.mmap_addr, m.mmap_len) };
                out_raw("  | mmap: ");
                out_dec_u32(count);
                out_raw(" regions, usable ");
                out_dec_u64(total / (1024 * 1024));
                out_line(" MiB");
            } else {
                out_line("");
            }
        }
        None => out_line("mem   : mbi not present"),
    }

    // —— ABI 兼容层 ——
    out_raw("abi   : linux-x64 ");
    out_dec_u64(syscall::linux_x64_count() as u64);
    out_line(" syscalls [first-class ABI]");
    out_raw("abi   : darwin-x64 ");
    out_dec_u64(syscall::darwin_x64_count() as u64);
    out_line(" bsd syscalls + mach-traps shim [M6]");
    out_line("abi   : win32 shim modules: ntdll/kernel32/user32/gdi32/ws2_32 [M3]");
    out_line("compat: PE | ELF | Mach-O -> FUJR .run container (fujo-compat v0.1)");

    // —— 状态 ——
    out_line("sched : boot CPU online (SMP in M1)");
    out_line("ready : FujoOS kernel up; idle loop engaged");
    out_line("");
}

fn out_raw(s: &str) {
    vga::write_str(s);
    serial::write_str(s);
}

fn out_line(s: &str) {
    vga::write_line(s);
    serial::write_line(s);
}

fn out_hex_u32(v: u32) {
    out_raw("0x");
    let mut buf = [0u8; 8];
    for i in 0..8 {
        let d = ((v >> (4 * i)) & 0xF) as u8;
        buf[7 - i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
    }
    out_raw(core::str::from_utf8(&buf).unwrap());
}

fn out_dec_u32(mut v: u32) {
    let mut buf = [0u8; 16];
    let mut i = 16;
    if v == 0 {
        out_raw("0");
        return;
    }
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    out_raw(core::str::from_utf8(&buf[i..]).unwrap());
}

fn out_dec_u64(mut v: u64) {
    let mut buf = [0u8; 24];
    let mut i = 24;
    if v == 0 {
        out_raw("0");
        return;
    }
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    out_raw(core::str::from_utf8(&buf[i..]).unwrap());
}

// ---------------------------------------------------------------------------
// CPUID / 多引导信息
// ---------------------------------------------------------------------------

fn cpuid_vendor() -> [u8; 12] {
    let mut res = [0u32; 4];
    unsafe { fujo_cpuid_leaf0(res.as_mut_ptr()) }
    let mut v = [0u8; 12];
    v[0..4].copy_from_slice(&res[1].to_le_bytes());
    v[4..8].copy_from_slice(&res[2].to_le_bytes());
    v[8..12].copy_from_slice(&res[3].to_le_bytes());
    v
}

struct MbInfo {
    flags: u32,
    mem_lower_kb: u32,
    mem_upper_kb: u32,
    mmap_len: u32,
    mmap_addr: u32,
}

/// 解析 multiboot v1 info 结构（偏移规范见 GRUB 文档）。
unsafe fn parse_mbi(ptr: u32) -> Option<MbInfo> {
    if ptr == 0 {
        return None;
    }
    let p = ptr as *const u32;
    let flags = p.read();
    let mut m = MbInfo {
        flags,
        mem_lower_kb: 0,
        mem_upper_kb: 0,
        mmap_len: 0,
        mmap_addr: 0,
    };
    if flags & 1 != 0 {
        m.mem_lower_kb = p.add(1).read();
        m.mem_upper_kb = p.add(2).read();
    }
    if flags & 0x40 != 0 {
        m.mmap_len = p.add(11).read();
        m.mmap_addr = p.add(12).read();
    }
    Some(m)
}

/// 遍历 mmap 条目（entry size 为自身长度减 4），统计可用内存。
unsafe fn sum_mmap(addr: u32, len: u32) -> (u32, u64) {
    let mut count = 0u32;
    let mut total: u64 = 0;
    let mut off = addr;
    let end = addr.wrapping_add(len);
    while off < end {
        let p = off as *const u8;
        let size = (p as *const u32).read();
        if size == 0 {
            break;
        }
        let base = (p.add(4) as *const u64).read();
        let length = (p.add(12) as *const u64).read();
        let typ = (p.add(20) as *const u32).read();
        let _ = base;
        if typ == 1 {
            total += length;
        }
        count += 1;
        off = off.wrapping_add(size + 4);
    }
    (count, total)
}
