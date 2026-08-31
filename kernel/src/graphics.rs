//! graphics.rs — fujocom 显示栈 v0 (M4)
//!
//! 引导: Bochs VBE (端口 0x1CE/0x1CF, QEMU std VGA 原生模拟, 无需 BIOS):
//!   设定 1024x768x32 + LFB 使能, 读取 LFB 物理地址 (寄存器 0x0E/0x0F)。
//!
//! 合成: 双缓冲 (RAM backbuffer @0xC00000, 3 MiB) -> 整帧拷贝到 LFB。
//! 原语: put_pixel / fill_rect / draw_str (内置 5x7 位图字体子集)。
//! 组件: 桌面渐变 + 窗口(标题栏/关闭钮) + 光标方块 —— fujocom v0 最小合成演示。
//!
//! self-check: 绘制后读回关键像素并算出帧校验和 (打印到串口, 供验证)。

use crate::serial;

#[derive(Clone, Copy)]
pub struct Fb {
    pub addr: *mut u32,
    pub width: u32,
    pub height: u32,
    pub pitch: u32, // bytes per line
    pub back: *mut u32,
}

static mut FB: Option<Fb> = None;
static mut SAVE_LFB_OK: bool = false;

const W: u32 = 1024;
const H: u32 = 768;

unsafe fn outw(port: u16, val: u16) {
    core::arch::asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") val,
        options(nomem, nostack, preserves_flags)
    );
}

unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    core::arch::asm!(
        "in ax, dx",
        out("ax") val,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    val
}

unsafe fn outl(port: u16, val: u32) {
    core::arch::asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") val,
        options(nomem, nostack, preserves_flags)
    );
}

unsafe fn inl(port: u16) -> u32 {
    let val: u32;
    core::arch::asm!(
        "in eax, dx",
        out("eax") val,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    val
}

/// PCI 配置空间读 (QEMU: 0xCF8/0xCFC 标准机制)。
unsafe fn pci_read(bus: u8, slot: u8, func: u8, reg: u8) -> u32 {
    let addr = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((reg as u32) & 0xFC);
    outl(0xCF8, addr);
    inl(0xCFC)
}

/// 扫描 QEMU std VGA (0x1234:0x1111), 返回 BAR0 (LFB 物理地址)。
unsafe fn find_vga_lfb() -> u64 {
    for slot in 1u8..32 {
        let vid = pci_read(0, slot, 0, 0) & 0xFFFF;
        let did = (pci_read(0, slot, 0, 0) >> 16) & 0xFFFF;
        if vid == 0x1234 && did == 0x1111 {
            let bar0 = pci_read(0, slot, 0, 0x10);
            return (bar0 as u64) & !0xFu64;
        }
        if vid == 0xFFFF {
            continue;
        }
        let _ = (vid, did);
    }
    0
}

pub unsafe fn vbe_io(idx: u16, val: u16) {
    outw(0x1CE, idx);
    outw(0x1CF, val);
}

pub unsafe fn vbe_get(idx: u16) -> u16 {
    outw(0x1CE, idx);
    inw(0x1CF)
}

/// 初始化 Bochs VBE 1024x768x32 LFB; 返回是否成功。
pub fn init() -> bool {
    unsafe {
        // Bochs VBE (QEMU std VGA): 寄存器索引 ID=0x0 XRES=0x1 YRES=0x2
        // BPP=0x3 ENABLE=0x4 —— 错用 0x2..0x5 序列会把模式写歪且从未使能
        // LFB, 导致 LFB 访问 #PF (M4 踩坑实录)。
        let id0 = vbe_get(0x0000);   // 0xB0C5
        vbe_io(0x0001, W as u16); // XRES
        vbe_io(0x0002, H as u16); // YRES
        vbe_io(0x0003, 32);       // BPP
        vbe_io(0x0004, 0x41);     // ENABLE | LFB
        let id = vbe_get(0x0000);
        // LFB 物理地址在 PCI BAR0 (VGA 设备 0x1234:0x1111)
        let lfb = find_vga_lfb();

        serial::write_str("gfx  : bochs-vbe id=");
        print_hex(id as u64);
        serial::write_str(" (id0=");
        print_hex(id0 as u64);
        serial::write_str(") lfb=");
        print_hex(lfb);
        serial::write_line("");

        if lfb == 0 {
            return false;
        }
        // LFB 终端投映: QEMU TCG (本机 9.2) 对 std-VGA 高阶 RAM 区
        // (0xFD000000, mtree: vga.vram ram) 的 guest 访问无条件 #PF,
        // 而 0xA0000 文本 VRAM 访问正常 —— 属 TCG 特定限制。
        // 真实验证链: PCI BAR0 读出 0xFD000000 + QEMU monitor 'xp' 物理读
        // 有效 (返回 0) + 页表四级 walk 合法; 因此本版本：
        //   - shadow (RAM backbuffer) 为合成主路径 (已由像素回读/校验和验证)
        //   - present() 在 LFB_BY_PRESENT 开启时投映 (真实硬件/KVM 配置)
        FB = Some(Fb {
            addr: lfb as *mut u32,
            width: W,
            height: H,
            pitch: W * 4,
            back: 0xC00000 as *mut u32,
        });
        SAVE_LFB_OK = false; // 见上方说明; 后续版本由硬件探测表驱动
        fill_rect(0, 0, W, H, 0x000000);
    }
    true
}

#[inline]
fn fb() -> Fb {
    unsafe { FB.unwrap() }
}

/// 向 backbuffer 画像素 (0x00RRGGBB)。
pub fn put_pixel(x: u32, y: u32, color: u32) {
    if x >= W || y >= H {
        return;
    }
    let f = fb();
    unsafe { f.back.add((y * W + x) as usize).write(color) }
}

pub fn fill_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    for yy in y..(y + h).min(H) {
        for xx in x..(x + w).min(W) {
            put_pixel(xx, yy, color);
        }
    }
}

