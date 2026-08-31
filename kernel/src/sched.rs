//! sched.rs — M13 抢占式调度 v0 (PIT 时间片轮转, 单核)
//!
//! Task 模型: 每任务独立内核栈 (保存现场/中断帧区) + 用户栈 + 状态。
//! 切换时机: PIT IRQ0 (100Hz) —— 桩保存全寄存器 → fujo_tick_sched 轮转 →
//! 桩把 rsp 换成下一任务的保存帧 → pop 寄存器 + iretq 恢复下一任务的
//! 用户上下文 (用户 RIP/RSP/RFLAGS 全在保存帧里, 现场零丢失)。
//! 仅当: 中断发生于用户态 (CS=0x23) 且任务数 >= 2 (否则零开销返回)。
//! 注: 内核态中断 (syscall 期) 不切换 —— v0 抢占只发生在用户态边界。

use crate::gdt;
use crate::serial;

pub const MAX_TASKS: usize = 8;
pub const TASK_RUNNABLE: u8 = 1;
pub const TASK_DEAD: u8 = 3;

#[derive(Clone, Copy)]
pub struct Task {
    pub saved_rsp: u64,
    pub kstack_top: u64,
    pub state: u8,
    /// M18 信号: 处理函数 (0=未注册), pending (待投递), active (处理中)
    pub sig_handler: u64,
    pub sig_pending: bool,
    pub sig_active: bool,
}

static mut TASKS: [Task; MAX_TASKS] = [
    Task { saved_rsp: 0, kstack_top: 0, state: 0, sig_handler: 0, sig_pending: false, sig_active: false };
    MAX_TASKS
];
static mut TASK_COUNT: usize = 0;
static mut CUR: usize = 0;
static mut SWITCHES: u64 = 0;
static mut MULTI: bool = false;

/// 切换桩引用的全局: 下一任务的内核栈保存帧指针 (桩 mov rsp, [rip + ...])。
#[no_mangle]
pub static mut sched_next_rsp: u64 = 0;
/// #PF 桩引用的切换标志: 崩溃任务终止后的目标任务帧 (桩先查再换栈)。
#[no_mangle]
pub static mut pf_must_switch: u64 = 0;

/// M14: 终止当前任务 (用户致命 #PF 等), 切换给下一存活任务。
/// 返回 true 表示已设置 pf_must_switch (桩下一次 iretq 前转场)。
pub fn terminate_current_and_next() -> bool {
    unsafe {
        if TASK_COUNT < 2 {
            return false; // 单任务: 让调用方走原有停机诊断
        }
        TASKS[CUR].state = TASK_DEAD;
        serial::write_str("proc: task ");
        print_dec(CUR as u64);
        serial::write_line(" terminated (crash isolated) - scheduling survivors");
        let mut next = (CUR + 1) % TASK_COUNT;
        let mut guard = 0usize;
        while TASKS[next].state != TASK_RUNNABLE && guard < TASK_COUNT {
            next = (next + 1) % TASK_COUNT;
            guard += 1;
        }
        if TASKS[next].state == TASK_RUNNABLE {
            CUR = next;
            sched_next_rsp = TASKS[next].saved_rsp;
            gdt::set_rsp0(TASKS[next].kstack_top);
            pf_must_switch = 1;
            true
        } else {
            false // 全部死亡: 调用方停机
        }
    }
}

/// shell `os run threads` 打开双任务模式。
pub fn set_multi() {
    unsafe {
        MULTI = true;
    }
}

pub fn multi_task() -> bool {
    unsafe { MULTI }
}

/// M59: 游戏模式 (前台调度标记; PIT 切换对游戏任务给独占轮)。
pub static mut GAME_MODE: bool = false;

pub fn set_game_mode(on: bool) {
    unsafe {
        GAME_MODE = on;
    }
}

pub fn game_mode() -> bool {
    unsafe { GAME_MODE }
}

/// 当前任务 id (M14: 演示/进程标识)。
pub fn current_task() -> usize {
    unsafe { CUR }
}

// ---------------------------------------------------------------------------
// M18 · 信号原语: 注册/投递/复位
// ---------------------------------------------------------------------------

pub fn set_sig_handler(tid: usize, handler: u64) {
    unsafe {
        if tid < TASK_COUNT {
            TASKS[tid].sig_handler = handler;
        }
    }
}

/// 置目标任务 pending (返回是否有效 tid)。
pub fn sig_pending(tid: usize) -> bool {
    unsafe {
        if tid < TASK_COUNT && TASKS[tid].state == TASK_RUNNABLE {
            TASKS[tid].sig_pending = true;
            true
        } else {
            false
        }
    }
}

pub fn clear_sig_active(tid: usize) {
    unsafe {
        if tid < TASK_COUNT {
            TASKS[tid].sig_active = false;
        }
    }
}

