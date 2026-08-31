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

// ---------------------------------------------------------------------------
// M10.1 启动 Logo: 几何徽章 (CP437 块字符绘制, 非纯文字) —— 直接写文本显存。
// 六边形盾形(蓝底白块) + 黄色 F 单字母(块拼) + 标题 + 提示行。
// 纯 ASCII 源(块字符用 \x 转义), 保证任何工具链/控制台无编码问题。
// ---------------------------------------------------------------------------

#[inline]
fn cell(row: usize, col: usize, ch: u8, attr: u8) {
    unsafe {
        VGA_BUF.add(row * WIDTH + col).write(((attr as u16) << 8) | ch as u16);
    }
}

/// 徽章行形状: (start_col, width) —— 六边形 15 行 (CP437 块 = 真实绘制的像素徽章)。
const HEX_ROWS: &[(usize, usize)] = &[
    (16, 8),
    (13, 14),
    (10, 20),
    (8, 24),
    (7, 26),
    (6, 28),
    (6, 28),
    (6, 28),
    (6, 28),
    (6, 28),
    (6, 28),
    (7, 26),
    (8, 24),
    (10, 20),
    (13, 14),
    (16, 8),
];

pub fn logo() {
    clear();
    let row0 = 3usize;
    // 六边形徽章: 蓝底白块 (attr 0x1F = 白字/蓝底, 块字符着色)
    for (r, &(col, w)) in HEX_ROWS.iter().enumerate() {
        let row = row0 + r;
        for k in col..col + w {
            cell(row, k, b'\xDB', 0x1F);
        }
    }
    // 黄色 F 单字母 (█ 块拼装, 徽章内部; attr 0x1E = 黄字/蓝底)
    let fx = 40usize; // 居中: col 40
    let fy = row0 + 4; // 徽章内顶部
    // 竖干 2 宽 x 9 高
    for r in 0..9 {
        for c in 0..2 {
            cell(fy + r, fx + c + 0, b'\xDB', 0x1E);
            cell(fy + r, fx + c + 1, b'\xDB', 0x1E);
        }
    }
    // 顶横 18 宽 x 2 高
    for r in 0..2 {
        for c in 0..18 {
            cell(fy + r, fx + c, b'\xDB', 0x1E);
        }
    }
    // 中横 14 宽 x 2 高
    for r in 0..2 {
        for c in 0..14 {
            cell(fy + 3 + r, fx + c, b'\xDB', 0x1E);
        }
    }
    // 徽章右下角: 白色高亮点
    cell(row0 + 9, 66, b'\xDB', 0x0F);
    cell(row0 + 10, 55, b'\xDB', 0x0F);
    // 标题 (自绘块之后, 作为补充说明文字)
    let title = "F U J O S";
    let mut c = 36usize;
    for ch in title.bytes() {
        cell(row0 + 17, c, ch, 0x1F);
        c += 1;
    }
    let ver = "0.1.0-dev  CPU x86_64  AI-native";
    let mut c = 30usize;
    for ch in ver.bytes() {
        cell(row0 + 19, c, ch, 0x07);
        c += 1;
    }
    // 提示行 (os shell 命令)
    let hint = "type: os run hermes   (launch Hermes agent CLI)";
    let mut c = 22usize;
    for ch in hint.bytes() {
        cell(row0 + 22, c, ch, 0x0F);
        c += 1;
    }
    // 副提示
    let hint2 = "[10s idle auto-run]  [help] [os run hermes]";
    let mut c = 26usize;
    for ch in hint2.bytes() {
        cell(row0 + 23, c, ch, 0x08);
        c += 1;
    }
}