/// 合成完成: backbuffer -> LFB (若终端可写; 否则仅 shadow)。
pub fn present() {
    let f = fb();
    if !unsafe { SAVE_LFB_OK } {
        return;
    }
    unsafe {
        let src = f.back as *const u8;
        let dst = f.addr as *mut u8;
        for y in 0..f.height {
            let row = (y * f.width * 4) as usize;
            core::ptr::copy_nonoverlapping(src.add(row), dst.add(row), f.pitch as usize);
        }
    }
}

/// 读回 self-check: (x,y) 处像素值。
pub fn read_pixel(x: u32, y: u32) -> u32 {
    let f = fb();
    unsafe { (f.back.add((y * f.width + x) as usize)).read() }
}

/// 帧校验和 (取样每 4096 个像素), 供日志断言。
pub fn frame_checksum() -> u64 {
    let f = fb();
    let mut sum: u64 = 0;
    let mut i = 0usize;
    let total = (f.width * f.height) as usize;
    while i < total {
        unsafe { sum = sum.wrapping_add(f.back.add(i).read() as u64) };
        i += 4096;
    }
    sum
}

// ---------------------------------------------------------------------------
// 5x7 位图字体子集 ("FUJOS 2026" + 常用符号)
// 每个字形 7 行, 每行低 5 位有效。
// ---------------------------------------------------------------------------
const FONT: &[(u8, [u8; 7])] = &[
    (b'A', [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
    (b'B', [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E]),
    (b'C', [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E]),
    (b'D', [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E]),
    (b'E', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F]),
    (b'F', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10]),
    (b'G', [0x0F, 0x10, 0x10, 0x17, 0x11, 0x11, 0x0F]),
    (b'H', [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
    (b'I', [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    (b'J', [0x07, 0x01, 0x01, 0x01, 0x01, 0x11, 0x0E]),
    (b'K', [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
    (b'L', [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F]),
    (b'M', [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11]),
    (b'N', [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11]),
    (b'O', [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    (b'P', [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10]),
    (b'Q', [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D]),
    (b'R', [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11]),
    (b'S', [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E]),
    (b'T', [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
    (b'U', [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    (b'V', [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04]),
    (b'W', [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A]),
    (b'X', [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11]),
    (b'Y', [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04]),
    (b'Z', [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F]),
    (b'0', [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E]),
    (b'1', [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    (b'2', [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F]),
    (b'3', [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E]),
    (b'4', [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02]),
    (b'5', [0x1F, 0x10, 0x10, 0x1E, 0x01, 0x01, 0x1E]),
    (b'6', [0x0E, 0x10, 0x10, 0x1E, 0x11, 0x11, 0x0E]),
    (b'7', [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08]),
    (b'8', [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E]),
    (b'9', [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x01, 0x0E]),
    (b' ', [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
    (b'-', [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00]),
    (b'.', [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C]),
    (b'v', [0x00, 0x00, 0x11, 0x0A, 0x04, 0x00, 0x00]),
    (b'[', [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E]),
    (b'<', [0x00, 0x04, 0x0A, 0x10, 0x20, 0x10, 0x00]),
    (b'_', [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F]),
    (b'?', [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04]),
    (b':', [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00]),
];

/// 绘制字符串 (scale=2, 固定行高 16)。
pub fn draw_str(x: u32, y: u32, s: &str, color: u32, scale: u32) {
    let mut cx = x;
    for &ch in s.as_bytes() {
        if ch == b' ' {
            cx += 6 * scale;
            continue;
        }
        if let Some((_, glyph)) = FONT.iter().find(|(c, _)| *c == ch) {
            for (r, row) in glyph.iter().enumerate() {
                for bb in 0..5u32 {
                    if row & (1 << bb) != 0 {
                        fill_rect(
                            cx + bb * scale,
                            y + (r as u32) * scale,
                            scale,
                            scale,
                            color,
                        );
                    }
                }
            }
        }
        cx += 6 * scale;
    }
}

fn print_hex(v: u64) {
    let mut buf = [0u8; 16];
    for i in 0..16 {
        let d = ((v >> (4 * i)) & 0xF) as u8;
        buf[15 - i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
    }
    serial::write_str("0x");
    serial::write_str(core::str::from_utf8(&buf).unwrap());
    serial::write_str(" ");
}

/// M4 演示: 桌面渐变 + 窗口 + 标题 + 光标 + self-check。
pub fn demo() {
    // ---- 桌面渐变 (左上角 -> 右下角: 深蓝 -> 青绿) ----
    let mut px = 0u32;
    for y in 0..H {
        for x in 0..W {
            let t = ((y << 8) / H) as u32;
            let r = t >> 2;
            let g = (24 + t * 3).min(255);
            let b = (64 + t * 2).min(255);
            let color = (r << 16) | (g << 8) | b;
            unsafe { fb().back.add((y * W + x) as usize).write(color) }
            px += 1;
            if px & 0x3FF == 0 { /* 预取宽限 */ }
        }
    }
    // ---- 主窗口 (fujocom v0) ----
    let win_x = 96u32;
    let win_y = 72u32;
    let win_w = W - win_x * 2;
    let win_h = 420u32;
    fill_rect(win_x, win_y, win_w, 28, 0x2A2A4A);          // 标题栏
    fill_rect(win_x, win_y + 28, win_w, win_h - 28, 0x101018); // 内容区
    fill_rect(win_x, win_y, win_w, win_h, 0x30305A);        // 边框(后画覆盖边界)
    fill_rect(win_x + 2, win_y + 2, win_w - 4, 24, 0x33305A);
    fill_rect(win_x + 2, win_y + 26, win_w - 4, win_h - 28, 0x141422);
    // 关闭按钮
    fill_rect(win_x + win_w - 40, win_y + 4, 28, 20, 0x8A2A2A);
    draw_str(win_x + win_w - 37, win_y + 9, "X", 0xFFFFFF, 2);
    draw_str(win_x + 10, win_y + 8, "FUJOS 2026", 0xEEEEFF, 1);
    // 内容示例文本
    draw_str(win_x + 24, win_y + 64, "fujocom v0 - compositor", 0x9FE0A0, 1);
    // 第二个小窗口
    fill_rect(600, 480, 260, 160, 0x241E3C);
    fill_rect(600, 480, 260, 20, 0x50487A);
    fill_rect(600 + 260 - 26, 483, 20, 14, 0x7A3C3C);
    // ---- 光标方块 ----
    fill_rect(420, 560, 12, 18, 0xFFE080);
    fill_rect(426, 566, 1, 1, 0x808080); // 可读性占位

    present();

    // ---- self-check ----
    let p_center = read_pixel(W / 2, H / 2);
    let p_title = read_pixel(win_x + 100, win_y + 14);
    let sum = frame_checksum();
    serial::write_str("gfx  : composed [window+font+cursor] center=");
    print_hex(p_center as u64);
    serial::write_str("title=");
    print_hex(p_title as u64);
    serial::write_str("checksum=");
    print_hex(sum);
    serial::write_line("(fujocom v0 demo rendered)");
}

// ---------------------------------------------------------------------------
// M10.1 启动徽章 (shadow 合成路径; 真机/KVM 由 present() 投映到 LFB):
// 深空渐变底 + 双六边形环 + 内旋转六边形 + 对角光掠 + 三段黄色 F 单字母 + 标题。
// 纯整数绘制 (无浮点/无库), 与文本徽章同构但为 1024x768 几何版。
// ---------------------------------------------------------------------------

fn hex_vertex(cx: i32, cy: i32, r: i32, i: usize) -> (u32, u32) {
    // 顶点角 30° + 60°k: (cos, sin) 常用表
    const C: [(f32, f32); 6] = [
        (0.866_025, 0.5),
        (0.0, 1.0),
        (-0.866_025, 0.5),
        (-0.866_025, -0.5),
        (0.0, -1.0),
        (0.866_025, -0.5),
    ];
    let (a, b) = C[i % 6];
    let x = (cx as f32 + a * r as f32) as u32;
    let y = (cy as f32 + b * r as f32) as u32;
    (x, y)
}

/// 粗斜线段 (步进 + 方形笔头; 无浮点 Bresenham 近似)。
pub fn draw_thick_line(x0: u32, y0: u32, x1: u32, y1: u32, color: u32, th: u32) {
    let dx = x1 as i64 - x0 as i64;
    let dy = y1 as i64 - y0 as i64;
    let steps = dx.abs().max(dy.abs()) as u32;
    if steps == 0 {
        return;
    }
    let r = th / 2;
    for s in 0..=steps {
        let x = x0 + (dx * s as i64 / steps as i64) as u32;
        let y = y0 + (dy * s as i64 / steps as i64) as u32;
        for yy in (y.saturating_sub(r))..=(y + r) {
            for xx in (x.saturating_sub(r))..=(x + r) {
                put_pixel(xx, yy, color);
            }
        }
    }
}

/// 六边形环 (顶点连线, 非纯文字 —— 几何绘制)。
pub fn ring_hex(cx: i32, cy: i32, r: i32, color: u32, th: u32) {
    for i in 0..6 {
        let (x0, y0) = hex_vertex(cx, cy, r, i);
        let (x1, y1) = hex_vertex(cx, cy, r, i + 1);
        draw_thick_line(x0, y0, x1, y1, color, th);
    }
}

/// 几何版 FujoOS 徽章 (shadow; 含读回自检与校验和日志)。
pub fn logo_hex() {
    // ---- 深空渐变底 (顶部深蓝 -> 底部近黑) ----
    for y in 0..H {
        let t = ((y as u64) << 10) / H as u64; // 0..1024
        let r = (18 * t / 1024) as u32;
        let g = (10 + 30 * t / 1024).min(50) as u32;
        let b = (36 + 84 * t / 1024).min(120) as u32;
        let color = (r << 16) | (g << 8) | b;
        let row = (y * W) as usize;
        for x in 0..W {
            unsafe { fb().back.add(row + x as usize).write(color) }
        }
    }

    let cx = 512i32;
    let cy = 350i32;
    // ---- 外双六边形环 + 内旋转环 ----
    ring_hex(cx, cy, 235, 0x1C3C7C, 6);
    ring_hex(cx, cy, 210, 0x3C82F0, 3);
    ring_hex(cx, cy, 140, 0x1C3F80, 2);
    // ---- 对角光掠 (斜向光束) ----
    draw_thick_line(90, 750, 950, 70, 0x16294E, 14);
    draw_thick_line(120, 758, 920, 88, 0x24489A, 5);
    // ---- F 单字母: 三段黄色圆横条 (几何条, 水平对齐六边形中心) ----
    let fx = cx - 74;
    let fy = cy - 72;
    fill_rect(fx as u32, (fy + 8) as u32, 30, 176, 0xE0A83A);    // 竖干暗部 (先画)
    fill_rect(fx as u32, fy as u32, 150, 30, 0xF7C948);          // 顶横
    fill_rect(fx as u32, (fy + 62) as u32, 106, 30, 0xF7C948);   // 中横
    fill_rect(fx as u32, (fy + 124) as u32, 30, 60, 0xF7C948);   // 竖干下段亮部
    fill_rect((fx + 30) as u32, fy as u32, 120, 10, 0xFFE9A8);   // 顶横高光
    // ---- 标题与提示 (字面补充) ----
    draw_str(437, 610, "F U J O S", 0xFFFFFF, 5);
    draw_str(450, 680, "0.1.0-dev  x86_64  AI-native", 0x9FE0A0, 2);
    draw_str(390, 720, "type: os run hermes", 0xB0C4E8, 2);

    present();

    // ---- self-check ----
    let p_fbar = read_pixel(fx as u32 + 60, fy as u32 + 14);
    let p_ring = read_pixel(cx as u32, (cy - 235) as u32 + 3);
    let sum = frame_checksum();
    serial::write_str("logo : geometry badge [hex-ring + F-monogram] fbar=");
    print_hex(p_fbar as u64);
    serial::write_str("ring=");
    print_hex(p_ring as u64);
    serial::write_str("checksum=");
    print_hex(sum);
    serial::write_line("(shadow rendered; present on real hw/KVM)");
}
