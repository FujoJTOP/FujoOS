//! mouse.rs — PS/2 鼠标驱动 + IRQ12 + 命中测试/焦点 (M36)
//!
//! 流程: 8042 AUX 使能(0xA8 + 命令字节 bit1) -> 鼠标默认/流水模式 ->
//! IRQ12 (向量 0x2C, 从片 IRQ4) -> 3 字节数据包状态机 -> 坐标/按键累积 ->
//! 命中测试(矩形表) -> 焦点 id。用户态经 fujo 原生原语查询:
//!   0x5410 mouse_info(ptr)   写 u32×4 (x, y, buttons, steps)
//!   0x5411 mouse_rects(ptr,n) 注册命中矩形 [x0,y0,x1,y1,id]×n
//!   0x5412 mouse_focus()      -> 当前焦点 id (0xFFFFFFFF = 无)

use crate::serial;

pub static mut MS_X: u32 = 0;
pub static mut MS_Y: u32 = 0;
pub static mut MS_BTN: u32 = 0;
pub static mut MS_STEPS: u64 = 0;

/// 命中矩形表 (x0, y0, x1, y1, id) — 最多 8 个, 注册顺序即 z-order。
pub static mut HIT_RECTS: [(u32, u32, u32, u32, u32); 8] = [(0, 0, 0, 0, 0xFFFF_FFFF); 8];
pub static mut FOCUS_ID: u32 = 0xFFFF_FFFF;

static mut MS_STATE: u32 = 0;
static mut MS_PKT: [u8; 3] = [0; 3];

fn ms_inb(p: u16) -> u8 {
    let v: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") v, in("dx") p, options(nomem, nostack)); }
    v
}

fn ms_outb(p: u16, v: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") p, in("al") v, options(nomem, nostack)); }
}

/// 8042 鼠标初始化 (QEMU ps2 鼠标)。**风险点 (M36 实证)**: 读 8042 命令字节
/// 期间若 IRQ1 键盘中断读走 0x60 数据, 会把键盘扫描码当命令字节写回,
/// 杀死键盘 —— 因此整个序列在禁 IRQ1 下完成。QEMU 默认 ps2 鼠标即
/// stream 模式, 无需 F6/F4 配置 (省 ACK 竞态)。IRQ12 掩码: master IRQ2
/// (级联) + slave IRQ4 (从片), 在 com2 之后调用 (避免 uart2 覆盖掩码)。
pub fn init() {
    unsafe {
        // 禁键盘 IRQ1 (0x21 bit0): 防 0x60 争抢
        let m0 = ms_inb(0x21);
        ms_outb(0x21, m0 | 0x02);
        while ms_inb(0x64) & 2 != 0 {}
        ms_outb(0x64, 0xA8); // enable AUX (鼠标)
        while ms_inb(0x64) & 2 != 0 {}
        ms_outb(0x64, 0x20); // 读命令字节
        while ms_inb(0x64) & 1 == 0 {}
        let cmd = ms_inb(0x60);
        while ms_inb(0x64) & 2 != 0 {}
        ms_outb(0x64, 0x60);
        ms_outb(0x60, cmd | 0x02); // bit1 = AUX IRQ 使能 (保留键盘 bit0 原态)
        // F4: enable reporting (ps2 鼠标默认禁包, 必须显式开启; M36 实证)
        while ms_inb(0x64) & 2 != 0 {}
        ms_outb(0x64, 0xD4);
        while ms_inb(0x64) & 2 != 0 {}
        ms_outb(0x60, 0xF4);
        // 等 ACK (0xFA) — 禁 IRQ1 下读 0x60 不会再与键盘争抢
        while ms_inb(0x64) & 1 == 0 {}
        let _ = ms_inb(0x60);
        // 清清 0x60 (可能的残余)
        if ms_inb(0x64) & 1 != 0 {
            let _ = ms_inb(0x60);
        }
        // PH 清空 0x60: ACK/残余字节在 IRQ12 开启前全部排空 (M37 实证:
        // 0xFA 污染状态机 -> 坐标被假包打满 65535 -> 命中全失败)
        while ms_inb(0x64) & 1 != 0 {
            let _ = ms_inb(0x60);
        }
        MS_STATE = 0;
        // PIC: master IRQ1 恢复 + IRQ2 (cascade) 开; slave IRQ4 (IRQ12) 开
        let m = ms_inb(0x21);
        ms_outb(0x21, (m & !0x02) & !0x04);
        let s = ms_inb(0xA1);
        ms_outb(0xA1, s & !0x10);
        serial::write_line("mouse: ps/2 aux enabled, irq12 armed (M36)");
    }
}

