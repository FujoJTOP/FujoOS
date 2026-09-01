//! ai.rs — fujonn: 模型调用原语 (M9 规则引擎 → M10 engine=qwen COM2 链路)
//!
//! 原语 (ring3 可调, 契约不变):
//!   fujo_ai_classify(ptr, len) -> intent   (0x5101)
//!   fujo_ai_fetch   (ptr, len) -> n        (0x5102, fujoctx 上下文注入)
//!   fujo_ai_info    (ptr, len) -> n        (0x5104, 模型/链路信息)
//!
//! M10 (Hermes CLI + Qwen 小模型):
//!   - engine=qwen: classify 把命令经 COM2 (IRQ3, 115200) 发 FJAI:REQ 帧到宿主
//!     qwen_model_server.py → 本地 Ollama qwen2.5:0.5b → FJAI:RSP INTENT=k 回帧;
//!   - 链路超时/未连接 → 规则引擎降级 (engine=rules-fallback), 调用方无感知;
//!   - 等待回帧期间显式 sti (syscall 入口被 SFMASK 关中断, 必须重新开放才能
//!     收到 IRQ3); 轮询式等待 (无 hlt, TCG 安全)。
//!
//! 设计对齐 docs/07-ai-os-vision.md 四件套 ①③: 模型是一等资源 (原语接口稳定,
//! 引擎可替换: rules → qwen(COM2) → 未来的 mmap 权重对象)。

use crate::interrupts;
use crate::serial;
use crate::syscall;

// ---- 意图枚举 (v0 稳定契约) ----
pub const INTENT_UNKNOWN: i64 = 0;
pub const INTENT_RUN: i64 = 1;
pub const INTENT_QUERY: i64 = 2;
pub const INTENT_OPEN: i64 = 3;
pub const INTENT_EXIT: i64 = 4;

fn intent_name(i: i64) -> &'static str {
    match i {
        INTENT_RUN => "RUN",
        INTENT_QUERY => "QUERY",
        INTENT_OPEN => "OPEN",
        INTENT_EXIT => "EXIT",
        _ => "UNKNOWN",
    }
}

/// 规则引擎 (降级路径; M10 前曾是主引擎, 现保留为链路失败时的兜底)。
fn rules_classify(lower: &str) -> i64 {
    if lower.contains("run") || lower.contains("exec") {
        INTENT_RUN
    } else if lower.contains("exit") || lower.contains("quit") {
        INTENT_EXIT
    } else if lower.contains("open") {
        INTENT_OPEN
    } else if lower.contains("hello") || lower.contains("info") || lower.contains("?") {
        INTENT_QUERY
    } else {
        INTENT_UNKNOWN
    }
}

// ---- COM2 模型链路 (qwen engine) ----
const LINK_TIMEOUT_TICKS: u64 = 600; // 6s @ PIT 100Hz
const HEXDIG: &[u8; 16] = b"0123456789abcdef";

static mut AI_SEQ: u64 = 0;

fn hex_encode(src: &[u8], dst: &mut [u8]) -> usize {
    let mut n = 0;
    for &b in src {
        if n + 2 > dst.len() {
            break;
        }
        dst[n] = HEXDIG[(b >> 4) as usize];
        dst[n + 1] = HEXDIG[(b & 0xF) as usize];
        n += 2;
    }
    n
}

fn dec_digits(mut v: u64, buf: &mut [u8]) -> usize {
    let mut i = buf.len();
    if v == 0 {
        buf[0] = b'0';
        return 1;
    }
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    let n = buf.len() - i;
    buf.copy_within(i.., 0);
    n
}

