//! ipc.rs — M18 IPC 原语 v0 (管道 / 共享内存 / 信号)
//!
//! 三件套 (fujo 原生槽, 供 ring3 与多任务使用):
//!   0x5110 fujo_pipe(ptr)  创建管道 -> 在 ptr 处写 [rfd, wfd] (两个 i32)
//!   0x5111 fujo_shm()      返回共享内存窗口基址 (全局固定 0xA00000, 64KiB)
//!   0x5120 fujo_sigset(h)  注册当前任务信号处理函数 (用户地址)
//!   0x5121 fujo_sigkill(tid, sig)  向任务投递信号 (置 pending 位)
//!   0x5122 fujo_sigret()   复位本任务 sig_active (处理函数返回前调用)
//!
//! 模型注: M13 多任务共享同一地址空间 (用户页恒等映射, 无每进程页表), 因此
//! 共享内存 = 固定窗口 (天然共享) —— 管道与信号是真正的内核原语。
//!
//! 信号投递: PIT 中断发生于用户态 (CS=0x23) 且当前任务 pending 时, 在保存帧上
//! 构造用户栈帧 [RIP][CS][RFLAGS][RSP][SS] (iretq 序), 将 RIP 改写为 handler;
//! handler 以 `iretq` 返回被中断点 —— 经典 trampoline。

use crate::serial;

pub const PIPE_MAX: usize = 8;
pub const PIPE_SIZE: usize = 512;
pub const SHM_BASE: u64 = 0xA00000;
pub const SHM_LEN: usize = 0x10000; // 64KiB

#[derive(Clone, Copy)]
pub struct Pipe {
    pub used: bool,
    pub data: [u8; PIPE_SIZE],
    pub head: usize,
    pub len: usize,
    /// 存活端点计数 (两端关闭 -> 回收槽)
    pub ends: u8,
}

static mut PIPES: [Pipe; PIPE_MAX] = [Pipe { used: false, data: [0; PIPE_SIZE], head: 0, len: 0, ends: 0 }; PIPE_MAX];