/// IRQ12 入口: 数据包状态机 + EOI (master+slave)。
#[no_mangle]
pub extern "C" fn fujo_ms_irq() {
    unsafe {
        let b = ms_inb(0x60);
        match MS_STATE {
            0 => {
                if b & 0x08 != 0 {
                    MS_PKT[0] = b;
                    MS_STATE = 1;
                }
            }
            1 => {
                MS_PKT[1] = b;
                MS_STATE = 2;
            }
            _ => {
                MS_PKT[2] = b;
                MS_STATE = 0;
                let dx = MS_PKT[1] as i8 as i32;
                let dy = MS_PKT[2] as i8 as i32;
                let nx = MS_X as i32 + dx;
                let ny = MS_Y as i32 - dy; // 屏幕坐标 y 向下为正
                MS_X = nx.clamp(0, 0xFFFF) as u32;
                MS_Y = ny.clamp(0, 0xFFFF) as u32;
                MS_BTN = (MS_PKT[0] & 7) as u32;
                MS_STEPS += 1;
                // 命中测试: 首个包含坐标的矩形 (注册顺序=顶层优先)
                let mut id = 0xFFFF_FFFFu32;
                for i in 0..8usize {
                    let (x0, y0, x1, y1, rid) = HIT_RECTS[i];
                    if rid != 0xFFFF_FFFF
                        && MS_X >= x0
                        && MS_X <= x1
                        && MS_Y >= y0
                        && MS_Y <= y1
                    {
                        id = rid;
                        break;
                    }
                }
                if id != FOCUS_ID {
                    crate::wmsg::notify_focus_changed(FOCUS_ID, id);
                    FOCUS_ID = id;
                }
                // M37: 移动/按钮消息投递 (焦点窗口)
                crate::wmsg::notify_moved(MS_X, MS_Y, MS_BTN, FOCUS_ID);
                if MS_PKT[0] & 7 != 0 {
                    crate::wmsg::notify_button(MS_X, MS_Y, MS_BTN, FOCUS_ID);
                }
            }
        }
        ms_outb(0x20, 0x20); // master EOI
        ms_outb(0xA0, 0x20); // slave EOI (从片级联)
    }
}

/// 0x5410: 写 u32×4 (x, y, buttons, steps低32) 到用户 ptr。
pub fn fujo_mouse_info(ptr: u64) -> i64 {
    unsafe {
        let p = ptr as *mut u32;
        p.add(0).write(MS_X);
        p.add(1).write(MS_Y);
        p.add(2).write(MS_BTN);
        p.add(3).write(MS_STEPS as u32);
    }
    0
}

/// 0x5411: 注册命中矩形 [x0,y0,x1,y1,id]×n (用户指针, n<=8)。
pub fn fujo_mouse_rects(ptr: u64, n: u64) -> i64 {
    unsafe {
        let n = n.min(8) as usize;
        for i in 0..n {
            let e = (ptr as *const u32).add(i * 5);
            HIT_RECTS[i] = (e.add(0).read(), e.add(1).read(), e.add(2).read(), e.add(3).read(), e.add(4).read());
        }
        for i in n..8 {
            HIT_RECTS[i] = (0, 0, 0, 0, 0xFFFF_FFFF);
        }
    }
    0
}

/// 0x5412: 当前焦点。
pub fn fujo_mouse_focus() -> i64 {
    unsafe { FOCUS_ID as i64 }
}
