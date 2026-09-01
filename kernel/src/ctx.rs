//! ctx.rs — M89: fujoctx 升级 (窗口焦点/文件变更/syscall 摘要注入)
//!
//! 上下文注入面: 生成一行摘要 (供 0x5102 fujo_ai_fetch 上下文旅):
//!   "fujoctx v1 win_focus=N files=N syscalls=N ticks=N\n"
//! win_focus: wmsg 焦点窗口 (0=none/占位); files: VFS 写计数;
//! syscalls: M68 计数器 CTR[1]; ticks: PIT。
//! 接口: 0x7F01 ctx_snap(ptr, cap) → 摘要长度。

use crate::perf;
use crate::vfs;

fn put(b: *mut u8, pos: &mut usize, cap: usize, s: &[u8]) -> usize {
    let n = s.len().min(cap.saturating_sub(*pos));
    for i in 0..n {
        unsafe { b.add(*pos + i).write(s[i]) };
    }
    *pos += n;
    n
}

fn put_dec(b: *mut u8, pos: &mut usize, cap: usize, v: u64) -> usize {
    let mut num = [0u8; 20];
    let mut i = 20usize;
    let mut x = v;
    if x == 0 {
        return put(b, pos, cap, b"0");
    }
    while x > 0 {
        i -= 1;
        num[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    put(b, pos, cap, &num[i..])
}

/// 0x7F01: 生成摘要到 (ptr), 返回长度 (cap 不足截断)。
pub fn fujo_ctx_snap(ptr: u64, cap: u64) -> i64 {
    let cap = cap as usize;
    let b = ptr as *mut u8;
    let mut pos = 0usize;
    let sys = perf::sys_delta();
    let ticks = crate::interrupts::ticks();
    let files = vfs::fs_writes();
    let focus: u64 = 0; // v0: 焦点窗口 id (后续 fujoctx 面)

    put(b, &mut pos, cap, b"fujoctx v1 win_focus=");
    put_dec(b, &mut pos, cap, focus);
    put(b, &mut pos, cap, b" files=");
    put_dec(b, &mut pos, cap, files);
    put(b, &mut pos, cap, b" syscalls=");
    put_dec(b, &mut pos, cap, sys);
    put(b, &mut pos, cap, b" ticks=");
    put_dec(b, &mut pos, cap, ticks);
    if pos < cap {
        unsafe { b.add(pos).write(b'\n') };
        pos += 1;
    }
    pos as i64
}

// ---------------------------------------------------------------------------
// M90: 上下文压缩 (fujoctx 链: 截断+摘要窗口策略; 委托宿主大模型面)
// ---------------------------------------------------------------------------

const MID: &[u8] = b"[...ctx-compressed...]";

/// 0x8001: 压缩: 保留头 win 字节 + 尾 win/2 字节, 中间替换标记。
pub fn fujo_ctx_compress(src: u64, len: u64, dst: u64, cap: u64, win: u64) -> i64 {
    let len = len as usize;
    let win = (win as usize).min(len / 3).max(8);
    let srcb = unsafe { core::slice::from_raw_parts(src as *const u8, len) };
    let b = dst as *mut u8;
    let cap = cap as usize;
    let mut pos = 0usize;

    // 头部
    for i in 0..win.min(cap) {
        unsafe { b.add(i).write(srcb[i]) };
    }
    pos = win.min(cap);
    // 中间标记
    for &c in MID.iter() {
        if pos < cap {
            unsafe { b.add(pos).write(c) };
            pos += 1;
        }
    }
    // 尾部
    let tail = (win / 2).min(len - win).max(1);
    for i in 0..tail.min(cap.saturating_sub(pos)) {
        unsafe { b.add(pos).write(srcb[len - tail + i]) };
        pos += 1;
    }
    pos as i64
}