/// 信号投递: 当前任务 (中断于用户态) 有 pending 且注册 handler 且未在处理中时,
/// 在用户栈上构造 iretq 帧 [RIP][CS][RFLAGS][RSP][SS] (5×8B), 将中断保存帧的
/// RIP 改为 handler、RSP 指向新帧 —— handler 以 `iretq` 返回被中断点。
/// 返回 true = 已投递 (帧被改写)。
fn maybe_deliver_signal(regs: *mut u64) -> bool {
    unsafe {
        if CUR >= TASK_COUNT {
            return false; // M39 防护: CUR 越界直接跳过 (根因: 内核 BSS 与模块区
                          // 边界的写穿, 见 M39 记录)
        }
        let t = &mut TASKS[CUR];
        if !t.sig_pending || t.sig_active || t.sig_handler == 0 {
            return false;
        }
        let old_rsp = regs.add(12).read();
        if old_rsp < 0x80 {
            return false;
        }
        // handler 入口 RSP%16==8 (SysV); 帧头在其上方 40B 不重叠
        let new_rsp = (old_rsp & !0xF) - 8;
        let frame = new_rsp as *mut u64;
        frame.add(0).write(regs.add(9).read()); // 被中断 RIP
        frame.add(1).write(0x23u64); // CS
        frame.add(2).write(regs.add(11).read()); // RFLAGS
        frame.add(3).write(old_rsp); // 复原 RSP
        frame.add(4).write(0x1Bu64); // SS
        // 改写中断保存帧: 恢复位置 = handler
        regs.add(9).write(t.sig_handler);
        regs.add(12).write(new_rsp);
        t.sig_pending = false;
        t.sig_active = true;
        serial::write_str("ipc  : signal -> task ");
        print_dec(CUR as u64);
        serial::write_line("");
        true
    }
}

/// M22 fork: 克隆当前任务为独立任务 (共享地址空间 v0: 用户栈物理拷贝到 0x700000)。
/// 若当前任务未登记 (单任务隐式运行, TASK_COUNT==0), 先把父登记为 TASKS[0]
/// (帧 = 当前现场, rax=1=返回 tid), 再建子 TASKS[1] (rax=0)。
/// 返回子 tid; None = 任务表满。
pub fn fork_current(rip: u64, rsp: u64, regs: &[u64; 8]) -> Option<usize> {
    unsafe {
        if TASK_COUNT >= MAX_TASKS {
            return None;
        }
        // 构造保存帧 (布局同 spawn): [RIP][CS][RFLAGS][RSP][SS] + 9 槽
        // regs 序: [0]=r11 [1]=r10 [2]=r9 [3]=r8 [4]=rdi [5]=rsi [6]=rdx [7]=rcx
        // 嵌套 fn 不能继承外层 unsafe 上下文 -> 用宏内联。
        macro_rules! build_frame {
            ($kstack_top:expr, $rip:expr, $rsp:expr, $regs:expr, $rax:expr) => {{
                let fr = ($kstack_top - 0x40) as *mut u64;
                fr.add(0).write($rip);
                fr.add(1).write(0x23u64);
                fr.add(2).write(0x202u64);
                fr.add(3).write($rsp);
                fr.add(4).write(0x1Bu64);
                for k in 0..8usize {
                    fr.sub(9 - k).write($regs[k]);
                }
                fr.sub(1).write($rax); // rax 槽
                fr as u64 - 72
            }};
        }
        let user_stack: u64 = 0x700000;
        // 拷贝当前用户栈 (0x600000 -> 0x700000, 最多 64KiB)
        let copy_top = 0x600000u64;
        let copy_lo = (rsp & !0xFFF).max(copy_top - 0x10000);
        let mut from = copy_lo;
        while from < copy_top {
            let b = (from as *const u8).read_volatile();
            ((user_stack - (copy_top - from)) as *mut u8).write_volatile(b);
            from += 1;
        }
        let new_rsp = user_stack - (copy_top - rsp);
        // 父登记 (若隐式单任务): 无需构造帧 —— 父登记后继续运行,
        // 首次 PIT 用户态中断时 fujo_tick_sched 以真实现场覆盖 saved_rsp。
        // 关键: 必须立即 set_rsp0 到独立内核栈 0x380000 —— 若沿用 0x300000
        // (syscall 栈), 子任务调用 write 会覆盖父的保存帧 (iretq 目标帧变
        // 垃圾 -> #GP, M22 实证: tRIP=0x2fffd8 tCS=0x0)。
        // 0x200000 不可用: 镜像直接落在 0x100000..0x251D18 且入口 0x21xxxx。
        let mut parent_idx = CUR;
        if TASK_COUNT == 0 {
            parent_idx = 0;
            TASKS[0] = Task {
                saved_rsp: 0,
                kstack_top: 0x380000,
                state: TASK_RUNNABLE,
                sig_handler: 0,
                sig_pending: false,
                sig_active: false,
            };
            TASK_COUNT = 1;
            crate::gdt::set_rsp0(0x380000);
        }
        // 子任务 (idx = TASK_COUNT)
        let idx = TASK_COUNT;
        if idx >= MAX_TASKS {
            return None;
        }
        let saved = build_frame!(0x340000, rip, new_rsp, regs, 0);
        TASKS[idx] = Task {
            saved_rsp: saved,
            kstack_top: 0x340000,
            state: TASK_RUNNABLE,
            sig_handler: 0,
            sig_pending: false,
            sig_active: false,
        };
        TASK_COUNT += 1;
        serial::write_str("sched: fork parent=");
        print_dec(parent_idx as u64);
        serial::write_str(" child=");
        print_dec(idx as u64);
        serial::write_line("");
        Some(idx)
    }
}

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