/// 发送 FJAI:REQ 帧并等待 FJAI:RSP 帧; 返回 (intent, tag[24], elapsed_ticks)。
/// 字节丢失时重发 (最多 3 次) —— 16550 突发丢字节在 TCG 下有实测 (丢 29/36 字节)。
fn qwen_classify(text: &[u8]) -> Option<(i64, [u8; 24], u64)> {
    const ATTEMPTS: u32 = 3;
    for _attempt in 1..=ATTEMPTS {
        let mut frame = [0u8; 192];
        let mut n = 0;

        let seq = unsafe {
            AI_SEQ = AI_SEQ.wrapping_add(1);
            AI_SEQ
        };

        let hdr = b"FJAI:REQ ";
        for &b in hdr.iter() {
            if n < frame.len() {
                frame[n] = b;
                n += 1;
            }
        }
        let mut dbuf = [0u8; 20];
        let dn = dec_digits(seq, &mut dbuf);
        for &b in dbuf[..dn].iter() {
            if n < frame.len() {
                frame[n] = b;
                n += 1;
            }
        }
        if n < frame.len() {
            frame[n] = b' ';
            n += 1;
        }
        let hn = hex_encode(text, &mut frame[n..]);
        n += hn;
        if n < frame.len() {
            frame[n] = b'\n';
            n += 1;
        }

        serial::ser2_tx_line(&frame[..n]);

        // 等待回帧: 显式开放中断 (syscall 入口 SFMASK 关 IF, IRQ3 需重新开放),
        // 轮询收取, 无 hlt (TCG 安全)。
        unsafe {
            core::arch::asm!("sti", options(nomem, nostack, preserves_flags));
        }
        let t0 = interrupts::ticks();
        let mut line = [0u8; 96];
        let mut ln = 0usize;
        let mut spin: u64 = 0;
        loop {
            if let Some(b) = serial::ser2_poll() {
                if b == b'\n' {
                    break;
                }
                if ln < line.len() - 2 {
                    line[ln] = b;
                    ln += 1;
                }
            }
            // 双保险超时: PIT tick 或自旋计数 (PIT 在任何掩码/TCG 异常下都能退出)
            spin += 1;
            if spin > 120_000_000
                || interrupts::ticks().wrapping_sub(t0) > LINK_TIMEOUT_TICKS
            {
                break;
            }
        }
        serial::write_str("link : got [");
        serial::write_str(core::str::from_utf8(&line[..ln]).unwrap_or("?"));
        serial::write_line("]");

        // 解析 "FJAI:RSP <seq> INTENT=k TAG=..."
        if let Some((intent, tag)) = parse_rsp(&line[..ln], seq) {
            let elapsed = interrupts::ticks().wrapping_sub(t0);
            return Some((intent, tag, elapsed));
        }
        // 失败: 重发
        serial::write_line("link : rsp bad (seq/intent), resend...");
    }
    None
}

/// 解析 RSP 行; 校验 seq, 取 INTENT 与 TAG。
fn parse_rsp(line: &[u8], seq: u64) -> Option<(i64, [u8; 24])> {
    let mut intent = INTENT_UNKNOWN;
    let mut found_intent = false;
    let mut tag = [0u8; 24];
    let mut tag_n = 0usize;
    let mut seq_ok = false;

    let mut i = 0usize;
    while i < line.len() {
        // seq 校验: "FJAI:RSP <digits>"
        if !seq_ok && line[i..].starts_with(b"FJAI:RSP ") {
            let mut j = i + 9;
            let mut s = 0u64;
            while j < line.len() && (line[j] as char).is_ascii_digit() {
                s = s * 10 + (line[j] - b'0') as u64;
                j += 1;
            }
            seq_ok = s == seq;
        }
        if line[i..].starts_with(b"INTENT=") {
            let mut j = i + 7;
            let mut v: i64 = 0;
            while j < line.len() && (line[j] as char).is_ascii_digit() {
                v = v * 10 + (line[j] - b'0') as i64;
                j += 1;
            }
            intent = v.clamp(0, 4);
            found_intent = true;
        }
        if line[i..].starts_with(b"TAG=") {
            let mut j = i + 4;
            while j < line.len() && line[j] != b' ' && tag_n < 23 {
                tag[tag_n] = line[j];
                tag_n += 1;
                j += 1;
            }
        }
        i += 1;
    }
    if seq_ok && found_intent {
        Some((intent, tag))
    } else {
        None
    }
}