fn print_dec(v: u64) {
    let mut buf = [0u8; 24];
    let mut i = 24;
    let mut x = v;
    if x == 0 {
        serial::write_str("0");
        return;
    }
    while x > 0 {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    serial::write_str(core::str::from_utf8(&buf[i..]).unwrap());
}

fn print_hex(v: u64) {
    const HX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..16 {
        let d = ((v >> (4 * (15 - i))) & 0xF) as u8;
        buf[2 + i] = HX[d as usize];
    }
    serial::write_str(core::str::from_utf8(&buf).unwrap());
}

/// 建立管道: 分配 Pipe + 两个 fd, 写 [rfd, wfd] (i32) 到用户 ptr。
/// 返回 0 成功; -24 (-EMFILE) / -12 (-ENOMEM)。
pub fn fujo_pipe(ptr: u64) -> i64 {
    unsafe {
        if !(0x400000..0xC00000).contains(&ptr) || ptr + 8 > 0xC00000 {
            return -14; // -EFAULT
        }
        // 找空闲 Pipe
        let mut pi: Option<usize> = None;
        for i in 0..PIPE_MAX {
            if !PIPES[i].used {
                pi = Some(i);
                break;
            }
        }
        let pi = match pi {
            Some(p) => p,
            None => return -12, // -ENOMEM (pipe 用尽)
        };
        // fd 分配 (经 vfs 表登记, 保证 read/write/close 路径统一)
        let rfd = match crate::vfs::alloc_pipe_fd(&mut PIPES[pi]) {
            Some(r) => r,
            None => return -24, // -EMFILE
        };
        let wfd = match crate::vfs::alloc_pipe_fd(&mut PIPES[pi]) {
            Some(w) => w,
            None => {
                crate::vfs::free_fd(rfd as usize);
                return -24; // -EMFILE
            }
        };
        PIPES[pi].used = true;
        PIPES[pi].head = 0;
        PIPES[pi].len = 0;
        PIPES[pi].ends = 2; // 读写两端
        // 写入用户数组 [rfd, wfd]
        (ptr as *mut u32).write(rfd as u32);
        (ptr as *mut u32).add(1).write(wfd as u32);
        serial::write_str("ipc  : pipe created rfd=");
        print_dec(rfd as u64);
        serial::write_str(" wfd=");
        print_dec(wfd as u64);
        serial::write_line("");
        0
    }
}

/// 管道端点关闭: 递减 ends, 返回是否两端全关 (Some(pi) 需要回收)。
pub fn pipe_end_close(p: *const Pipe) -> Option<usize> {
    unsafe {
        let pp = p as *mut Pipe;
        if (*pp).ends > 0 {
            (*pp).ends -= 1;
        }
        if (*pp).ends == 0 {
            // 需要回收: 从 PIPES 找槽索引
            for i in 0..PIPE_MAX {
                if core::ptr::eq(core::ptr::addr_of!(PIPES[i]) as *const u8, p as *const u8) {
                    return Some(i);
                }
            }
        }
        None
    }
}

/// 回收 Pipe 槽 (双端关闭后): 清空数据 + used=false。
pub fn pipe_recycle(idx: usize) {
    unsafe {
        if idx < PIPE_MAX && PIPES[idx].used {
            PIPES[idx].used = false;
            PIPES[idx].head = 0;
            PIPES[idx].len = 0;
            PIPES[idx].ends = 0;
            serial::write_str("ipc  : pipe slot ");
            print_dec(idx as u64);
            serial::write_line(" recycled (both ends closed)");
        }
    }
}

/// 管道读: 从 pipe 拷贝 min(len, 可用) 字节到用户 buf; 空则返回 0。
pub fn pipe_read(p: *const Pipe, buf: u64, len: u64) -> i64 {
    unsafe {
        if !(0x400000..0xC00000).contains(&buf) {
            return -14; // -EFAULT
        }
        let pp = p as *mut Pipe;
        let av = (*pp).len;
        let n = (len as usize).min(av).min(PIPE_SIZE);
        let mut k = 0usize;
        while k < n {
            let idx = ((*pp).head + k) % PIPE_SIZE;
            core::ptr::write_volatile(
                (buf as *mut u8).add(k),
                (*pp).data[idx],
            );
            k += 1;
        }
        (*pp).head = ((*pp).head + n) % PIPE_SIZE;
        (*pp).len -= n;
        n as i64
    }
}

/// 管道写: 拷贝 min(len, 空闲) 字节; 满则写放得下的。
pub fn pipe_write(p: *mut Pipe, ptr: u64, len: u64) -> i64 {
    unsafe {
        if !(0x400000..0xC00000).contains(&ptr) {
            return -14; // -EFAULT
        }
        let free = PIPE_SIZE - (*p).len;
        let n = (len as usize).min(free);
        let start = ((*p).head + (*p).len) % PIPE_SIZE;
        let mut k = 0usize;
        while k < n {
            let idx = (start + k) % PIPE_SIZE;
            (*p).data[idx] = (ptr as *const u8).add(k).read_volatile();
            k += 1;
        }
        (*p).len += n;
        n as i64
    }
}

/// fujo_shm(): 返回共享窗口基址 (全局固定, 多任务同名同址即共享)。
pub fn fujo_shm() -> i64 {
    // M19: 每次获取登记一个 SHM 对象 (引用语义 v0: 每次调用 +1)
    let _ = crate::kobj::alloc(crate::kobj::K_SHM, SHM_BASE);
    SHM_BASE as i64
}

/// fujo_sigset(h): 注册当前任务处理函数。
pub fn fujo_sigset(handler: u64) -> i64 {
    if handler != 0 && !(0x400000..0x800000).contains(&handler) {
        serial::write_line("ipc  : sigset bad handler -EINVAL");
        return -22; // -EINVAL
    }
    let tid = crate::sched::current_task();
    crate::sched::set_sig_handler(tid, handler);
    // M19: 信号对象登记 (每任务至多一条; 统计/审计)
    if handler != 0 {
        let _ = crate::kobj::alloc(crate::kobj::K_SIG, tid as u64);
    }
    serial::write_str("ipc  : task ");
    print_dec(tid as u64);
    serial::write_str(" sig handler @");
    print_hex(handler);
    serial::write_line("");
    0
}

/// fujo_sigkill(tid, sig): 置目标任务 pending。
pub fn fujo_sigkill(tid: u64, _sig: u64) -> i64 {
    let ok = crate::sched::sig_pending(tid as usize);
    if ok {
        serial::write_str("ipc  : sig -> task ");
        print_dec(tid);
        serial::write_line("");
        0
    } else {
        serial::write_str("ipc  : sigkill bad tid ");
        print_dec(tid);
        serial::write_line(" -EINVAL");
        -22
    }
}

/// fujo_sigret(): 处理函数完成, 复位 active (允许下次投递)。
pub fn fujo_sigret() -> i64 {
    crate::sched::clear_sig_active(crate::sched::current_task());
    0
}
