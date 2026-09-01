//! interrupts.rs — IDT / PIC 重映射 / PIT 定时器 (M1)
//!
//! 功能: 15 个异常向量(0..14, 含 #PF) + IRQ0(PIT 100Hz) 实时计数。
//! 说明: 内核中断栈由 TSS.rsp0 自动切换（用户态中断同样安全）。

use core::arch::asm;

use crate::serial;

pub const PIT_HZ: u64 = 100;
const PIT_DIVISOR: u16 = 11932; // 1193182 / ~100Hz

/// 计时器计数 (global_asm 直接引用此符号)
#[no_mangle]
pub static mut pit_ticks: u64 = 0;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct IdtEntry {
    off_lo: u16,
    sel: u16,
    ist: u8,
    attr: u8,
    off_mid: u16,
    off_hi: u32,
    zero: u32,
}

impl IdtEntry {
    const fn empty() -> Self {
        IdtEntry { off_lo: 0, sel: 0, ist: 0, attr: 0, off_mid: 0, off_hi: 0, zero: 0 }
    }
    unsafe fn set(&mut self, handler: u64, sel: u16, attr: u8) {
        self.off_lo = handler as u16;
        self.sel = sel;
        self.ist = 0;
        self.attr = attr;
        self.off_mid = (handler >> 16) as u16;
        self.off_hi = (handler >> 32) as u32;
        self.zero = 0;
    }
}

#[repr(C, packed)]
struct IdtPtr {
    limit: u16,
    base: u64,
}

static mut IDT_PTR: IdtPtr = IdtPtr { limit: 0, base: 0 };

const IDT_SIZE: usize = 0x2D; // 0..0x2C (异常 + IRQ0/1/3 + IRQ12 鼠标 M36)

static mut IDT: [IdtEntry; IDT_SIZE] = [IdtEntry::empty(); IDT_SIZE];

extern "C" {
    fn fujo_exc_stub_table();
    fn fujo_pit_stub();
    fn fujo_kbd_stub();
    fn fujo_ser2_stub();
    fn fujo_pf_stub();
    fn fujo_ms_stub();
    fn fujo_dbg_stub();
    fn fujo_bp_stub();
}