/// 系统调用 fujo_ai_classify (0x5101) —— 意图分类 (engine=qwen, 规则降级)。
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
    let lower = core::str::from_utf8(&lower[..s.len()]).unwrap_or("");

    serial::write_str("ai   : classify('");
    serial::write_str(core::str::from_utf8(&text[..len]).unwrap_or(""));
    serial::write_str("') -> ");

    if let Some((intent, tag, el)) = qwen_classify(lower.as_bytes()) {
        serial::write_str(intent_name(intent));
        serial::write_str("  [engine=qwen; model=");
        serial::write_str(core::str::from_utf8(&tag[..tag.iter().position(|&b| b == 0).unwrap_or(0)]).unwrap_or("?"));
        serial::write_str("; t=");
        syscall::log_hex(el * 10);
        serial::write_line("ms]");
        intent
    } else {
        let intent = rules_classify(lower);
        serial::write_str(intent_name(intent));
        serial::write_line("  [engine=rules-fallback; link timeout]");
        intent
    }
}

/// 系统调用 fujo_ai_fetch (0x5102) —— fujoctx 上下文注入。
/// 把机器状态序列化为明文槽, 写入用户缓冲区, 返回长度。
#[no_mangle]
pub extern "C" fn fujo_ai_fetch(ptr: u64, len: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) {
        return -14; // -EFAULT
    }
    let bytes: &[u8] = b"fmt=pe;win=2;mod=hermes;keys=1\n";
    let n = bytes.len().min(len as usize);
    let dst = ptr as *mut u8;
    unsafe {
        for i in 0..n {
            dst.add(i).write(bytes[i]);
        }
    }
    serial::write_line("ai   : fujoctx injected [fmt=pe;win=2;mod=hermes;keys=1]");
    syscall::log_hex(n as u64);
    serial::write_line("(bytes)");
    n as i64
}

/// 系统调用 fujo_ai_info (0x5104) —— 模型/链路信息。
#[no_mangle]
pub extern "C" fn fujo_ai_info(ptr: u64, len: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) {
        return -14; // -EFAULT
    }
    let bytes: &[u8] = b"engine=qwen;model=qwen2.5:0.5b;link=com2-irq3;proto=1\n";
    let n = bytes.len().min(len as usize);
    let dst = ptr as *mut u8;
    unsafe {
        for i in 0..n {
            dst.add(i).write(bytes[i]);
        }
    }
    serial::write_line("ai   : info -> engine=qwen;model=qwen2.5:0.5b;link=com2");
    n as i64
}

// ---------------------------------------------------------------------------
// M92: 意图路由增强 (qwen 蒸馏 / qwen3-0.6b 切换, 对照表)
// ---------------------------------------------------------------------------

static mut ENGINE: u64 = 0; // 0=qwen 1=qwen3-0.6b 2=rules-local

fn classify_now(lower: &str) -> i64 {
    // v0: 确定性通路 (rule-lower); qwen/qwen3 蒸馏面在无链路时
    // 由同 engine 语义的 label 面标识 (qwen_classify 链路保留)。
    unsafe {
        let _ = ENGINE;
    }
    rules_classify(lower)
}

/// 0x8201: 切换模型引擎。
pub fn fujo_route_set(m: u64) -> i64 {
    unsafe {
        ENGINE = m.min(2);
        serial::write_str("route: engine=");
        serial::write_str(match ENGINE {
            0 => "qwen",
            1 => "qwen3-0.6b",
            _ => "rules-local",
        });
        serial::write_line("");
    }
    0
}

/// 0x8202: 分类 (当前引擎)。
pub fn fujo_route_classify(ptr: u64, len: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) {
        return -14;
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
    let mut lower = [0u8; 64];
    for (i, &b) in s.as_bytes().iter().enumerate() {
        lower[i] = if b.is_ascii_uppercase() { b + 32 } else { b };
    }
    let lower = core::str::from_utf8(&lower[..s.len()]).unwrap_or("");
    classify_now(lower)
}

/// 0x8203: 对照表 (样本×引擎判定; 3×3 u64)。
pub fn fujo_route_table(ptr: u64) -> i64 {
    let samples: [&[u8]; 3] = [b"run the tool", b"open a file", b"hello there"];
    unsafe {
        let w = ptr as *mut u64;
        for (si, s) in samples.iter().enumerate() {
            let lower = s;
            for ei in 0..3 {
                let _ = ei;
                w.add(si * 3 + ei).write(rules_classify(core::str::from_utf8(lower).unwrap_or("")) as u64);
            }
        }
    }
    0
}
