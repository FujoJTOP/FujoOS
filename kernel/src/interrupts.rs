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

const IDT_SIZE: usize = 0x25; // 0..0x24 (异常 + IRQ0/1 键盘 + IRQ3 COM2 模型链路)

static mut IDT: [IdtEntry; IDT_SIZE] = [IdtEntry::empty(); IDT_SIZE];

extern "C" {
    fn fujo_exc_stub_table();
    fn fujo_pit_stub();
    fn fujo_kbd_stub();
    fn fujo_ser2_stub();
}

core::arch::global_asm!(r#"
    .text
    # ---- 异常桩: 每个桩固定 14 字节: mov rdi,N(7B) + call rel32(5B) + ud2(2B) ----
    # (M4 踩坑: jmp 会优化成 rel8 短跳; 不用宏展开 —— 宏+完整重建在工具链上
    #  偶发装配瞬态 (M9 DEV 记录), 手写定长桩为确定性方案)
    .p2align 4
    .global fujo_exc_stub_table
fujo_exc_stub_table:
    mov rdi, 0
    call fujo_exc
    ud2
    mov rdi, 1
    call fujo_exc
    ud2
    mov rdi, 2
    call fujo_exc
    ud2
    mov rdi, 3
    call fujo_exc
    ud2
    mov rdi, 4
    call fujo_exc
    ud2
    mov rdi, 5
    call fujo_exc
    ud2
    mov rdi, 6
    call fujo_exc
    ud2
    mov rdi, 7
    call fujo_exc
    ud2
    mov rdi, 8
    call fujo_exc
    ud2
    mov rdi, 9
    call fujo_exc
    ud2
    mov rdi, 10
    call fujo_exc
    ud2
    mov rdi, 11
    call fujo_exc
    ud2
    mov rdi, 12
    call fujo_exc
    ud2
    mov rdi, 13
    call fujo_exc
    ud2
    mov rdi, 14
    call fujo_exc
    ud2

    # ---- PIT (IRQ0) —— 计数 + EOI + 返回 ----
    .p2align 4
    .global fujo_pit_stub
fujo_pit_stub:
    inc qword ptr [rip + pit_ticks]
    push rax
    mov al, 0x20
    out 0x20, al
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
    # 与键盘桩同一原则: 保存全部 caller-saved (C 会破坏 rsi/r8-r11)。
    .p2align 4
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
"#);

/// 异常处理（C 侧, 只打印并停机）— 带现场诊断 (M10: CS/RIP/CR2 定位)
#[no_mangle]
pub extern "C" fn fujo_exc(vec: u64) -> ! {
    serial::write_str("EXCEPTION vec=");
    // 十进制打印
    let mut buf = [0u8; 4];
    let mut v = vec;
    let mut i = 4;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    serial::write_str(core::str::from_utf8(&buf[i..]).unwrap());
    // 现场: 从当前 rsp 起, 调用返回地址之后为异常帧; 直接 dump 8 个 qword
    unsafe {
        let sp: u64;
        asm!("mov {}, rsp", out(reg) sp, options(nomem, nostack, preserves_flags));
        serial::write_str(" sp=");
        crate::syscall::log_hex(sp);
        for k in 1..=8u64 {
            let v = core::ptr::read((sp as *const u64).add(k as usize));
            crate::syscall::log_hex(v);
        }
        if vec == 14 {
            let mut cr2: u64;
            asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags));
            serial::write_str(" cr2=");
            crate::syscall::log_hex(cr2);
        }
    }
    serial::write_line("  -- kernel halted");
    loop {
        crate::hlt();
    }
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
        for i in 0..15usize {
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
        // IRQ3 的 PIC 掩码在 serial::uart2_init() 末尾开放 (0x21=0xF5)。

        core::ptr::write_volatile(core::ptr::addr_of_mut!(IDT_PTR.limit), (IDT_SIZE * 16 - 1) as u16);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(IDT_PTR.base), &raw mut IDT as u64);
        asm!("lidt [{}]", in(reg) core::ptr::addr_of_mut!(IDT_PTR), options(nostack));
    }
}

pub fn ticks() -> u64 {
    unsafe { core::ptr::addr_of!(pit_ticks).read() }
}