/// 创建任务: 在 (kstack_top) 处压入初始 iret 帧 + 9 个零寄存器槽。
fn spawn(kstack_top: u64, user_stack: u64, entry: u64) -> usize {
    unsafe {
        let idx = TASK_COUNT;
        if idx >= MAX_TASKS {
            return 0;
        }
        let frame = (kstack_top - 0x40) as *mut u64;
        // iret 帧 (栈顶->下, 与 CPU 异常压栈序一致): [RIP][CS][RFLAGS][RSP][SS]
        frame.add(0).write(entry);
        frame.add(1).write(0x23);
        frame.add(2).write(0x202);
        frame.add(3).write(user_stack);
        frame.add(4).write(0x1B);
        for k in 0..9usize {
            frame.sub(9 - k).write(0); // 寄存器槽 (r11..rax), 初始零
        }
        TASKS[idx] = Task {
            saved_rsp: frame as u64 - 72,
            kstack_top,
            state: TASK_RUNNABLE,
            sig_handler: 0,
            sig_pending: false,
            sig_active: false,
        };
        TASK_COUNT += 1;
        idx
    }
}

/// M13 挂接 (enter_user_test 装载模块后调用, 仅 multi 模式):
/// 任务 A = 模块 (用户栈 0x600000, 内核栈 0x2C0000),
/// 任务 B = 同一镜像 (用户栈 0x640000, 内核栈 0x280000)。
pub fn spawn_tasks(entry: u64) {
    unsafe {
        if TASK_COUNT != 0 {
            return;
        }
        spawn(0x2C0000, 0x5FFFF8, entry);
        spawn(0x280000, 0x63FFF8, entry);
        CUR = 0; // A 先跑 (enter_user_test iretq 进入)
        gdt::set_rsp0(0x2C0000);
        serial::write_line("sched: 2 tasks (A user=0x600000 / B user=0x640000) - timeslice armed");
    }
}

/// PIT tick 钩子 (asm 桩以 (vec=0, regs=帧) 调用; 帧布局见 interruptions.rs 桩注释)。
/// 返回 1 = 已切换 (桩将 rsp 换成 sched_next_rsp), 0 = 继续当前任务。
#[no_mangle]
pub extern "C" fn fujo_tick_sched(_vec: u64, regs: *const u64) -> i64 {
    unsafe {
        // 中断帧 (9 寄存器之后, 栈顶->下): [RIP][CS][RFLAGS][RSP_user][SS]
        // —— CPU 先压 SS/RSP/RFLAGS/CS/RIP, RIP 在栈顶 (M13 现场确证 +10=CS)。
        let cs = regs.add(10).read() as u16;
        if cs != 0x23 {
            return 0; // 内核态 (0x08): 不切换不投递
        }
        // M18: 用户态中断点检查信号 (仅当前任务)
        let delivered = maybe_deliver_signal(regs as *mut u64);
        if TASK_COUNT < 2 {
            return 0; // 单任务: 无切换 (信号已投递则帧已改写返回)
        }
        let _ = delivered;
        TASKS[CUR].saved_rsp = regs as u64;
        let mut next = (CUR + 1) % TASK_COUNT;
        let mut guard = 0usize;
        while TASKS[next].state != TASK_RUNNABLE && guard < TASK_COUNT {
            next = (next + 1) % TASK_COUNT;
            guard += 1;
        }
        if next != CUR && TASKS[next].state == TASK_RUNNABLE {
            CUR = next;
            sched_next_rsp = TASKS[next].saved_rsp;
            gdt::set_rsp0(TASKS[next].kstack_top);
            SWITCHES += 1;
            if SWITCHES <= 8 || SWITCHES % 1000 == 0 {
                serial::write_str("sched: ctx-switch #");
                print_dec(SWITCHES);
                serial::write_str(" -> task ");
                print_dec(next as u64);
                serial::write_line("");
            }
            return 1;
        }
        0
    }
}
