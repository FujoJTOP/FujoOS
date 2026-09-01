//! shader.rs — M62: 着色器内核评估 (compute 子集 v0)
//!
//! 每像素字节码 VM (32B 上限, 8 寄存器 u32, 解释执行):
//!   op: 0=halt 1=const r,v 2=add r,a,b 3=mul r,a,b 4=sub r,a,b
//!       5=color r (r = argb 色) 6=idx (r0 = y*W+x)
//! 执行: 每像素: r0=idx, 跑程序, r8/r7 组合? -> 简化: 寄存器 r1 = 输出色。
//! 0x6901 shader_load(ptr, n) / 0x6902 shader_run(x,y,w,h) /
//! 0x6903 shader_pixel(x,y) 读回 / 0x6904 shader_ops() 统计。
//! 评估文档: docs/11-shader-eval.md。

use crate::font;

static mut PROG: [u32; 32] = [0; 32]; // 每指令字: op<<24 | r<<16 | a<<8 | b
static mut PROG_LEN: usize = 0;
static mut OPS: u64 = 0;

fn setp(x: u64, y: u64, c: u32) {
    if x >= font::fb_w() as u64 || y >= font::fb_h() as u64 {
        return;
    }
    unsafe {
        let p = (font::BACKBUFFER + (y * font::fb_w() as u64 + x) * 4) as *mut u32;
        p.write(c);
    }
}

/// 0x6901
pub fn fujo_shader_load(ptr: u64, n: u64) -> i64 {
    unsafe {
        PROG_LEN = (n as usize).min(32);
        for i in 0..PROG_LEN {
            PROG[i] = ((ptr as *const u32).add(i)).read();
        }
    }
    0
}

/// 0x6902: 区域执行.
pub fn fujo_shader_run(x: u64, y: u64, w: u64, h: u64) -> i64 {
    let mut regs = [0u32; 8];
    for py in 0..h {
        for px in 0..w {
            regs = [0; 8];
            regs[0] = ((y + py) * font::fb_w() as u64 + (x + px)) as u32;
            let mut pc = 0usize;
            unsafe {
                while pc < PROG_LEN {
                    let opcode = (PROG[pc] >> 24) & 0xFF;
                    let r = ((PROG[pc] >> 16) & 0xFF) as usize;
                    let a = ((PROG[pc] >> 8) & 0xFF) as usize;
                    let b = (PROG[pc] & 0xFF) as usize;
                    OPS += 1;
                    match opcode {
                        0 => break,
                        1 => regs[r] = b as u32,
                        2 => regs[r] = regs[a].wrapping_add(regs[b]),
                        3 => regs[r] = regs[a].wrapping_mul(regs[b]),
                        4 => regs[r] = regs[a].wrapping_sub(regs[b]),
                        5 => regs[r] = ((b as u32) << 8) | regs[a], // 组合色
                        6 => regs[0] = ((y + py) * font::fb_w() as u64 + (x + px)) as u32,
                        _ => break,
                    }
                    pc += 1;
                }
            }
            setp(x + px, y + py, regs[1]);
        }
    }
    0
}

/// 0x6903
pub fn fujo_shader_pixel(x: u64, y: u64) -> i64 {
    if x >= font::fb_w() as u64 || y >= font::fb_h() as u64 {
        return 0;
    }
    unsafe {
        let p = (font::BACKBUFFER + (y * font::fb_w() as u64 + x) * 4) as *const u32;
        p.read() as i64
    }
}

/// 0x6904: 已执行指令数 (性能面).
pub fn fujo_shader_ops() -> i64 {
    unsafe { OPS as i64 }
}
