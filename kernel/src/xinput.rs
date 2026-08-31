//! xinput.rs — XInput 式输入抽象 v0 (M53): 键鼠统一聚合
//!
//! XInput 布局 状态: buttons bitmask + 4 摇杆轴 (i16 -32767..32767)。
//! 输入源: 键盘 (WASD=左摇杆, 空格=button0, Z/X=button1/2,
//! Enter=start, Backspace=back) + 鼠标 (相对位移=右摇杆, 按钮=bit8)。
//! fujo: 0x6001 xin_get(ptr->u32×5) / 0x6002 xin_reset() /
//!       0x6003 xin_press(bit) (程序自测注入口)。

use crate::serial;

pub static mut XIN_BUTTONS: u32 = 0;
pub static mut XIN_LX: i16 = 0;
pub static mut XIN_LY: i16 = 0;
pub static mut XIN_RX: i16 = 0;
pub static mut XIN_RY: i16 = 0;

/// kbd 钩子 (decode 后; 逻辑按键状态 v0: 按保持)。
pub fn kbd_hook(c: char) {
    unsafe {
        let mut dx = 0i16;
        let mut dy = 0i16;
        match c {
            'w' => dy = -32767,
            's' => dy = 32767,
            'a' => dx = -32767,
            'd' => dx = 32767,
            ' ' => XIN_BUTTONS |= 1,
            'x' => XIN_BUTTONS |= 2,
            'z' => XIN_BUTTONS |= 4,
            '\n' => XIN_BUTTONS |= 0x10,
            '\x08' => XIN_BUTTONS |= 0x20,
            _ => {}
        }
        if dx != 0 {
            XIN_LX = dx;
        }
        if dy != 0 {
            XIN_LY = dy;
        }
    }
}

/// mouse 钩子 (相对位移/按钮)。
pub fn mouse_hook(dx: i32, dy: i32, btn: u32) {
    unsafe {
        let rx = (XIN_RX as i32 + dx).clamp(-32767, 32767) as i16;
        let ry = (XIN_RY as i32 - dy).clamp(-32767, 32767) as i16;
        XIN_RX = rx;
        XIN_RY = ry;
        if btn != 0 {
            XIN_BUTTONS |= 0x100;
        } else {
            XIN_BUTTONS &= !0x100;
        }
    }
}

/// 0x6001: 读取 (ptr -> u32×5: buttons, lx, ly, rx, ry — 轴按 i16 符号扩展)。
pub fn fujo_xin_get(ptr: u64) -> i64 {
    unsafe {
        let p = ptr as *mut u32;
        p.add(0).write(XIN_BUTTONS);
        p.add(1).write(XIN_LX as u32);
        p.add(2).write(XIN_LY as u32);
        p.add(3).write(XIN_RX as u32);
        p.add(4).write(XIN_RY as u32);
        serial::write_str("xin  : state");
    }
    0
}

/// 0x6002: 复位。
pub fn fujo_xin_reset() -> i64 {
    unsafe {
        XIN_BUTTONS = 0;
        XIN_LX = 0;
        XIN_LY = 0;
        XIN_RX = 0;
        XIN_RY = 0;
    }
    0
}

/// 0x6003: 自测注入口 (模拟按下某个 bit)。
pub fn fujo_xin_press(bit: u64) -> i64 {
    unsafe {
        XIN_BUTTONS |= (bit as u32) & 0xFFFFFFFF;
    }
    0
}
