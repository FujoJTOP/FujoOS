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

#[derive(Clone, Copy)]
pub struct Task {
    pub saved_rsp: u64,
    pub kstack_top: u64,
    pub state: u8,
}

static mut TASKS: [Task; MAX_TASKS] = [
    Task { saved_rsp: 0, kstack_top: 0, state: 0 };
    MAX_TASKS
];
static mut TASK_COUNT: usize = 0;
static mut CUR: usize = 0;
static mut SWITCHES: u64 = 0;
static mut MULTI: bool = false;

/// 切换桩引用的全局: 下一任务的内核栈保存帧指针 (桩 mov rsp, [rip + ...])。
#[no_mangle]
pub static mut sched_next_rsp: u64 = 0;

/// shell `os run threads` 打开双任务模式。
pub fn set_multi() {
    unsafe {
        MULTI = true;
    }
}

pub fn multi_task() -> bool {
    unsafe { MULTI }
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
        spawn(0x2C0000, 0x600000, entry);
        spawn(0x280000, 0x640000, entry);
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
        if cs != 0x23 || TASK_COUNT < 2 {
            return 0; // 内核态 (0x08) 或未启用多任务: 不切换
        }
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
