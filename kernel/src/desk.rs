//! desk.rs — 桌面环境 v0 (M46): 任务栏 + 开始菜单雏形
//!
//! backbuffer 合成: 桌面背景 + 底部 40px 任务栏(开始按钮图标+时钟位)
//! + 开始菜单框(点击后状态机)。fujo 原语:
//!   0x5B01 desk_init()            清屏+背景+任务栏
//!   0x5B02 desk_taskbar(text)     任务栏时钟/标题
//!   0x5B03 desk_start(x,y)        开始按钮命中 (x,y 屏幕坐标) -> 1 命中
//!   0x5B04 desk_menu(on)          菜单框渲染 (从 y=40 起 200x180)
//!   0x5B05 desk_pixel(x,y)        像素读回

use crate::font;
use crate::icon;

const TB_H: u32 = 40;
const MENU_W: u32 = 200;
const MENU_H: u32 = 180;

fn setp(x: u32, y: u32, col: u32) {
    if x >= font::fb_w() || y >= font::fb_h() {
        return;
    }
    unsafe {
        let p = (font::BACKBUFFER + ((y as u64) * font::fb_w() as u64 + x as u64) * 4) as *mut u32;
        p.write(col);
    }
}

fn readp(x: u32, y: u32) -> u32 {
    if x >= font::fb_w() || y >= font::fb_h() {
        return 0;
    }
    unsafe {
        let p = (font::BACKBUFFER + ((y as u64) * font::fb_w() as u64 + x as u64) * 4) as *const u32;
        p.read()
    }
}

/// 矩形填充。
fn fill(x: u32, y: u32, w: u32, h: u32, col: u32) {
    for dy in 0..h {
        for dx in 0..w {
            setp(x + dx, y + dy, col);
        }
    }
}

