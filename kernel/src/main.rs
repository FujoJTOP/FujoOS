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

mod ai;
mod a11y;
mod ahci;
mod acpi;
mod virtio;
mod net;
mod ata;
mod audio;
mod asm;
mod blit;
mod capability;
mod clip;
mod ctx;
mod dbg;
mod desk;
mod dxwrap;
mod display;
mod dump;
mod editor;
mod elf_loader;
mod fjfs;
mod font;
mod fujocc;
mod fujr;
mod gamemode;
mod game2;
mod gdi;
mod gl;
mod hw;
mod hvm;
mod gdt;
mod graphics;
mod interrupts;
mod ipc;
mod irq;
mod icon;
mod ime;
mod infer;
mod installer;
mod kbd;
mod kobj;
mod ld;
mod leak;
mod macho_loader;
mod mem;
mod modelcard;
mod modelreg;
mod mouse;
mod platform;
mod wmsg;
mod xinput;
mod pe_loader;
mod pcache;
mod perf;
mod save;
mod sched;
mod serial;
mod sessions;
mod shader;
mod shell;
mod smp;
mod syscall;
mod term;
mod timer;
mod utest;
mod upd;
mod vfs;
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
// flags = 0x10003: 显式地址(meminfo + 模块对齐)，**不请求视频** (W20 A/B 实测:
// QEMU 9.2 TCG -kernel 拒绝视频请求 -> 内核 0 输出, 参考机主路径必须无 bit2;
// GRUB 真机路径的 mbi framebuffer 接收代码保留 (mbi bit12 一旦出现即接管)。
// 视频请求双变体: scripts/ 记录 (docs/74 §4b) —— 真机镜像以 --video 标志
// 生成 flags=0x10007 变体 (GRUB 会提供 framebuffer)。
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
    // flags bit2 置位时以下 4 字段生效 (视频请求); 默认不动 (保持 8 字段头)
    mode_type: u32,
    width: u32,
    height: u32,
    depth: u32,
}

#[used]
#[link_section = ".multiboot"]
static MB_HEADER: MultibootHeader = MultibootHeader {
    magic: 0x1BAD_B002,
    flags: 0x0001_0003,
    checksum: 0xE451_4FFB,    header_addr: 0x0010_0000,
    load_addr: 0x0010_0000,
    // 注意: 必须覆盖整个镜像(rodata/data/bss)。内核增长时要同步扩大,
    // 否则尾部段不被加载 -> 字符串/内嵌二进制为垃圾 RAM (M1 踩坑实录);
    // M15 又踩: 镜像 1.18MB 已超 0x120000 -> 尾部覆盖引导模块区 (bad magic);
    // M20 再踩: 镜像 1.38MB=0x151D18, 0x100000+0x151D18=0x251D18 > 0x230000
    //   -> 模块区(load_end 之后)与内核尾部重叠, ELF 头被部分覆盖。
    // 当前覆盖到 0x2C0000 (W35 回滚: load_end>2C0000 使 initrd 模块顶入 0x400000
    // 用户区与 tcc 加载冲突 -> m128 同址 GP; 约束: 0x100000 + flatten --pad 值)。
    load_end_addr: 0x002C_0000,
    bss_end_addr: 0x002C_0000,
    entry_addr: 0x0010_1000,
    mode_type: 0,
    width: 1024,
    height: 768,
    depth: 32,
};

// ---------------------------------------------------------------------------
// 引导桩 + 页表（boot/gen_stub32.py 生成）。位于物理 0x101000，与 kernel.ld 一致。
// ---------------------------------------------------------------------------
#[used]
#[link_section = ".boot_blob"]
static BOOT_BLOB: [u8; 0x33040] = *include_bytes!("../boot_blob.bin");

/// ELF 入口占位（真正入口是引导桩 far-jump 的 rust64_entry）。
#[no_mangle]
pub extern "C" fn _start() {}

