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
