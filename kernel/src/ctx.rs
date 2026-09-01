//! ctx.rs — M89: fujoctx 升级 (窗口焦点/文件变更/syscall 摘要注入)
//!
//! 上下文注入面: 生成一行摘要 (供 0x5102 fujo_ai_fetch 上下文旅):
//!   "fujoctx v1 win_focus=N files=N syscalls=N ticks=N\n"
//! win_focus: wmsg 焦点窗口 (0=none/占位); files: VFS 写计数;
//! syscalls: M68 计数器 CTR[1]; ticks: PIT。
//! 接口: 0x7F01 ctx_snap(ptr, cap) → 摘要长度。
//!
//! M112 (AI For Next ② 感知通道): 结构态 fujoctx v2 + 事件环形缓冲。
//!   0x8002 ctx_subscribe(mask)  —— 单订阅槽: 设 mask + 读游标复位
//!   0x8003 ctx_events(ptr,cap)  —— 拷贝 mask 内事件 (40B/条), 游标推进
//!   0x8004 ctx_inject(kind,a,b) —— 注入合成事件 (demo/测试)
//!   0x8005 ctx_struct(ptr,cap)  —— 结构态文本 (tasks/win/rate/anom)

use crate::perf;
use crate::sched;
use crate::serial;
use crate::vfs;
use crate::wmsg;

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

// ---------------------------------------------------------------------------
// M112 · 感知通道: 事件环形缓冲 (AI 的眼睛) + 结构态
// ---------------------------------------------------------------------------

pub const EV_SYSCALL: u64 = 1;
pub const EV_FILE: u64 = 2;
pub const EV_WINDOW: u64 = 3;
pub const EV_EXIT: u64 = 4;
pub const EV_ANOMALY: u64 = 5;
/// 全部种类的 mask 位 (bit = kind-1)。
pub const EV_ANY: u64 = 0x1F;

const EV_N: usize = 128; // 128 槽 × 40B = 5KB (BSS)
const EV_SZ: usize = 5; // u64×5: ts, kind, pid, a, b

static mut EV: [[u64; EV_SZ]; EV_N] = [[0; EV_SZ]; EV_N];
static mut EV_W: usize = 0; // 写位置 (单调, mod N 取槽)
static mut EV_C: usize = 0; // 读游标 (订阅槽)
static mut EV_SUB: u64 = EV_ANY;

/// 内核/宿主事件注入 (环形覆盖, 满了先推游标)。
pub fn ev_push(kind: u64, pid: u64, a: u64, b: u64) {
    unsafe {
        let s = &mut EV[EV_W % EV_N];
        s[0] = crate::interrupts::ticks();
        s[1] = kind & 0xFF;
        s[2] = pid;
        s[3] = a;
        s[4] = b;
        EV_W += 1;
        if EV_W - EV_C > EV_N {
            EV_C = EV_W - EV_N; // 消费者太慢: 丢弃最旧
        }
    }
}

/// 0x8002: 订阅 (单槽: mask + 游标复位)。
pub fn fujo_ctx_subscribe(mask: u64) -> i64 {
    unsafe {
        EV_SUB = mask;
        EV_C = EV_W;
        serial::write_str("ctx  : subscribe mask=");
        crate::syscall::debug_hex(mask);
        serial::write_line("");
    }
    0
}

/// 0x8003: 拷贝 mask 内事件 (40B/条: ts,kind,pid,a,b), 游标推进 (未进 mask 的
/// 被消费跳过 —— "订阅只收 mask 内事件")。返回条数; 缓冲不足则丢失尾部。
pub fn fujo_ctx_events(ptr: u64, cap: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) {
        return -14; // -EFAULT
    }
    let b = ptr as *mut u64;
    let cap = cap as usize;
    unsafe {
        let mut n = 0usize;
        let mut c = EV_C;
        while c < EV_W {
            let s = EV[c % EV_N];
            let kind = s[1];
            let pass = (EV_SUB & (1u64 << ((kind & 63) - 1))) != 0;
            c += 1;
            if pass {
                if (n + 1) * (EV_SZ * 8) > cap {
                    break; // 缓冲不足: 剩余未消费
                }
                for k in 0..EV_SZ {
                    b.add(n * EV_SZ + k).write(s[k]);
                }
                n += 1;
            }
        }
        EV_C = c;
        n as i64
    }
}

/// 0x8004: 注入合成事件 (demo/回归测试)。
pub fn fujo_ctx_inject(kind: u64, a: u64, b: u64) -> i64 {
    ev_push(kind, crate::sched::current_task() as u64, a, b);
    0
}

/// 结构态文本构建器 (0x8005 + shm 上下文共用):
/// "fujoctx v2 tasks=K win=W rate=R anom=A events=W-C\n" + " t{pid}:{state}"...
pub fn ctx_build_text(b: *mut u8, cap: usize) -> usize {
    let mut pos = 0usize;
    put(b, &mut pos, cap, b"fujoctx v2 tasks=");
    put_dec(b, &mut pos, cap, sched::task_count() as u64);
    put(b, &mut pos, cap, b" win=");
    put_dec(b, &mut pos, cap, wmsg::wm_count() as u64);
    put(b, &mut pos, cap, b" rate=");
    put_dec(b, &mut pos, cap, sys_sampled());
    put(b, &mut pos, cap, b" anom=");
    put_dec(b, &mut pos, cap, crate::ai::anom_total());
    put(b, &mut pos, cap, b" events=");
    put_dec(b, &mut pos, cap, unsafe { (EV_W - EV_C) as u64 });
    for i in 0..crate::sched::MAX_TASKS {
        let st = sched::task_state(i);
        if st != 0 {
            put(b, &mut pos, cap, b" t");
            put_dec(b, &mut pos, cap, i as u64);
            put(b, &mut pos, cap, b":");
            put_dec(b, &mut pos, cap, st as u64);
        }
    }
    if pos < cap {
        unsafe { b.add(pos).write(b'\n') };
        pos += 1;
    }
    pos
}

/// M112: syscall 采样注记 (syscall.rs 每 1000 次调用置一次)。
pub fn sys_note(pid: u64, total: u64) {
    unsafe {
        SYS_SAMPLE = total;
    }
    ev_push(EV_SYSCALL, pid, total, 0);
}

fn sys_sampled() -> u64 {
    unsafe { SYS_SAMPLE }
}

static mut SYS_SAMPLE: u64 = 0;

/// 0x8005: 结构态 → 用户缓冲 (模型上下文/演示断言)。
pub fn fujo_ctx_struct(ptr: u64, cap: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) {
        return -14;
    }
    let n = ctx_build_text(ptr as *mut u8, cap as usize);
    serial::write_str("ctx  : struct -> ");
    crate::syscall::debug_dec(n as u64);
    serial::write_line(" bytes");
    n as i64
}