core::arch::global_asm!(r#"
    .text
    # ---- M75: #DB (向量 1) 调试桩: 保存现场 -> fujo_dbg_exc(vec, regs) ----
    .p2align 4
    .global fujo_dbg_stub
fujo_dbg_stub:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    mov rdi, 1
    mov rsi, rsp
    call fujo_dbg_exc
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq

    # ---- M75: #BP (向量 3, int3 软件断点) —— 记录 + rip-1 + 恢复原字节 ----
    .p2align 4
    .global fujo_bp_stub
fujo_bp_stub:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    mov rdi, 3
    mov rsi, rsp
    call fujo_dbg_bp_exc
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq

    # ---- 异常桩: 每个桩固定 14 字节: mov rdi,N(7B) + call rel32(5B) + ud2(2B) ----
    # (M4 踩坑: jmp 会优化成 rel8 短跳; 不用宏展开 —— 宏+完整重建在工具链上
    #  偶发装配瞬态 (M9 DEV 记录), 手写定长桩为确定性方案)
    .p2align 4
    .global fujo_exc_stub_table
fujo_exc_stub_table:
    mov rdi, 0
    call fujo_exc_c
    ud2
    mov rdi, 1
    call fujo_exc_c
    ud2
    mov rdi, 2
    call fujo_exc_c
    ud2
    mov rdi, 3
    call fujo_exc_c
    ud2
    mov rdi, 4
    call fujo_exc_c
    ud2
    mov rdi, 5
    call fujo_exc_c
    ud2
    mov rdi, 6
    call fujo_exc_c
    ud2
    mov rdi, 7
    call fujo_exc_c
    ud2
    mov rdi, 8
    call fujo_exc_c
    ud2
    mov rdi, 9
    call fujo_exc_c
    ud2
    mov rdi, 10
    call fujo_exc_c
    ud2
    mov rdi, 11
    call fujo_exc_c
    ud2
    mov rdi, 12
    call fujo_exc_c
    ud2
    mov rdi, 13
    call fujo_exc_c
    ud2
    mov rdi, 14
    call fujo_exc_c
    ud2

    # ---- 异常帧捕获 trampoline (M20: 全寄存器保存 + 用户态可转场) ----
    # 保存顺序必须与 PIT/#PF 桩一致: 栈顶=regs[0]=r11 ... regs[8]=rax,
    # 其后 [ERR?][RIP][CS][RFLAGS][RSP?][SS?]。
    # C 返回 1 => 转场 (sched_next_rsp 指向幸存任务帧; 与 #PF 桩同路径),
    # 返回 0 => C 已停机 (不会返回)。
    .p2align 4
    .global fujo_exc_c
fujo_exc_c:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    mov rsi, rsp
    call fujo_exc2
    test rax, rax
    jz 1f
    mov rsp, [rip + sched_next_rsp]
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq
1:
    ud2

    # ---- PIT (IRQ0) —— 计数 + EOI + 调度钩子 (M13: 时间片轮转) ----
    # 全程保存 caller-saved (C 调度器会破坏), 切任务时换 rsp 到新帧。
    .p2align 4
    .global fujo_pit_stub
fujo_pit_stub:
    inc qword ptr [rip + pit_ticks]
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    mov rdi, 0
    mov rsi, rsp
    call fujo_tick_sched
    push rax          # 先存返回值
    mov al, 0x20
    out 0x20, al      # EOI 必须在恢复寄存器前 (mov al 会毁 rax!; M13 现场)
    pop rax
    test rax, rax
    jz 1f
    mov rsp, [rip + sched_next_rsp]
1:
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq

    # ---- 键盘 (IRQ1) —— 调 C 处理 (读 0x60/入环/EOI) ----
    # 必须保存全部 caller-saved 寄存器 (C 函数会破坏 rsi/r8-r11 等),
    # 否则中断返回后主循环寄存器损坏 (M6 踩坑实录: kbd stub 只存
    # rax/rcx/rdx/rdi -> demo 收尾卡死)。
    .p2align 4
    .global fujo_kbd_stub
fujo_kbd_stub:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    call fujo_kbd_irq
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq

    # ---- COM2 (IRQ3) —— 模型链路 RX: C 排空入环 (M10) ----
    # 与键盘桩同一原则: 保存全部 caller-saved (C 会破坏 rsi/r8-r11)。    .p2align 4
    .global fujo_ser2_stub
fujo_ser2_stub:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    call fujo_ser2_irq
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq

    # ---- 鼠标 (IRQ12, 向量 0x2C, M36) —— 3 字节包处理 + 双 EOI ----
    .p2align 4
    .global fujo_ms_stub
fujo_ms_stub:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    call fujo_ms_irq
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq

    # ---- #PF (向量 14) —— M12 按需零页: 保存全部 caller-saved,
    #      C 处理 (fujo_pf_handler) 后 pop 寄存器 + iretq 重试原指令。 ----
    .p2align 4
    .global fujo_pf_stub
fujo_pf_stub:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    mov rdi, 14
    mov rsi, rsp          # regs 帧: [0..8]=r11..rax, [9]=ERR, [10]=RIP, [11]=CS,
    call fujo_pf_handler  #              [12]=RFLAGS, [13]=RSP, [14]=SS
    # M14: 崩溃任务终止 -> 转场到幸存任务帧 (pf_must_switch 由 sched 设置)
    cmp qword ptr [rip + pf_must_switch], 0
    jz 1f
    mov rsp, [rip + sched_next_rsp]
    mov qword ptr [rip + pf_must_switch], 0
    pop r11             # 目标帧 (PIT 保存, 无错误码): 9 寄存器 + iretq
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq
1:
    pop r11             # 本帧 (含错误码): 9 寄存器 + 跳过 err + iretq
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    add rsp, 8            # 跳过错误码
    iretq
"#);

/// 异常处理（C 侧, M20 版）— 多任务时用户态异常不停机, 崩溃隔离转场。
/// 帧布局 (trampoline 传入 regs = 9 寄存器槽顶):
///   regs[0..8]  = r11..rax
///   regs[9]     = 返回地址 (IDT 桩 call fujo_exc_c 压入! 陷阱: CALL 在
///                  CPU 帧与 9 寄存器之间)
///   regs[10..]  = CPU 帧: 有错误码 (8,10,11,12,13,14,17):
///                 [ERR][RIP][CS][RFLAGS][RSP?][SS?]
///                 无错误码: [RIP][CS][RFLAGS][RSP?][SS?]
/// 用户态 (特权切换) 才压 RSP/SS; 内核态无。
/// 返回 1 = 已转场 (桩换 sched_next_rsp 恢复幸存任务), 0 = 停机 (不返回)。
#[no_mangle]
pub extern "C" fn fujo_exc2(vec: u64, regs: *mut u64) -> i64 {
    let has_err = matches!(vec, 8 | 10 | 11 | 12 | 13 | 14 | 17);
    let e = if has_err { 1 } else { 0 };
    unsafe {
        let cs = regs.add(10 + e + 1).read() as u16; // [9]=ret, [10+e]=RIP, [10+e+1]=CS
        // 判定: 用户态 (CPL3) 且多任务 -> 崩溃隔离转场 (M14 路径)
        if cs & 3 == 3 {
            serial::write_str("EXC  user vec=");
            print_exc_dec(vec);
            serial::write_str(" cs=");
            crate::syscall::log_hex(cs as u64 & 0xFF);
            let rip = regs.add(10 + e).read();
            serial::write_str(" rip=");
            crate::syscall::log_hex(rip);
            // M84: 崩溃转储 (minidump, 隔离转场/停机前捕获)
            crate::dump::note_exc(vec, regs, e as u64);
            // M14: 终止当前任务 + 转场幸存者
            if crate::sched::terminate_current_and_next() {
                return 1;
            }
            // 单任务: 无幸存者 -> 停机诊断
            serial::write_line("  -- no survivors, kernel halted");
            loop {
                crate::hlt();
            }
        }
        // 内核态异常: 诊断 + 停机
        serial::write_str("EXCEPTION vec=");
        print_exc_dec(vec);
        serial::write_str(" (kernel) rip=");
        let rip = regs.add(10 + e).read();
        crate::syscall::log_hex(rip);
        serial::write_str(" cs=");
        crate::syscall::log_hex(cs as u64 & 0xFF);
        if e == 1 {
            // 有错误码: 打印 (iretq 恢复段失败时 err=选择子)
            serial::write_str(" err=");
            crate::syscall::log_hex(regs.add(10).read());
            // 帧解码: [10]=ERR [11]=RIP [12]=CS [13]=RFLAGS; iretq 目标帧在
            // 下方: [14..18]=tRIP tCS tRFLAGS tRSP tSS
            for k in 0..9usize {
                serial::write_str(" f");
                crate::syscall::log_hex(k as u64);
                serial::write_str("=");
                crate::syscall::log_hex(regs.add(11 + k).read());
            }
        }
        if vec == 14 {
            let mut cr2: u64;
            asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags));
            serial::write_str(" cr2=");
            crate::syscall::log_hex(cr2);
        }
        serial::write_line("  -- kernel halted");
        loop {
            crate::hlt();
        }
    }
}

