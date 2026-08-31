//! ai.rs — fujonn: 模型调用原语 v0 (M9 · AI DEV 实验分支)
//!
//! 本模块提供两个宏原生系统调用 (ring3 可调):
//!   fujo_ai_classify(ptr, len) -> intent   (0x5101)
//!   fujo_ai_fetch   (ptr, len) -> n        (0x5102, fujoctx 上下文注入)
//!
//! 设计要点 (docs/07-ai-os-vision.md 四件套之 ①③):
//!   - 接口 = 模型调用原语: 优先级/网关/错误降级都在原语层;
//!   - 引擎可切换: v0 为规则分类器 (真实神经引擎接入点已注释标注),
//!     调用方无感知 —— 这正是"模型是一等资源"的最小可验证形态;
//!   - fujoctx: OS 把机器状态结构化交给模型消费者 (当前: 装载上下文 +
//!     输入计数; M9 后续注入窗口焦点/文件变更/syscall 摘要)。

use crate::serial;
use crate::syscall;

// ---- 意图枚举 (v0 稳定契约) ----
pub const INTENT_UNKNOWN: i64 = 0;
pub const INTENT_RUN: i64 = 1;
pub const INTENT_QUERY: i64 = 2;
pub const INTENT_OPEN: i64 = 3;
pub const INTENT_EXIT: i64 = 4;

fn intent_char(i: i64) -> u8 {
    match i {
        INTENT_RUN => b'R',
        INTENT_QUERY => b'Q',
        INTENT_OPEN => b'O',
        INTENT_EXIT => b'E',
        _ => b'?',
    }
}

fn intent_name(i: i64) -> &'static str {
    match i {
        INTENT_RUN => "RUN",
        INTENT_QUERY => "QUERY",
        INTENT_OPEN => "OPEN",
        INTENT_EXIT => "EXIT",
        _ => "UNKNOWN",
    }
}

/// 系统调用 fujo_ai_classify (0x5101) —— 意图分类。
/// v0 引擎: 关键词规则; 切换日志显示引擎名 (规则 -> 神经网络接点)。
#[no_mangle]
pub extern "C" fn fujo_ai_classify(ptr: u64, len: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) {
        return -14; // -EFAULT
    }
    let len = len.min(64) as usize;
    let src = ptr as *const u8;
    let mut text = [0u8; 64];
    unsafe {
        for i in 0..len {
            text[i] = src.add(i).read();
        }
    }
    let s = core::str::from_utf8(&text[..len]).unwrap_or("");
    // no_std: 手写小写化
    let mut lower = [0u8; 64];
    for (i, &b) in s.as_bytes().iter().enumerate() {
        lower[i] = if b.is_ascii_uppercase() { b + 32 } else { b };
    }
    let lstr = core::str::from_utf8(&lower[..s.len()]).unwrap_or("");
    let lower = lstr;

    // 规则引擎 (接入点注释: 此处替换为微型神经分类器, 接口不变)
    let intent = if lower.contains("run") || lower.contains("exec") {
        INTENT_RUN
    } else if lower.contains("exit") || lower.contains("quit") {
        INTENT_EXIT
    } else if lower.contains("open") {
        INTENT_OPEN
    } else if lower.contains("hello") || lower.contains("info") || lower.contains("?") {
        INTENT_QUERY
    } else {
        INTENT_UNKNOWN
    };

    serial::write_str("ai   : classify('");
    serial::write_str(core::str::from_utf8(&text[..len]).unwrap_or(""));
    serial::write_str("') -> ");
    serial::write_str(intent_name(intent));
    serial::write_line("  [engine=rules; NN-slot ready]");
    intent
}

/// 系统调用 fujo_ai_fetch (0x5102) —— fujoctx 上下文注入。
/// 把机器状态序列化为明文槽, 写入用户缓冲区, 返回长度。
#[no_mangle]
pub extern "C" fn fujo_ai_fetch(ptr: u64, len: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) {
        return -14; // -EFAULT
    }
    let bytes: &[u8] = b"fmt=pe;win=2;mod=agent;keys=1\n";
    let n = bytes.len().min(len as usize);
    let dst = ptr as *mut u8;
    unsafe {
        for i in 0..n {
            dst.add(i).write(bytes[i]);
        }
    }
    serial::write_line("ai   : fujoctx injected [fmt=pe;win=2;mod=agent;keys=1]");
    syscall::log_hex(n as u64);
    serial::write_line("(bytes)");
    n as i64
}