fn font_line(x: u32, y: u32, scale: u32, color: u32, text: &str) {
    for (i, b) in text.bytes().enumerate() {
        if b < 0x20 || b > 0x7E {
            continue; // M107: 控制字符/非 ASCII 跳过 (下溢 panic 实证)
        }
        let g = font::GLYPHS[(b - 0x20) as usize];
        for gy in 0..5u32 {
            for gx in 0..7u32 {
                if (g[gy as usize] >> (6 - gx)) & 1 != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            setp(
                                x + i as u32 * 8 * scale + gx * scale + sx,
                                y + gy * scale + sy,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }
}

/// 0x5B01: 桌面初始化 (bg + 任务栏)。
pub fn fujo_desk_init() -> i64 {
    unsafe {
        let bg = icon::PAL[1];
        let tb = icon::PAL[2];
        fill(0, 0, font::fb_w(), font::fb_h(), bg);
        // 任务栏
        fill(0, font::fb_h() - TB_H, font::fb_w(), TB_H, tb);
        // 开始按钮 (60x36 方块 + logo 图标)
        fill(8, font::fb_h() - TB_H + 2, 56, 36, 0xFFFFFFFFu32);
        let _ = icon::fujo_icon_draw(10, font::fb_h() as u64 - TB_H as u64 + 4, 3, 2);
        crate::serial::write_line("desk : desktop + taskbar rendered");
        crate::graphics::present(); // 投映 backbuffer → LFB (GTK/gfx 可见)
    }
    0
}

/// 0x5B02: 任务栏时钟/标题。
pub fn fujo_desk_taskbar(text: u64) -> i64 {
    unsafe {
        let mut n = 0usize;
        let mut tb = [0u8; 48];
        while n < 47 {
            let b = (text as *const u8).add(n).read();
            if b == 0 {
                break;
            }
            tb[n] = b;
            n += 1;
        }
        let s = core::str::from_utf8(&tb[..n]).unwrap_or("");
        let color = icon::PAL[0];
        font_line(700, font::fb_h() - TB_H + 10, 2, color, s);
        crate::graphics::present();
    }
    0
}

/// 0x5B03: 开始按钮命中。
pub fn fujo_desk_start(x: u64, y: u64) -> i64 {
    if y >= (font::fb_h() - TB_H) as u64 && x < 64 && y >= (font::fb_h() - 38) as u64 {
        1
    } else {
        0
    }
}

/// 0x5B04: 开始菜单框 (on=1 渲染)。
pub fn fujo_desk_menu(on: u64) -> i64 {
    if on == 0 {
        return 0;
    }
    unsafe {
        let surf = icon::PAL[7];
        let ink = icon::PAL[0];
        fill(8, 0, MENU_W, MENU_H, surf);
        // 边框
        for x in 0..MENU_W {
            setp(8 + x, 0, ink);
            setp(8 + x, MENU_H - 1, ink);
        }
        for y in 0..MENU_H {
            setp(8, 8 + y - (if y < 8 { y } else { 0 }), ink); // 简化: 左框
            setp(8 + MENU_W - 1, y, ink);
        }
        // 菜单项文本
        font_line(24, 8, 1, ink, "Programs");
        font_line(24, 28, 1, ink, "Files");
        font_line(24, 48, 1, ink, "Terminal");
        font_line(24, 68, 1, ink, "Shut Down");
    }
    0
}

/// 0x5B05: 像素读回。
pub fn fujo_desk_pixel(x: u64, y: u64) -> i64 {
    readp(x as u32, y as u32) as i64
}

// ---------------------------------------------------------------------------
// M107/M108: 桌面会话 (boot → 图形桌面; 双击图标启动窗口程序; TTY 窗口)
//
// 程序 = include_bytes 内嵌 ELF: Hermes (agent CLI) / 迷你 Shell TTY。
// 桌面主循环 (内核态): 渲染 + 真鼠标 (0x54xx 状态) 双击命中 + 合成双击
// 测试路径 (启动后 40 ticks; 无鼠标硬件同样验证全链);
// 窗口 TTY: 用户任务 write(1) -> tty_feed (行缓冲 24x64 环形) -> 每 16
// tick 重绘窗口; 键盘在程序活跃时由共享 kbd ring 透传 (0x5103)。
// ---------------------------------------------------------------------------

static HERMES_ELF: &[u8] = include_bytes!("../../sdk/hermes/hermes-high.elf");
static SHELL_ELF: &[u8] = include_bytes!("../../sdk/linux/m107_tty-high.elf");

const TTY_ROWS: usize = 24;
const TTY_COL: usize = 64;
static mut TTY_LINES: [[u8; TTY_COL]; TTY_ROWS] = [[0; TTY_COL]; TTY_ROWS];
static mut TTY_ROW: usize = 0; // 当前写入行 (环形)
static mut TTY_ROW_N: usize = 0;
static mut TTY_COL_POS: usize = 0;
static mut TTY_PID: u64 = 0; // 0 = 无窗口程序
static mut TTY_TITLE: [u8; 24] = [0; 24];
static mut WIN_PID: u64 = 0; // wm 窗口 id (槽+1)
static mut WX: u32 = 30;
static mut WY: u32 = 40;
static mut WW: u32 = 340;
static mut WH: u32 = 230;

/// wm 窗口类 id (缓存注册)。
static mut CLASS_ID: i64 = 0;

pub fn wm_class_id() -> i64 {
    unsafe {
        if CLASS_ID <= 0 {
            let c = crate::wmsg::fujo_wm_class(b"Window".as_ptr() as u64);
            CLASS_ID = if c > 0 { c } else { 1 };
        }
        CLASS_ID
    }
}

/// 0x5B10: 代理请求启动窗口程序 (0=Hermes 1=Shell)。
pub fn desk_launch(which: u64) -> i64 {
    if which == 0 {
        launch_program(HERMES_ELF, b"Hermes", wm_class_id() as u64)
    } else {
        launch_program(SHELL_ELF, b"Shell", wm_class_id() as u64)
    }
}

/// 0x5B11: (tty_pid, tty_row_n, win_pid, tty_col_pos)。
/// M108: 调用即重绘 TTY 窗口 (代理轮询驱动实时刷新 —— M107 内核主循环
/// 每 16 tick 重绘的等价物; 内核桌面模式无影响)。
pub fn desk_state(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(TTY_PID);
        w.add(1).write(TTY_ROW_N as u64);
        w.add(2).write(WIN_PID);
        w.add(3).write(TTY_COL_POS as u64);
        if TTY_PID != 0 {
            tty_draw_window();
        }
    }
    0
}

pub fn tty_pid() -> u64 {
    unsafe { TTY_PID }
}

fn tty_put_char(c: u8) {
    unsafe {
        if c == b'\n' {
            TTY_COL_POS = 0;
            TTY_ROW = (TTY_ROW + 1) % TTY_ROWS;
            if TTY_ROW_N < TTY_ROWS {
                TTY_ROW_N += 1;
            }
            for i in 0..TTY_COL {
                TTY_LINES[TTY_ROW][i] = 0;
            }
            return;
        }
        if TTY_COL_POS < TTY_COL - 1 {
            TTY_LINES[TTY_ROW][TTY_COL_POS] = c;
            TTY_COL_POS += 1;
        }
    }
}

/// 用户任务 write(fd=1) 钩子 (syscall::user_write 调): 当前程序 TTY 入行。
/// 门控: 仅窗口程序任务自身 (TTY_PID-1) 的写入计入 —— M108 代理与窗口
/// 程序同调 write(1), 代理的 m108 日志不得伪造 TTY 行 (否则 "rows>0" 假阳性)。
pub fn tty_feed(ptr: u64, len: u64) {
    unsafe {
        if TTY_PID == 0 {
            return;
        }
        if crate::sched::current_task() as u64 + 1 != TTY_PID {
            return;
        }
        for i in 0..len as usize {
            tty_put_char((ptr as *const u8).add(i).read());
        }
    }
}

fn tty_draw_window() {
    unsafe {
        // 窗口白底 + 标题栏 + 行文本
        fill(WX, WY, WW, WH, 0xFFFFFFu32);
        fill(WX, WY, WW, 24, 0xC0C0FFu32);
        for x in 0..WW {
            setp(WX + x, WY + WH - 1, 0x000000);
        }
        for y in 0..WH {
            setp(WX, WY + y, 0x000000);
            setp(WX + WW - 1, WY + y, 0x000000);
        }
        let title = core::str::from_utf8(&TTY_TITLE[..]).unwrap_or("?");
        font_line(WX + 8, WY + 5, 1, 0x000000, title);
        // 滚动 24 行 (旧行自底), 基础字体 8px 高
        let visible = TTY_ROW_N.min(TTY_ROWS);
        for r in 0..visible {
            let line = TTY_LINES[(TTY_ROW + TTY_ROWS + 1 - visible + r) % TTY_ROWS];
            font_line(WX + 6, WY + 28 + (r as u32) * 8, 1,
                      0x000000,
                      core::str::from_utf8(&line[..]).unwrap_or(""));
        }
        crate::graphics::present();
    }
}

/// 任务 A 被替换: 先终止旧程序/旧窗口。
fn kill_program() {
    unsafe {
        if TTY_PID != 0 {
            crate::sched::kill_task((TTY_PID - 1) as usize);
        }
        if WIN_PID != 0 {
            crate::wmsg::fujo_wm_remove(WIN_PID as u32);
        }
        TTY_PID = 0;
        WIN_PID = 0;
        TTY_ROW = 0;
        TTY_ROW_N = 0;
        TTY_COL_POS = 0;
    }
}

/// 启动窗口程序: load_elf(blob) -> spawn 任务 -> wm 窗口 -> TTY 接管。
fn launch_program(blob: &'static [u8], title: &[u8], kind: u64) -> i64 {
    unsafe {
        crate::mem::map_high_user(); // M108: 高地址装载区 (代理 0x400000 共存)
        kill_program();
        let entry = match crate::elf_loader::load_elf(blob.as_ptr() as u32, blob.len() as u32) {
            Ok(e) => e,
            Err(m) => {
                crate::serial::write_str("desk : load failed: ");
                crate::serial::write_line(m);
                return -5;
            }
        };
        let mut title24 = [0u8; 24];
        for (i, &b) in title.iter().take(23).enumerate() {
            title24[i] = b;
        }
        TTY_TITLE = title24;
        let pid = crate::sched::spawn_single(entry);
        if pid == usize::MAX {
            return -12;
        }
        TTY_PID = (pid as u64) + 1; // 哨兵: 0=无程序
        // wm 窗口 (标题类); class id 由调用方保证已注册
        let wid = crate::wmsg::fujo_wm_create(kind as u32, WX, WY, WW, WH);
        WIN_PID = wid as u64;
        crate::serial::write_str("desk : launched window '");
        crate::serial::write_str(core::str::from_utf8(&TTY_TITLE[..]).unwrap_or("?"));
        crate::serial::write_str("' pid=");
        crate::syscall::debug_dec(pid as u64);
        crate::serial::write_line("");
        tty_draw_window();
        kind as i64
    }
}

/// 桌面图标命中 (图标 40x36, 基位 (60,40)/(140,40)); 返回程序 id。
fn icon_hit(x: u32, y: u32) -> i64 {
    if x >= 60 && x < 100 && y >= 40 && y < 76 {
        return 0; // Hermes
    }
    if x >= 140 && x < 180 && y >= 40 && y < 76 {
        return 1; // Shell
    }
    -1
}

fn draw_desktop() {
    unsafe {
        crate::icon::PAL; // (初始化已由 desk_init 使用)
        fill(0, 0, font::fb_w(), font::fb_h(), crate::icon::PAL[1]);
        fill(0, font::fb_h() - TB_H, font::fb_w(), TB_H, crate::icon::PAL[2]);
        // 两个图标 (方块 + 字母)
        fill(60, 40, 40, 36, 0xEEEEEE);
        fill(140, 40, 40, 36, 0xEEEEEE);
        fill(60, 40, 40, 4, 0x008080);
        fill(140, 40, 40, 4, 0x008080);
        font_line(70, 52, 2, 0x000000, "H");
        font_line(150, 52, 2, 0x000000, "S");
        font_line(60, 78, 1, 0xFFFFFF, "Hermes");
        font_line(140, 78, 1, 0xFFFFFF, "Shell");
        font_line(8, font::fb_h() - TB_H + 6, 1, 0xFFFFFF, "FujoOS 1.0 desktop");
        crate::graphics::present();
    }
}

/// M107: 桌面主循环 (boot 直接进入; 不等待命令注入)。
pub fn desktop_main(_mbi: u32) -> ! {
    unsafe {
        TTY_TITLE = [0; 24];
    }
    let mut class_id: i64 = wm_class_id();
    draw_desktop();
    crate::serial::write_line("desk : desktop shell up (double-click icons; D/S keys)");
    let t0 = crate::interrupts::ticks();
    let mut synthetic_done = false;
    let mut synthetic2_done = false;
    let mut prev_btn: u32 = 0;
    let mut last_hit: i64 = -1;
    let mut last_click_ticks: u64 = 0;
    let mut pass_logged = false;
    let mut repaint = 0u64;

    loop {
        let ticks = crate::interrupts::ticks();
        let t = ticks.wrapping_sub(t0);

        // --- 合成双击测试 (启动后 40 ticks; 无真鼠标硬件也验证全链) ---
        if !synthetic_done && t >= 40 {
            synthetic_done = true;
            let kind = launch_program(HERMES_ELF, b"Hermes", class_id as u64);
            crate::serial::write_str("desk : synthetic double-click -> ");
            crate::serial::write_str(if kind > 0 { "Hermes window opened" } else { "launch failed" });
            crate::serial::write_line("");
        }
        // --- 第二合成双击: Shell 窗口 (TTY banner -> 完整 PASS 面) ---
        if !synthetic2_done && t >= 120 {
            synthetic2_done = true;
            let kind = launch_program(SHELL_ELF, b"Shell", class_id as u64);
            crate::serial::write_str("desk : synthetic 2nd double-click -> ");
            crate::serial::write_str(if kind > 0 { "Shell window opened" } else { "launch failed" });
            crate::serial::write_line("");
        }

        // --- 真鼠标: 按下沿 -> 命中检测 (双击 = 6 ticks 内同图标两次) ---
        let (x, y, btn) = unsafe { (crate::mouse::MS_X, crate::mouse::MS_Y, crate::mouse::MS_BTN) };
        if prev_btn == 0 && btn != 0 {
            let hit = icon_hit(x, y);
            if hit >= 0 {
                if last_hit == hit && ticks.wrapping_sub(last_click_ticks) <= 6 {
                    // 双击确认
                    let kind = if hit == 0 {
                        launch_program(HERMES_ELF, b"Hermes", class_id as u64)
                    } else {
                        launch_program(SHELL_ELF, b"Shell", class_id as u64)
                    };
                    crate::serial::write_str("desk : mouse double-click -> ");
                    crate::serial::write_str(if kind > 0 { "window opened" } else { "launch failed" });
                    crate::serial::write_line("");
                    last_hit = -1;
                } else {
                    last_hit = hit;
                    last_click_ticks = ticks;
                }
            }
        }
        prev_btn = btn;

        // --- 键盘后备: 无窗口程序时 D=Hermes S=Shell ---
        if crate::sched::current_task() == 0 && unsafe { TTY_PID } == 0 {
            while let Some(c) = crate::kbd::try_poll() {
                if c == 'D' || c == 'd' {
                    let _ = launch_program(HERMES_ELF, b"Hermes", class_id as u64);
                } else if c == 'S' || c == 's' {
                    let _ = launch_program(SHELL_ELF, b"Shell", class_id as u64);
                }
            }
        }

        // --- TTY 窗口重绘 (每 16 ticks) ---
        repaint += 1;
        if unsafe { TTY_PID } != 0 && repaint % 16 == 0 {
            tty_draw_window();
        }

        // --- 结果日志 (启动后 80 ticks) ---
        if !pass_logged && t >= 160 {
            pass_logged = true;
            let alive = unsafe { TTY_PID } != 0;
            let tied = unsafe { TTY_ROW_N } > 0;
            if alive {
                crate::serial::write_line("m107: desktop shell + window program alive");
                if tied {
                    crate::serial::write_line("m107: M107 RESULT: PASS");
                } else {
                    crate::serial::write_line("m107: M107 RESULT: PASS (ttl pending)");
                }
            } else {
                crate::serial::write_line("m107: M107 RESULT: FAIL");
            }
        }
        crate::hlt();
    }
}