fn print_exc_dec(v: u64) {
    let mut buf = [0u8; 4];
    let mut i = 4;
    let mut x = v;
    loop {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
        if x == 0 {
            break;
        }
    }
    serial::write_str(core::str::from_utf8(&buf[i..]).unwrap());
}

fn pic_remap() {
    unsafe {
        serial::outb(0x20, 0x11);
        serial::outb(0xA0, 0x11);
        serial::outb(0x21, 0x20); // master -> 0x20
        serial::outb(0xA1, 0x28); // slave  -> 0x28
        serial::outb(0x21, 0x04);
        serial::outb(0xA1, 0x02);
        serial::outb(0x21, 0x01);
        serial::outb(0xA1, 0x01);
        serial::outb(0x21, 0xFE); // 仅 IRQ0 (timer) — M5 二分: 键盘 IRQ1 暂时屏蔽
        serial::outb(0xA1, 0xFF);
    }
}

fn pit_setup() {
    unsafe {
        serial::outb(0x43, 0x36); // ch0, rate-gen, LSB/MSB
        serial::outb(0x40, PIT_DIVISOR as u8);
        serial::outb(0x40, (PIT_DIVISOR >> 8) as u8);
    }
}

pub fn init() {
    pic_remap();
    pit_setup();

    let stub_base = fujo_exc_stub_table as usize as u64;
    let pit_addr = fujo_pit_stub as usize as u64;
    let kbd_addr = fujo_kbd_stub as usize as u64;

    unsafe {
        for i in 0..14usize {
            // volatile: 防死存储消除 (优化器陷阱, 同 gdt.rs)
            let e = (core::ptr::addr_of_mut!(IDT) as *mut IdtEntry).add(i);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_lo), (stub_base + (i as u64) * 14) as u16);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).sel), 0x08u16);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).ist), 0u8);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).attr), 0x8Eu8);
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*e).off_mid),
                ((stub_base + (i as u64) * 14) >> 16) as u16,
            );
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_hi), ((stub_base + (i as u64) * 14) >> 32) as u32);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).zero), 0u32);
        }
        // ---- M75: 向量 1 (#DB) 调试器桩 ----
        let dbg_addr = fujo_dbg_stub as usize as u64;
        let e = (core::ptr::addr_of_mut!(IDT) as *mut IdtEntry).add(1);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_lo), dbg_addr as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).sel), 0x08u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).ist), 0u8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).attr), 0x8Eu8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_mid), (dbg_addr >> 16) as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_hi), (dbg_addr >> 32) as u32);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).zero), 0u32);

        // ---- M75: 向量 3 (#BP, int3 软件断点) ----
        // int3 是 INT 指令: 中断门 DPL 必须 >= CPL (3)!, 否则用户态
        // int3 → #GP (M75 实测: attr=0x8E → vec=13; 改 0xEE)。
        let bp_addr = fujo_bp_stub as usize as u64;
        let e = (core::ptr::addr_of_mut!(IDT) as *mut IdtEntry).add(3);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_lo), bp_addr as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).sel), 0x08u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).ist), 0u8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).attr), 0xEEu8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_mid), (bp_addr >> 16) as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_hi), (bp_addr >> 32) as u32);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).zero), 0u32);

        // ---- 向量 14 (#PF): M12 专用桩 (全寄存器保存, 可重试) ----
        let pf_addr = fujo_pf_stub as usize as u64;
        let e = (core::ptr::addr_of_mut!(IDT) as *mut IdtEntry).add(14);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_lo), pf_addr as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).sel), 0x08u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).ist), 0u8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).attr), 0x8Eu8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_mid), (pf_addr >> 16) as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_hi), (pf_addr >> 32) as u32);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).zero), 0u32);
        let e = (core::ptr::addr_of_mut!(IDT) as *mut IdtEntry).add(0x20);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_lo), pit_addr as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).sel), 0x08u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).ist), 0u8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).attr), 0x8Eu8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_mid), (pit_addr >> 16) as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_hi), (pit_addr >> 32) as u32);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).zero), 0u32);

        let e = (core::ptr::addr_of_mut!(IDT) as *mut IdtEntry).add(0x21);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_lo), kbd_addr as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).sel), 0x08u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).ist), 0u8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).attr), 0x8Eu8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_mid), (kbd_addr >> 16) as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_hi), (kbd_addr >> 32) as u32);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).zero), 0u32);

        // ---- IRQ3 (COM2 模型链路, M10) ----
        let ser2_addr = fujo_ser2_stub as usize as u64;
        let e = (core::ptr::addr_of_mut!(IDT) as *mut IdtEntry).add(0x23);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_lo), ser2_addr as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).sel), 0x08u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).ist), 0u8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).attr), 0x8Eu8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_mid), (ser2_addr >> 16) as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_hi), (ser2_addr >> 32) as u32);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).zero), 0u32);
        // ---- IRQ12 (PS/2 鼠标, M36) ----
        let ms_addr = fujo_ms_stub as usize as u64;
        let e = (core::ptr::addr_of_mut!(IDT) as *mut IdtEntry).add(0x2C);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_lo), ms_addr as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).sel), 0x08u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).ist), 0u8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).attr), 0x8Eu8);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_mid), (ms_addr >> 16) as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).off_hi), (ms_addr >> 32) as u32);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*e).zero), 0u32);

        // IRQ3 的 PIC 掩码在 serial::uart2_init() 末尾开放 (0x21=0xF5)。

        core::ptr::write_volatile(core::ptr::addr_of_mut!(IDT_PTR.limit), (IDT_SIZE * 16 - 1) as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(IDT_PTR.base), &raw mut IDT as u64);
        asm!("lidt [{}]", in(reg) core::ptr::addr_of_mut!(IDT_PTR), options(nostack));
    }
}

pub fn ticks() -> u64 {
    // M10.1 修复: 必须 volatile —— 此前普通读被 LLVM 提升出等待循环,
    // "while ticks-t0 < 250 {}" 编译成 jmp $ (死循环, 实测 RIP 停在 jmp 自身)。
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(pit_ticks)) }
}
