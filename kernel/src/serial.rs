//! fujo-kernel 串口输出 (COM1, 115200 8N1) —— 开发日志通道 + QEMU -serial stdio 捕获。

use core::arch::asm;

pub unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}

pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

pub unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    asm!("in ax, dx", out("ax") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

pub unsafe fn outw(port: u16, val: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") val, options(nomem, nostack, preserves_flags));
}

pub fn init() {
    unsafe {
        outb(0x3F9, 0x00); // 禁用中断
        outb(0x3FB, 0x80); // DLAB = 1
        outb(0x3F8, 0x03); // 波特率低字节 (115200)
        outb(0x3F9, 0x00); // 波特率高字节
        outb(0x3FB, 0x03); // 8N1
        outb(0x3FA, 0xC7); // FIFO 启用/清空
        outb(0x3F8, 0x0B); // 提交
    }
}

pub fn write_byte(b: u8) {
    // 等待发送保持寄存器空 (LSR bit5)
    while unsafe { inb(0x3FD) } & 0x20 == 0 {
        core::hint::spin_loop();
    }
    unsafe { outb(0x3F8, b) }
}

pub fn write_str(s: &str) {
    for &b in s.as_bytes() {
        write_byte(b);
    }
}

pub fn write_line(s: &str) {
    write_str(s);
    write_byte(b'\n');
}

// ---------------------------------------------------------------------------
// COM2 模型链路 (M10 · fujonn engine=qwen)
// 经 QEMU 第二个 -serial (tcp:127.0.0.1:4000) 连宿主机 qwen_model_server.py;
// Hermes CLI (ring3) 调 0x5101 -> 内核发 FJAI:REQ 帧等 FJAI:RSP 帧。
// ---------------------------------------------------------------------------
const SER2: u16 = 0x2F8;
const SER2_BUF_SIZE: usize = 512;

static mut SER2_BUF: [u8; SER2_BUF_SIZE] = [0; SER2_BUF_SIZE];
static mut SER2_HEAD: usize = 0;
static mut SER2_TAIL: usize = 0;

/// COM2 初始化: 115200 8N1, FIFO 使能, 先排空再开 RX 中断, 最后开放 IRQ3 (0x21=0xF5)。
pub fn uart2_init() {
    unsafe {
        outb(SER2 + 1, 0x00); // IER = 0 (禁中断, 配置期)
        outb(SER2 + 3, 0x80); // DLAB = 1
        outb(SER2 + 0, 0x01); // 115200
        outb(SER2 + 1, 0x00);
        outb(SER2 + 3, 0x03); // 8N1
        outb(SER2 + 2, 0xC7); // FIFO 启用/清空
        outb(SER2 + 4, 0x0B); // DTR|RTS
        // 排空待处理字符 (防数据提前触发中断)
        while inb(SER2 + 5) & 1 != 0 {
            let _ = inb(SER2 + 0);
        }
        outb(SER2 + 1, 0x01); // IER = 仅 RX 中断
        // 0xF8: IRQ0(PIT)+IRQ1(键盘) 开放, IRQ3 关闭 —— M10 v0 采用纯轮询 RX
        // (IRQ3 与轮询并读实测丢字节; 后续换 IIR/FIFO 触发再开中断)
        outb(0x21, 0xF8);
    }
}

/// IRQ3 处理 (asm 桩 fujo_ser2_stub 调用): 排空入环 + EOI (volatile 环读写)。
#[no_mangle]
pub extern "C" fn fujo_ser2_irq() {
    unsafe {
        let mut got = 0u32;
        while inb(SER2 + 5) & 1 != 0 {
            let b = inb(SER2 + 0);
            let t = core::ptr::read_volatile(core::ptr::addr_of!(SER2_TAIL));
            let h = core::ptr::read_volatile(core::ptr::addr_of!(SER2_HEAD));
            if (t + 1) % SER2_BUF_SIZE != h {
                core::ptr::write_volatile(core::ptr::addr_of_mut!(SER2_BUF[t]), b);
                core::ptr::write_volatile(core::ptr::addr_of_mut!(SER2_TAIL), (t + 1) % SER2_BUF_SIZE);
            }
            got += 1;
            if got > 128 {
                break;
            }
        }
        outb(0x20, 0x20); // EOI master
    }
}

/// 轮询读一个字节 (先取 IRQ3 环, 再直接读 UART LSR/RBR —— IRQ 不可靠时保底)。
pub fn ser2_poll() -> Option<u8> {
    unsafe {
        let h = core::ptr::read_volatile(core::ptr::addr_of!(SER2_HEAD));
        let t = core::ptr::read_volatile(core::ptr::addr_of!(SER2_TAIL));
        if h != t {
            let b = core::ptr::read_volatile(core::ptr::addr_of!(SER2_BUF[h]));
            core::ptr::write_volatile(core::ptr::addr_of_mut!(SER2_HEAD), (h + 1) % SER2_BUF_SIZE);
            return Some(b);
        }
        if inb(SER2 + 5) & 1 != 0 {
            return Some(inb(SER2 + 0));
        }
    }
    None
}

/// 发送一帧到 COM2 (轮询 THR 空)。
pub fn ser2_tx_line(bytes: &[u8]) {
    for &b in bytes {
        while unsafe { inb(SER2 + 5) } & 0x20 == 0 {
            core::hint::spin_loop();
        }
        unsafe { outb(SER2 + 0, b) }
    }
}
