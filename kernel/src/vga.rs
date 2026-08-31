//! VGA 文本模式 (0xB8000, 80x25) 输出 —— 开发显示通道。

pub const WIDTH: usize = 80;
pub const HEIGHT: usize = 25;
const VGA_BUF: *mut u16 = 0xB8000 as *mut u16;

static mut ROW: usize = 0;
static mut COL: usize = 0;
static mut COLOR: u8 = 0x07;

pub fn init() {
    clear();
}

/// 清屏并回到 (0,0)。
pub fn clear() {
    for i in 0..WIDTH * HEIGHT {
        unsafe { VGA_BUF.add(i).write(0x0720) }
    }
    unsafe {
        ROW = 0;
        COL = 0;
        COLOR = 0x07;
    }
}

pub fn set_color(color: u8) {
    unsafe { COLOR = color }
}

pub fn put_char(ch: u8) {
    match ch {
        b'\n' => unsafe {
            COL = 0;
            ROW += 1;
        },
        b'\r' => unsafe {
            COL = 0;
        },
        _ => unsafe {
            if COL >= WIDTH {
                COL = 0;
                ROW += 1;
            }
            if ROW >= HEIGHT {
                scroll();
                ROW = HEIGHT - 1;
            }
            let cell = ((COLOR as u16) << 8) | ch as u16;
            VGA_BUF.add(ROW * WIDTH + COL).write(cell);
            COL += 1;
        },
    }
}

fn scroll() {
    for row in 1..HEIGHT {
        for col in 0..WIDTH {
            unsafe {
                let v = VGA_BUF.add(row * WIDTH + col).read();
                VGA_BUF.add((row - 1) * WIDTH + col).write(v);
            }
        }
    }
    for col in 0..WIDTH {
        unsafe {
            VGA_BUF.add((HEIGHT - 1) * WIDTH + col).write(0x0720);
        }
    }
}

pub fn write_str(s: &str) {
    for &b in s.as_bytes() {
        put_char(b);
    }
}

pub fn write_line(s: &str) {
    write_str(s);
    put_char(b'\n');
}