/// cpuid leaf 0（global_asm: x86-64 的 rbx 内联操作数受 LLVM 限制，绕开）。
// NOTE: 用普通注释而非文档注释（宏调用语句不能带文档注释）
core::arch::global_asm!(r#"
    .text
    .global fujo_cpuid_leaf0
    .p2align 4
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
    if let Some(loc) = info.location() {
        let mut w2 = SerialWriter;
        let _ = core::write!(
            w2,
            "        at {}:{}:{}\n",
            loc.file(),
            loc.line(),
            loc.column()
        );
    }
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
    // W30: 加载器 IF 状态无关化 —— GRUB/ISO 交付时 IF 可能为开; 中断管理由
    // interrupts::init+sti 全权负责 (启动早期提前 IRQ 会以错码/时序崩溃)。
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack, preserves_flags));
        // W31 #17 根因修复: 引导期 SS=GRUB 交付值 (0x18=udata); 长模式分段基址
        // 忽略, SS 失效不影响数据访问, 但首个 iretq 复查 SS.RPL(3) vs CS.RPL(0)
        // 矛盾 -> #GP(err=0x18)。显式装载内核数据段。
        core::arch::asm!("mov ax, 0x10", "mov ss, ax", options(nomem, nostack, preserves_flags));
    }
    vga::init();
    serial::init();
    banner(magic, mbi);

    // ---- M1: 内核芯 ----
    gdt::init();
    interrupts::init();
    syscall::setup();
    // M11: 开启 SSE (CR4.OSFXSR|OSXMMEXCPT = 0x600) —— 引导桩仅设 PAE (0x20),
    // 用户/内核 -O2 向量化指令 (movups 等) 缺 OSFXSR 直接 #UD (实测 vec=6)。
    unsafe {
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        asm!("mov cr4, {}", in(reg) cr4 | 0x600, options(nomem, nostack, preserves_flags));
    }
    out_line("m1   : cr4=0x600 (sse/fxsave enabled)");
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

    // ---- M4: fujocom 显示栈 (Bochs VBE 1024x768x32 LFB + 双缓冲合成器) ----
    // ---- M5: 输入系统 (PS/2 键盘 IRQ1; 服务就绪, 演示延后到图形层之后) ----
    kbd::init();
    out_line("dbg  : after kbd init");
    // ---- M10: COM2 模型链路 (IRQ3, fujonn engine=qwen) ----
    serial::uart2_init();
    out_line("m10  : com2 model-link up (irq3 @115200) - engine=qwen waits host server");
    out_line("dbg  : before mouse init");

    // ---- M36: PS/2 鼠标 (IRQ12, 命中测试/焦点) ----
    mouse::init();

    // ---- M11/M12: 虚拟内存/堆 (U 位硬化 + 按需零页) ----
    mem::init();
    mem::harden_user_guard();
    mem::demand_zero_init();
    let _mapped = mem::map_high_ram(); // W20 p6: >1GiB 可用内存映射 (≤4GiB)
    out_line("mem  : virtual memory v0 - user heap 0x800000..0xC00000 (brk/mmap ready)");

    // ---- M16: ATA + FJFS 持久卷 (QEMU: -drive file=...,format=raw,if=ide) ----
    ata::init();
    if !crate::ahci::init() && unsafe { !crate::ata::ATA_PRESENT } {
        serial::write_line("disk : no ATA/AHCI device");
    }
    fjfs::init();
    if unsafe { crate::ata::ATA_PRESENT } {
        crate::fjfs::list();
    }

    // ---- M15: VFS 内存文件系统 (挂载前记录模块) ----
    syscall::remember_module(mbi);
    let mb = syscall::boot_module_info(mbi);
    if let Some((addr, len)) = mb {
        vfs::set_boot_module(addr, len);
    }
    vfs::init();

    // ---- M10.1: 启动 Logo (几何徽章, 单一界面) ----
    // M109: 移除 vga::logo 文本徽章叠加 —— 只保留图形层几何徽章
    // (避免"两个启动界面"叠影; 文本徽章在 0xFD000000 未投映时无意义)。
    serial::write_line("logo : boot splash (geometry badge)");
    let t0 = interrupts::ticks();
    while interrupts::ticks().wrapping_sub(t0) < 100 {}
    // 文本徽章展示完毕 -> 切图形层画几何徽章
    let gfx_ok = graphics::init();
    crate::smp::init(); // M64: CPUID 核探测 + 亲和/均衡统计就位
    crate::pcache::init(); // M66: 页缓存/模拟盘清零
    crate::perf::init(); // M68: 计时校准 + 性能计数器默认面
    crate::editor::selftest(); // M73: 迷你编辑器就绪
    crate::utest::init(); // M82: 单元测试套件注册
    crate::acpi::scan_all(); // M96: PCI 枚举 (真机引导最小集)
    let vblk = crate::virtio::init(); // W13: virtio-blk (参考机可复现)
    if vblk {
        out_line("vblk : virtio-blk ready (legacy PCI, polled vring)");
    }
    crate::net::init(); // W14: virtio-net (参考机 QEMU user-net)
    crate::smp::ap_bringup(); // W17: SMP AP 启动 (仅 -smp 2; SIPI@0x8000)
    crate::ai::ai_aud_boot(); // W19: AI 审计 boot 标记 (统一审计环确定性非空)
    if !gfx_ok {
        out_line("gfx  : framebuffer unavailable (VBE not present), geometry logo skipped");
    } else {
        graphics::logo_hex();
        kbd::demo(); // 依赖 fb() 的 M5 演示在此执行
    }
    serial::write_line("");

    // ---- 引导路由 ----
    // W16: FUJOMULT 多模块解析前置 (注册表在 shell 阶段可见; 原在首次 os run 时才解析)
    syscall::parse_multi_early();
    // M108: boot 模块 = m108_desk.elf -> 用户态桌面代理 (自驱动; 无命令注入)
    if syscall::boot_module_is_desk_proxy() {
        crate::sched::set_proxy_mode();
        syscall::enter_user_test(mbi); // > !: 不再返回
    }
    // W30: autostart (GRUB/ISO cmdline `fujo.run=<demo>`, 模块名匹配则直启;
    // 真机无 monitor sendkey —— 与 m108 同直启路径)
    if syscall::boot_autostart(mbi) {
        serial::write_line("boot: autostart (cmdline fujo.run) -> direct launch");
        syscall::enter_user_test(mbi); // > !: 不再返回
    }
    // 其他模块: os shell (经典注入路径; fujoci/fujoregress 兼容)
    if syscall::boot_module_present() {
        crate::shell::shell(mbi); // > !: 不再返回
    }
    // 无模块: M107 内核态桌面 (boot 直接进图形桌面; 双击图标开窗口程序)
    crate::desk::desktop_main(mbi); // > ! (无命令注入依赖)

    // ---- 不可达: M2/M3/M6 用户态测试 (shell 之前的直启路径已废止) ----
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
            }            // W20: 引导器交付 framebuffer (GRUB 真机 / QEMU 视频请求)
            if m.flags & 0x1000 != 0 {
                out_raw("fb    : mbi framebuffer addr=0x");
                out_hex_u64(m.fb_addr);
                out_raw(" pitch=");
                out_dec_u32(m.fb_pitch);
                out_raw(" ");
                out_dec_u32(m.fb_width);
                out_raw("x");
                out_dec_u32(m.fb_height);
                out_raw(" bpp=");
                out_dec_u32(m.fb_bpp as u32);
                out_raw(" type=");
                out_dec_u32(m.fb_type as u32);
                out_line("");
                crate::graphics::note_mbi_fb(m.fb_addr, m.fb_pitch, m.fb_width, m.fb_height, m.fb_bpp, m.fb_type);
            } else {
                out_line("fb    : no mbi framebuffer (VGA text fallback)");
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

fn out_hex_u64(v: u64) {
    out_raw("0x");
    let mut buf = [0u8; 16];
    for i in 0..16 {
        let d = ((v >> (4 * i)) & 0xF) as u8;
        buf[15 - i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
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
    /// W20: framebuffer (mbi flags bit12; 引导器交付的 LFB 描述)
    fb_addr: u64,
    fb_pitch: u32,
    fb_width: u32,
    fb_height: u32,
    fb_bpp: u8,
    fb_type: u8,
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
        fb_addr: 0,
        fb_pitch: 0,
        fb_width: 0,
        fb_height: 0,
        fb_bpp: 0,
        fb_type: 0,
    };
    if flags & 1 != 0 {
        m.mem_lower_kb = p.add(1).read();
        m.mem_upper_kb = p.add(2).read();
    }
    if flags & 0x40 != 0 {
        m.mmap_len = p.add(11).read();
        m.mmap_addr = p.add(12).read();
    }
    if flags & 0x1000 != 0 {
        // framebuffer: +23 u64 addr, +25 pitch, +26 w, +27 h, +28 bpp u8, +29 type u8
        m.fb_addr = (p.add(23).read() as u64) | ((p.add(24).read() as u64) << 32);
        m.fb_pitch = p.add(25).read();
        m.fb_width = p.add(26).read();
        m.fb_height = p.add(27).read();
        m.fb_bpp = (p.add(28) as *const u8).read();
        m.fb_type = (p.add(29) as *const u8).read();
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
            crate::mem::note_mmap(base, length); // W20 p6: 持久化可用区
        }
        count += 1;
        off = off.wrapping_add(size + 4);
    }
    (count, total)
}
