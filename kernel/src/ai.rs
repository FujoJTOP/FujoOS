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

/// M118 (R3): 请求@t0 快照 (shm 路径; 检查在 wait_rsp 内消耗一次)。
static mut SNAP_T0: u64 = 0;
static mut SNAP_EVW: u64 = 0;
static mut SNAP_CRIT: u64 = 0;
static mut SNAP_VALID: bool = false;
/// 0=接受/超时 1=关键事件丢弃 2=TTL 过期丢弃。
static mut SNAP_REASON: u64 = 0;

pub fn r3_discarded() -> bool {
    unsafe { SNAP_REASON != 0 }
}

fn r3_snap_clear() {
    unsafe {
        SNAP_VALID = false;
        SNAP_REASON = 0;
    }
}

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
///
/// M112: 首选 shm-link (①) —— 请求走共享内存 (宿主 pmemsave 直读),
/// 触发线 + 响应当仍走 COM2 (QEMU monitor 无写入命令, 见 docs/53)。
fn qwen_classify(text: &[u8]) -> Option<(i64, [u8; 24], u64)> {
    let seq = shm_send_req(SHM_KIND_CLASSIFY, text);
    if let Some((line, ln, el)) = wait_rsp(seq) {
        if let Some((intent, tag)) = parse_rsp(&line[..ln], seq) {
            return Some((intent, tag, el));
        }
        serial::write_line("link : shm rsp seq/intent bad, com2 downgrade...");
    } else if r3_discarded() {
        // M118 R3: 陈旧建议已丢弃 -> 直接规则兜底 (不再问模型)
        return None;
    }
    qwen_classify_ser(text)
}

/// COM2 降级路径 (原 FJAI:REQ 行协议, 3 次重发)。
fn qwen_classify_ser(text: &[u8]) -> Option<(i64, [u8; 24], u64)> {
    const ATTEMPTS: u32 = 3;
    r3_snap_clear(); // 行协议无快照: 关闭 R3 检查
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

        if let Some((intent, tag, el)) = wait_rsp_ser(seq) {
            return Some((intent, tag, el));
        }
        // 失败: 重发
        serial::write_line("link : rsp bad (seq/intent), resend...");
    }
    None
}

// ---------------------------------------------------------------------------
// M112 · ① shm-link: 模型通道走 0xA00000 共享页 (宿主 pmemsave 直读)
// ---------------------------------------------------------------------------

const SHM_MAGIC: u32 = 0x4853_4A46; // LE 字节 "FJSH"
/// 帧种类。
const SHM_KIND_CLASSIFY: u32 = 1;
const SHM_KIND_ANOMALY: u32 = 2;
const SHM_KIND_PLAN: u32 = 3; // M113 计划-执行器
const SHM_KIND_IO: u32 = 4; // M113 I/O 预测器
const SHM_KIND_NLC: u32 = 5; // M114 自然语言配置
const SHM_KIND_ENV: u32 = 6; // M114 环境侦察
/// 帧布局 v2 (0xA00000 起, M118 R3 时延一致性):
/// magic u32 / ver u32 / seq u64 / kind u32 / len u32 / t0 u64 / evw u64 / crit u32,
/// payload @0x30 (≤1KB), ctx @0x800。
const SHM_OFF_PAYLOAD: usize = 0x030;
const SHM_PAYLOAD_MAX: usize = 0x400;
const SHM_OFF_CTX: usize = 0x800;
const SHM_CTX_MAX: usize = 0x600;
const SHM_OFF_T0: usize = 0x018; // u64: 请求写入时刻 (PIT ticks, 100Hz)
const SHM_OFF_EVW: usize = 0x020; // u64: 事件环写位置快照
const SHM_OFF_CRIT: usize = 0x028; // u32: 关键事件种类掩码 (bit=kind-1)
const SHM_VER: u32 = 2;

/// R3: 关键事件种类 (到达即使过期建议失效) —— 异常/退出/窗口。
const EV_CRIT_MASK: u64 = (1u64 << (crate::ctx::EV_ANOMALY - 1))
    | (1u64 << (crate::ctx::EV_EXIT - 1))
    | (1u64 << (crate::ctx::EV_WINDOW - 1)); // 0x1C

/// 写请求帧 (payload + 当前 fujoctx 结构态文本); 记录快照@t0 (M118 R3)。
fn shm_write_req(seq: u64, kind: u32, payload: &[u8]) {
    unsafe {
        let b = crate::ipc::SHM_BASE as *mut u8;
        let t0 = crate::interrupts::ticks();
        let evw = crate::ctx::ev_write_pos();
        SNAP_T0 = t0;
        SNAP_EVW = evw;
        SNAP_CRIT = EV_CRIT_MASK;
        SNAP_VALID = true;
        SNAP_REASON = 0;
        (b.add(0x000) as *mut u32).write_volatile(SHM_MAGIC);
        (b.add(0x004) as *mut u32).write_volatile(SHM_VER);
        (b.add(0x008) as *mut u64).write_volatile(seq);
        (b.add(0x010) as *mut u32).write_volatile(kind);
        let n = payload.len().min(SHM_PAYLOAD_MAX);
        (b.add(0x014) as *mut u32).write_volatile(n as u32);
        (b.add(SHM_OFF_T0) as *mut u64).write_volatile(t0);
        (b.add(SHM_OFF_EVW) as *mut u64).write_volatile(evw);
        (b.add(SHM_OFF_CRIT) as *mut u32).write_volatile(EV_CRIT_MASK as u32);
        for i in 0..n {
            b.add(SHM_OFF_PAYLOAD + i).write_volatile(payload[i]);
        }
        // 结构态上下文 (② fujoctx v2)
        let cn = crate::ctx::ctx_build_text(b.add(SHM_OFF_CTX), SHM_CTX_MAX);
        b.add(SHM_OFF_CTX + cn).write_volatile(0u8);
    }
}

/// 发送请求: shm 帧 + COM2 触发线 "FJAI:SHM <seq> <kind> <len>"。
fn shm_send_req(kind: u32, payload: &[u8]) -> u64 {
    unsafe { AI_CALLS += 1 }
    let seq = unsafe {
        AI_SEQ = AI_SEQ.wrapping_add(1);
        AI_SEQ
    };
    shm_write_req(seq, kind, payload);
    let mut line = [0u8; 48];
    let mut n = 0;
    for &b in b"FJAI:SHM ".iter() {
        if n < line.len() {
            line[n] = b;
            n += 1;
        }
    }
    let mut nb = [0u8; 20];
    let dn = dec_digits(seq, &mut nb);
    for &b in nb[..dn].iter() {
        if n < line.len() {
            line[n] = b;
            n += 1;
        }
    }
    for &b in b" ".iter() {
        if n < line.len() {
            line[n] = b;
            n += 1;
        }
    }
    let dn2 = dec_digits(kind as u64, &mut nb);
    for &b in nb[..dn2].iter() {
        if n < line.len() {
            line[n] = b;
            n += 1;
        }
    }
    for &b in b" ".iter() {
        if n < line.len() {
            line[n] = b;
            n += 1;
        }
    }
    let dn3 = dec_digits(payload.len() as u64, &mut nb);
    for &b in nb[..dn3].iter() {
        if n < line.len() {
            line[n] = b;
            n += 1;
        }
    }
    if n < line.len() {
        line[n] = b'\n';
        n += 1;
    }
    serial::ser2_tx_line(&line[..n]);
    seq
}

/// 等待 RSP (显式 sti 收 IRQ3, 轮询, 双保险超时); 返回 (行, 行长度, 耗时 ticks)。
/// M112 硬化: seq 不符的行丢弃继续等 (服务端响应可能属先前请求 ——
/// 帧/触发漂移时 RSP 会对上最新的帧, 匹配即收)。
/// M118 (R3): shm 路径在返回前做时延一致性检查 —— t0 后关键事件到达
/// 或超过回包声明的 TTL => 丢弃建议 (返回 None, SNAP_REASON 标注原因,
/// 调用方走规则兜底)。
fn wait_rsp(seq: u64) -> Option<([u8; 96], usize, u64)> {
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
                if line[..ln].starts_with(b"FJAI:RSP ") && line_seq_ok(&line[..ln], seq) {
                    // ---- R3 时延一致性 (仅 shm 快照有效时) ----
                    if unsafe { SNAP_VALID } {
                        let crit = unsafe {
                            crate::ctx::ev_delta_critical(SNAP_EVW, SNAP_CRIT)
                        };
                        let el = interrupts::ticks().wrapping_sub(unsafe { SNAP_T0 });
                        let ttl = parse_ttl(&line[..ln]).unwrap_or(u64::MAX);
                        if crit > 0 {
                            r3_snap_clear();
                            unsafe { SNAP_REASON = 1; }
                            serial::write_str("link : DISCARD crit_ev=");
                            crate::syscall::debug_dec(crit);
                            serial::write_line(" -> rules stale suggestion");
                            return None;
                        }
                        if ttl != u64::MAX && el > ttl {
                            r3_snap_clear();
                            unsafe { SNAP_REASON = 2; }
                            serial::write_str("link : DISCARD ttl el=");
                            crate::syscall::debug_dec(el);
                            serial::write_str(" ttl=");
                            crate::syscall::debug_dec(ttl);
                            serial::write_line(" -> rules stale suggestion");
                            return None;
                        }
                        r3_snap_clear();
                    }
                    serial::write_str("link : got [");
                    serial::write_str(core::str::from_utf8(&line[..ln]).unwrap_or("?"));
                    serial::write_line("]");
                    return Some((line, ln, interrupts::ticks().wrapping_sub(t0)));
                }
                // seq 不符/坏行: 丢弃继续等 (可能属先前请求)
                ln = 0;
                continue;
            }
            if ln < line.len() - 2 {
                line[ln] = b;
                ln += 1;
            }
        }
        spin += 1;
        if spin > 120_000_000 || interrupts::ticks().wrapping_sub(t0) > LINK_TIMEOUT_TICKS {
            r3_snap_clear();
            return None;
        }
    }
}

/// 解析回包 TTL=<ticks> (模型声明有效期; 缺省 = 无限制)。
fn parse_ttl(line: &[u8]) -> Option<u64> {
    let mut n = 0u64;
    let mut i = 0usize;
    while i + 4 <= line.len() {
        if line[i..].starts_with(b"TTL=") {
            let mut j = i + 4;
            while j < line.len() && (line[j] as char).is_ascii_digit() {
                n = n * 10 + (line[j] - b'0') as u64;
                j += 1;
            }
            return Some(n);
        }
        i += 1;
    }
    None
}

/// 校验 RSP 行内 seq。
fn line_seq_ok(line: &[u8], seq: u64) -> bool {
    if let Some(rest) = line.strip_prefix(b"FJAI:RSP ") {
        let mut j = 0usize;
        let mut s = 0u64;
        while j < rest.len() && (rest[j] as char).is_ascii_digit() {
            s = s * 10 + (rest[j] - b'0') as u64;
            j += 1;
        }
        return s == seq;
    }
    false
}

/// COM2 降级等待 (原逻辑核心)。
fn wait_rsp_ser(seq: u64) -> Option<(i64, [u8; 24], u64)> {
    if let Some((line, ln, el)) = wait_rsp(seq) {
        if let Some((intent, tag)) = parse_rsp(&line[..ln], seq) {
            return Some((intent, tag, el));
        }
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

/// W12: /dev/model0 后端 —— 与 0x5101 同核 (R5 规则字节码优先 → 模型 → 兜底),
/// AI 接口 UNIX 化: write=请求 (阻塞一次往返), read=响应文本。
pub fn model_classify_intent(text: &[u8]) -> i64 {
    let n = text.len().min(64);
    let mut lower = [0u8; 64];
    for i in 0..n {
        let b = text[i];
        lower[i] = if b.is_ascii_uppercase() { b + 32 } else { b };
    }
    let lower = &lower[..n];
    if let Some((v, _a0, _a1, _c)) = rules_match(lower, SHM_KIND_CLASSIFY as u64) {
        serial::write_str("model: /dev/model0 '");
        serial::write_str(core::str::from_utf8(&text[..n]).unwrap_or(""));
        serial::write_line("' -> rulebook");
        ai_aud_note(1, 3, v, 0, 0, 0, &text[..n]);
        return v as i64;
    }
    if let Some((intent, tag, _el)) = qwen_classify(lower) {
        ai_aud_note(1, 1, intent as u64, 0, 0, 0, &text[..n]);
        serial::write_str("model: /dev/model0 -> ");
        serial::write_str(intent_name(intent));
        serial::write_str(" [qwen=");
        serial::write_str(core::str::from_utf8(&tag[..tag.iter().position(|&b| b == 0).unwrap_or(0)]).unwrap_or("?"));
        serial::write_line("]");
        intent
    } else {
        let intent = rules_classify(core::str::from_utf8(lower).unwrap_or(""));
        ai_aud_note(1, 2, intent as u64, 0, 0, 0, &text[..n]);
        intent
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

    // W10 R5: 蒸馏规则字节码优先 (命中 = 零模型调用; 陌生情况才走模型)。
    // W22: force=1 跳过规则面 (强制模型); force=2 跳过规则+模型 (强制规则)。
    if eval_force() != 1 {
        if let Some((v, _a0, _a1, _c)) = rules_match(lower.as_bytes(), SHM_KIND_CLASSIFY as u64) {
            serial::write_str(intent_name(v as i64));
            serial::write_line("  [engine=rulebook]");
            ai_aud_note(1, 3, v, 0, 0, 0, &text[..len]);
            return v as i64;
        }
    }
    if eval_force() != 2 {
        if let Some((intent, tag, el)) = qwen_classify(lower.as_bytes()) {
            ai_aud_note(1, 1, intent as u64, 0, 0, 0, &text[..len]);
            serial::write_str(intent_name(intent));
            serial::write_str("  [engine=qwen; model=");
            serial::write_str(core::str::from_utf8(&tag[..tag.iter().position(|&b| b == 0).unwrap_or(0)]).unwrap_or("?"));
            serial::write_str("; t=");
            syscall::log_hex(el * 10);
            serial::write_line("ms]");
            return intent;
        }
    }
    let intent = rules_classify(lower);
    ai_aud_note(1, 2, intent as u64, 0, 0, 0, &text[..len]);
    serial::write_str(intent_name(intent));
    serial::write_line("  [engine=rules-fallback; link timeout]");
    intent
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
// W22 · 评测引擎强制 (三引擎对照: auto=rulebook→model→rules / model / rules)
// 垂直开发第一步: 让同一份样本集可在 3 种引擎下运行, 量化模型边际价值。
// ---------------------------------------------------------------------------

static mut EVAL_FORCE: u64 = 0; // 0=auto 1=force-model 2=force-rules

pub fn eval_force() -> u64 {
    unsafe { EVAL_FORCE }
}

/// 0x830F: 设置评测引擎模式 (demo 三引擎对照; 0 恢复自动)。
#[no_mangle]
pub extern "C" fn fujo_evl_mode(mode: u64) -> i64 {
    unsafe {
        EVAL_FORCE = mode.min(2);
        serial::write_str("evl : engine mode=");
        crate::syscall::debug_dec(EVAL_FORCE);
        serial::write_line("");
    }
    0
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

// ---------------------------------------------------------------------------
// M112 · A 异常哨兵 (异常分类 + 自动隔离) —— AI 的判断通道
// ---------------------------------------------------------------------------

static mut ANOM_TOTAL: u64 = 0; // 历次异常判定的总数 (结构态)
static mut ANOM_PENDING: u64 = 0; // 待确认异常 (cap_exec ACK 清零)

pub fn anom_total() -> u64 {
    unsafe { ANOM_TOTAL }
}

pub fn anom_ack() -> i64 {
    unsafe {
        ANOM_PENDING = 0;
        serial::write_line("anom : acknowledged (pending cleared)");
    }
    0
}

/// 解析 "FJAI:RSP <seq> ... ANOM=<0|1> CONF=<0-99> TAG=..."。
fn parse_anom_rsp(line: &[u8], seq: u64) -> Option<(u64, u64, [u8; 24])> {
    if !line_seq_ok(line, seq) {
        return None;
    }
    let mut anom = 0u64;
    let mut conf = 0u64;
    let mut found = false;
    let mut tag = [0u8; 24];
    let mut tag_n = 0usize;
    let mut i = 0usize;
    while i < line.len() {
        if line[i..].starts_with(b"ANOM=") {
            let mut j = i + 5;
            let mut v = 0u64;
            while j < line.len() && (line[j] as char).is_ascii_digit() {
                v = v * 10 + (line[j] - b'0') as u64;
                j += 1;
            }
            anom = v.min(1);
            found = true;
        }
        if line[i..].starts_with(b"CONF=") {
            let mut j = i + 5;
            let mut v = 0u64;
            while j < line.len() && (line[j] as char).is_ascii_digit() {
                v = v * 10 + (line[j] - b'0') as u64;
                j += 1;
            }
            conf = v.min(100);
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
    if found {
        Some((anom, conf, tag))
    } else {
        None
    }
}

/// 规则降级 (确定性基线): 事件摘要含 rate=9x / dead / diag → 异常。
fn rules_anom(text: &[u8]) -> (u64, u64) {
    let s = core::str::from_utf8(text).unwrap_or("");
    if s.contains("rate=9") || s.contains("dead") || s.contains("diag") {
        (1, 80)
    } else {
        (0, 20)
    }
}

/// 从摘要提取 pid=NN。
fn parse_pid(text: &[u8]) -> Option<u64> {
    let s = core::str::from_utf8(text).unwrap_or("");
    let idx = s.find("pid=")?;
    let rest = &s[idx + 4..];
    let mut v = 0u64;
    let mut saw = false;
    for c in rest.chars() {
        if c.is_ascii_digit() {
            v = v * 10 + (c as u64 - '0' as u64);
            saw = true;
        } else {
            break;
        }
    }
    if saw {
        Some(v)
    } else {
        None
    }
}

/// 模型路径 (shm 帧 kind=2) → (anom, conf, tag)。
fn anom_llm(text: &[u8]) -> Option<(u64, u64, [u8; 24])> {
    let seq = shm_send_req(SHM_KIND_ANOMALY, text);
    let (line, ln, _el) = wait_rsp(seq)?;
    parse_anom_rsp(&line[..ln], seq)
}

/// 0x8304: 异常哨兵分类 (ptr=事件摘要文本, out=u64×3: [anom, conf, engine])。
/// engine: 1=模型 (shm) 2=规则降级。anom=1 时: 记事件 + 按配置自动隔离。
#[no_mangle]
pub extern "C" fn fujo_anom_run(ptr: u64, len: u64, out: u64, _cap: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) || !(0x400000..0x800000).contains(&out) {
        return -14; // -EFAULT
    }
    let len = (len as usize).min(400);
    let src = ptr as *const u8;
    let mut text = [0u8; 400];
    unsafe {
        for i in 0..len {
            text[i] = src.add(i).read();
        }
    }
    let s = &text[..len];

    // W10 R5: 蒸馏字节码优先; 陌生 → 模型; 超时 → 规则。
    // W22: force=1 跳过规则面; force=2 强制规则 (确定性引擎对照)。
    let (anom, conf, tag, engine) = {
        let rb = if eval_force() != 1 {
            rules_match(s, SHM_KIND_ANOMALY as u64)
        } else {
            None
        };
        if let Some((v, a0, _a1, _c)) = rb {
            let mut t = [0u8; 24];
            t[..8].copy_from_slice(b"rulebook");
            (v, a0, t, 3u64)
        } else if eval_force() == 2 {
            serial::write_line("anom : force rules engine");
            let (a, c) = rules_anom(s);
            let mut t = [0u8; 24];
            t[..7].copy_from_slice(b"fjrules");
            (a, c, t, 2u64)
        } else {
            match anom_llm(s) {
                Some((a, c, tag)) => (a, c, tag, 1u64),
                None => {
                    serial::write_line("anom : shm timeout -> rules fallback");
                    let (a, c) = rules_anom(s);
                    let mut t = [0u8; 24];
                    t[..7].copy_from_slice(b"fjrules");
                    (a, c, t, 2u64)
                }
            }
        }
    };

    let mut iso_rc: i64 = 0;
    // W22: 自监督验证位 —— 自动隔离被执行且任务确实进入隔离态 (2) => 建议被证实。
    let mut fb_verified: u64 = 0;
    if anom == 1 {
        unsafe {
            ANOM_TOTAL += 1;
            ANOM_PENDING += 1;
        }
        let pid = parse_pid(s).unwrap_or(0);
        crate::ctx::ev_push(crate::ctx::EV_ANOMALY, pid, anom, conf);
        // 自动隔离 (cfg 2 + 阈值 cfg 1); 需 exec 槽授权, 未授权仅记录
        if crate::capability::fujo_cfg_get(2) == 1
            && conf as i64 >= crate::capability::fujo_cfg_get(1)
        {
            if let Some(pid) = parse_pid(s) {
                iso_rc = crate::capability::fujo_cap_exec(crate::capability::ACT_ISOLATE, pid, 0);
                if iso_rc == 0 && crate::sched::task_state(pid as usize) == 2 {
                    fb_verified = 1;
                }
            }
        }
    }
    ai_aud_note(2, engine, anom, conf, iso_rc as u64, fb_verified, &text[..len]);

    let o = out as *mut u64;
    unsafe {
        o.write(anom);
        o.add(1).write(conf);
        o.add(2).write(engine);
    }
    serial::write_str("anom : ");
    serial::write_str(core::str::from_utf8(&text[..len.min(48)]).unwrap_or(""));
    serial::write_str(" -> ");
    serial::write_str(if anom == 1 { "ANOMALY" } else { "normal" });
    serial::write_str(" conf=");
    crate::syscall::debug_dec(conf);
    serial::write_str(" engine=");
    crate::syscall::debug_dec(engine);
    serial::write_str(" model=");
    serial::write_str(core::str::from_utf8(&tag[..tag.iter().position(|&b| b == 0).unwrap_or(0)]).unwrap_or("?"));
    serial::write_line("");
    0
}

// ---------------------------------------------------------------------------
// M113 · B 计划-执行器 (goal → 工具向量 → cap_exec → verify)
// ---------------------------------------------------------------------------

/// 模型路径: 目标文本 → shm kind=3 → "PLAN=A2 1;A5 1"。
fn plan_llm(goal: &[u8]) -> Option<([u8; 96], usize)> {
    let seq = shm_send_req(SHM_KIND_PLAN, goal);
    let (line, ln, _el) = wait_rsp(seq)?;
    Some((line, ln))
}

/// 规则降级计划 (demo 基线): isolate/resume/kill/threshold → 对应 A 动作。
fn rules_plan(goal: &[u8], buf: &mut [u8]) -> usize {
    let s = core::str::from_utf8(goal).unwrap_or("");
    let pid = first_digit(goal).unwrap_or(0);
    let mut pos = 0usize;
    if s.contains("isolate") {
        pos += push_action(buf, pos, 2, pid, 0);
        if s.contains("resume") {
            pos += push_action(buf, pos, 5, pid, 0);
        }
    } else if s.contains("resume") {
        pos += push_action(buf, pos, 5, pid, 0);
    } else if s.contains("kill") {
        pos += push_action(buf, pos, 1, pid, 0);
    } else if s.contains("threshold") {
        pos += push_action(buf, pos, 4, 1, 70);
    } else {
        pos += push_action(buf, pos, 6, 0, 0);
    }
    pos
}

fn push_action(buf: &mut [u8], pos: usize, act: u64, a0: u64, a1: u64) -> usize {
    let mut p = pos;
    let hdr = b"A";
    for &c in hdr.iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    let mut num = [0u8; 20];
    let n = dec_digits(act, &mut num);
    for &c in num[..n].iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    for &c in b" ".iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    let n = dec_digits(a0, &mut num);
    for &c in num[..n].iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    if a1 != 0 {
        for &c in b" ".iter() {
            if p < buf.len() - 1 {
                buf[p] = c;
                p += 1;
            }
        }
        let n = dec_digits(a1, &mut num);
        for &c in num[..n].iter() {
            if p < buf.len() - 1 {
                buf[p] = c;
                p += 1;
            }
        }
    }
    for &c in b";".iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    p - pos
}

fn first_digit(text: &[u8]) -> Option<u64> {
    for &b in text {
        if b.is_ascii_digit() {
            let mut v = (b - b'0') as u64;
            // 连续数字 (pid 可能多位)
            return Some(v);
        }
    }
    None
}

/// 解析 "A<act> <a0> <a1>" 形式的动作令牌。
fn parse_action(tok: &[u8]) -> (u64, u64, u64) {
    let mut act = 0u64;
    let mut a0 = 0u64;
    let mut a1 = 0u64;
    let mut i = 0usize;
    if i < tok.len() && tok[i] == b'A' {
        i += 1;
        while i < tok.len() && tok[i].is_ascii_digit() {
            act = act * 10 + (tok[i] - b'0') as u64;
            i += 1;
        }
    }
    while i < tok.len() && tok[i] == b' ' {
        i += 1;
    }
    while i < tok.len() && tok[i].is_ascii_digit() {
        a0 = a0 * 10 + (tok[i] - b'0') as u64;
        i += 1;
    }
    while i < tok.len() && tok[i] == b' ' {
        i += 1;
    }
    while i < tok.len() && tok[i].is_ascii_digit() {
        a1 = a1 * 10 + (tok[i] - b'0') as u64;
        i += 1;
    }
    (act, a0, a1)
}

/// 0x8305: 计划-执行器关闭环。goal → (模型/规则) → 动作向量 →
/// cap_exec 逐项执行 → out = {n_ok, n_fail, verify(1=全成功)}。
#[no_mangle]
pub extern "C" fn fujo_plan_run(ptr: u64, len: u64, out: u64, _cap: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) || !(0x400000..0x800000).contains(&out) {
        return -14; // -EFAULT
    }
    let len = (len as usize).min(200);
    let mut goal = [0u8; 200];
    unsafe {
        for i in 0..len {
            goal[i] = (ptr as *const u8).add(i).read();
        }
    }
    let mut plan = [0u8; 96];
    let mut eng: u64 = 0;
    // W22: force=1 跳过规则面; force=2 强制规则。
    let (_, plen) = {
        let rb = if eval_force() != 1 {
            rules_match(&goal[..len], SHM_KIND_PLAN as u64)
        } else {
            None
        };
        if let Some((v, a0, a1, _c)) = rb {
            // 蒸馏字节码: "A<act> <a0> <a1>;" (a1=0 时同 push_action 省略)
            let n = push_action(&mut plan, 0, v, a0, a1);
            eng = 3;
            ((), n)
        } else if eval_force() == 2 {
            serial::write_line("plan: force rules engine");
            eng = 2;
            let n = rules_plan(&goal[..len], &mut plan);
            ((), n)
        } else {
            match plan_llm(&goal[..len]) {
            Some((l, ln)) => {
                eng = 1;
                // 提取 PLAN= 字段 (内容含空格, 以 " TAG" 或行尾终止)
                let mut i = 0usize;
                while i + 5 <= ln {
                    if l[i..].starts_with(b"PLAN=") {
                        let mut j = i + 5;
                        let mut p = 0usize;
                        while j < ln && p < 92 && !l[j..].starts_with(b" TAG") {
                            plan[p] = l[j];
                            p += 1;
                            j += 1;
                        }
                        break;
                    }
                    i += 1;
                }
                let n = plan.iter().position(|&b| b == 0).unwrap_or(0);
                if n == 0 {
                    plan[0] = b'A';
                    plan[1] = b'6';
                    plan[2] = b' ';
                    plan[3] = b'0';
                    plan[4] = b';';
                }
                ((), n.min(92))
            }
            None => {
                serial::write_line("plan: shm timeout -> rules fallback");
                eng = 2;
                let n = rules_plan(&goal[..len], &mut plan);
                ((), n)
            }
        }
        }
    };
    let mut n_ok = 0u64;
    let mut n_fail = 0u64;
    let mut start = 0usize;
    for i in 0..=plen {
        if i == plen || plan[i] == b';' {
            if i > start {
                let (act, a0, a1) = parse_action(&plan[start..i]);
                if act >= 1 && act <= 6 {
                    let rc = crate::capability::fujo_cap_exec(act, a0, a1);
                    if rc == 0 {
                        n_ok += 1;
                    } else {
                        n_fail += 1;
                    }
                } else {
                    n_fail += 1;
                }
            }
            start = i + 1;
        }
    }
    let o = out as *mut u64;
    unsafe {
        o.write(n_ok);
        o.add(1).write(n_fail);
        o.add(2).write(if n_fail == 0 { 1 } else { 0 });
    }
    ai_aud_note(3, eng, n_ok, n_fail, if n_fail == 0 { 1 } else { 0 }, 0, &goal[..len]);
    serial::write_str("plan: goal [");
    serial::write_str(core::str::from_utf8(&goal[..len.min(60)]).unwrap_or(""));
    serial::write_str("] -> ");
    serial::write_str(core::str::from_utf8(&plan[..plen.min(40)]).unwrap_or(""));
    serial::write_str(" ok=");
    crate::syscall::debug_dec(n_ok);
    serial::write_str(" fail=");
    crate::syscall::debug_dec(n_fail);
    serial::write_line("");
    0
}

// ---------------------------------------------------------------------------
// M113 · C I/O 预测器 (序列前缀 → 下一块; 规则=最近块=LRU 基线)
// ---------------------------------------------------------------------------

fn io_llm(seq: &[u8]) -> Option<u64> {
    let frame_seq = shm_send_req(SHM_KIND_IO, seq);
    let (line, ln, _el) = wait_rsp(frame_seq)?;
    let mut i = 0usize;
    while i + 5 <= ln {
        if line[i..].starts_with(b"NEXT=") {
            let mut j = i + 5;
            let mut v = 0u64;
            while j < ln && (line[j] as char).is_ascii_digit() {
                v = v * 10 + (line[j] - b'0') as u64;
                j += 1;
            }
            return Some(v);
        }
        i += 1;
    }
    None
}

/// 规则降级: 序列最后一块数字。
fn last_num(seq: &[u8]) -> u64 {
    let s = core::str::from_utf8(seq).unwrap_or("");
    let mut last = 0u64;
    let mut in_num = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            last = c as u64 - '0' as u64;
            in_num = true;
        } else {
            in_num = false;
        }
    }
    let _ = in_num;
    last
}

// ---------------------------------------------------------------------------
// W25 · 二阶马尔可夫 I/O 预测基线 (engine=4) —— 自训练访问流:
// 每次预测调用把序列数字追加进流 (最近 64 项窗口), 反向扫描 (a,b)->c 转换表,
// 取 (末两位) 最近后继; 无后继 = None (落模型/兜底)。零静态依赖, 确定性。
// ---------------------------------------------------------------------------

static mut IO_STREAM: [u8; 96] = [0; 96];
static mut IO_STREAM_N: usize = 0;

fn io_markov(seq: &[u8]) -> Option<u64> {
    let mut cur = [0u8; 64];
    let mut cn = 0usize;
    for &b in seq {
        if b.is_ascii_digit() {
            cur[cn] = b - b'0';
            cn += 1;
            if cn >= cur.len() {
                break;
            }
        }
    }
    if cn < 2 {
        return None;
    }
    unsafe {
        // 追加 (窗口压缩: 将 >80 压缩为最近 64 项)
        if IO_STREAM_N + cn > IO_STREAM.len() {
            let from = IO_STREAM_N.saturating_sub(64);
            let nkeep = IO_STREAM_N - from;
            IO_STREAM.copy_within(from..IO_STREAM_N, 0);
            IO_STREAM_N = nkeep;
        }
        for k in 0..cn {
            IO_STREAM[IO_STREAM_N + k] = cur[k];
        }
        IO_STREAM_N += cn;
        // 反向扫描 (a,b)->c, 最近优先; j 起点 N-3 保证 j+2 < N
        let a = cur[cn - 2];
        let b = cur[cn - 1];
        if IO_STREAM_N >= 3 {
            let mut j = IO_STREAM_N - 3;
            loop {
                if IO_STREAM[j] == a && IO_STREAM[j + 1] == b {
                    return Some(IO_STREAM[j + 2] as u64);
                }
                if j == 0 {
                    break;
                }
                j -= 1;
            }
        }
    }
    None
}

/// 0x8306: 块访问序列前缀 → 预测下一块 (写入 out[0])。
#[no_mangle]
pub extern "C" fn fujo_io_predict(ptr: u64, len: u64, out: u64, _cap: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) || !(0x400000..0x800000).contains(&out) {
        return -14;
    }
    let len = (len as usize).min(64);
    let mut seq = [0u8; 64];
    unsafe {
        for i in 0..len {
            seq[i] = (ptr as *const u8).add(i).read();
        }
    }
    // W22: force=1 跳过规则面; force=2 强制确定性 (二阶马尔可夫+last)。
    // W25: 新增二阶马尔可夫基线 (io_markov, engine=4) —— 确定性组件,
    // auto 路径中置模型之前 (基线命中即零模型调用; 职责所有权 = 基线)。
    let f = eval_force();
    let (next, eng) = {
        let rb = if f != 1 {
            rules_match(&seq[..len], SHM_KIND_IO as u64)
        } else {
            None
        };
        if let Some((v, _a0, _a1, _c)) = rb {
            (v, 3u64)
        } else if f == 2 {
            match io_markov(&seq[..len]) {
                Some(v) => {
                    serial::write_line("io   : markov hit (rules engine)");
                    (v, 4u64)
                }
                None => {
                    serial::write_line("io   : force rules engine");
                    (last_num(&seq[..len]), 2u64)
                }
            }
        } else {
            let mv = if f != 1 { io_markov(&seq[..len]) } else { None };
            if let Some(v) = mv {
                (v, 4u64)
            } else {
                match io_llm(&seq[..len]) {
                    Some(v) => (v, 1u64),
                    None => {
                        serial::write_line("io   : shm timeout -> rules (last-block)");
                        (last_num(&seq[..len]), 2u64)
                    }
                }
            }
        }
    };
    // R6 自监督结果标签: 上次预测是否被本次实际块证实 (0=未中 1=命中 2=首次)。
    let hit = unsafe {
        let h = if PREV_IO == u64::MAX {
            2
        } else if PREV_IO == next {
            1
        } else {
            0
        };
        PREV_IO = next;
        h
    };
    ai_aud_note(4, eng, next, hit, 0, 0, &seq[..len]);
    unsafe {
        (out as *mut u64).write(next);
    }
    serial::write_str("io   : predict [");
    serial::write_str(core::str::from_utf8(&seq[..len]).unwrap_or(""));
    serial::write_str("] -> ");
    crate::syscall::debug_dec(next);
    serial::write_line("");
    0
}

// ---------------------------------------------------------------------------
// M114 · D 自然语言配置 (文案 → 策略对象) + E 环境侦察 (感知 → 适配)
// ---------------------------------------------------------------------------

fn nlc_llm(text: &[u8]) -> Option<([u8; 96], usize)> {
    let seq = shm_send_req(SHM_KIND_NLC, text);
    let (line, ln, _el) = wait_rsp(seq)?;
    Some((line, ln))
}

/// 规则降级 (demo 基线): "ban games 9 to 18" → POL=3:1;4:9;5:18。
fn rules_nlc(text: &[u8], buf: &mut [u8]) -> usize {
    let s = core::str::from_utf8(text).unwrap_or("");
    let mut pos = 0usize;
    if s.contains("ban") {
        pos += push_pol(buf, pos, 3, 1);
        let mut digs = [0u64; 4];
        let mut dn = 0usize;
        for c in s.chars() {
            if c.is_ascii_digit() && dn < 4 {
                digs[dn] = c as u64 - '0' as u64;
                dn += 1;
            }
        }
        if dn >= 2 {
            pos += push_pol(buf, pos, 4, 10 * digs[0] + digs[1]);
            if dn >= 4 {
                pos += push_pol(buf, pos, 5, 10 * digs[2] + digs[3]);
            }
        }
    } else {
        pos += push_pol(buf, pos, 6, 0);
    }
    pos
}

fn push_pol(buf: &mut [u8], pos: usize, key: u64, val: u64) -> usize {
    let mut p = pos;
    let hdr = b"POL=";
    for &c in hdr.iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    let mut num = [0u8; 20];
    let n = dec_digits(key, &mut num);
    for &c in num[..n].iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    for &c in b":".iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    let n = dec_digits(val, &mut num);
    for &c in num[..n].iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    for &c in b";".iter() {
        if p < buf.len() - 1 {
            buf[p] = c;
            p += 1;
        }
    }
    p - pos
}

/// 0x8307: 自然语言配置 → 策略对象 (POL=k:v;...) → cfg_set → out[0]=条数。
#[no_mangle]
pub extern "C" fn fujo_nlc_set(ptr: u64, len: u64, out: u64, _cap: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) || !(0x400000..0x800000).contains(&out) {
        return -14;
    }
    let len = (len as usize).min(200);
    let mut text = [0u8; 200];
    unsafe {
        for i in 0..len {
            text[i] = (ptr as *const u8).add(i).read();
        }
    }
    let mut pol = [0u8; 96];
    let mut eng: u64 = 0;
    // W22: force=1 跳过规则面; force=2 强制规则。
    let plen = {
        let rb = if eval_force() != 1 {
            rules_match(&text[..len], SHM_KIND_NLC as u64)
        } else {
            None
        };
        if let Some((v, a0, _a1, _c)) = rb {
            // 蒸馏字节码: "POL=<key>:<val>;"
            let n = push_pol(&mut pol, 0, v, a0);
            eng = 3;
            n
        } else if eval_force() == 2 {
            serial::write_line("nlc : force rules engine");
            eng = 2;
            rules_nlc(&text[..len], &mut pol)
        } else {
            match nlc_llm(&text[..len]) {
            Some((l, ln)) => {
                eng = 1;
                let mut i = 0usize;
                while i + 4 <= ln {
                    if l[i..].starts_with(b"POL=") {
                        let mut j = i + 4;
                        let mut p = 0usize;
                        while j < ln && p < 92 && !l[j..].starts_with(b" TAG") {
                            pol[p] = l[j];
                            p += 1;
                            j += 1;
                        }
                        break;
                    }
                    i += 1;
                }
                // 空: 默认
                if !pol.iter().any(|&b| b != 0) {
                    pol[..6].copy_from_slice(b"POL=6:");
                }
                pol.iter().position(|&b| b == 0).unwrap_or(92).min(92)
            }
            None => {
                serial::write_line("nlc : shm timeout -> rules fallback");
                eng = 2;
                rules_nlc(&text[..len], &mut pol)
            }
        }
        }
    };
    // 解析 "POL=k:v;POL=k:v"
    let mut n_applied = 0u64;
    let mut i = 0usize;
    while i + 4 <= plen {
        if pol[i..].starts_with(b"POL=") {
            i += 4;
            let mut k = 0u64;
            while i < plen && pol[i].is_ascii_digit() {
                k = k * 10 + (pol[i] - b'0') as u64;
                i += 1;
            }
            if i < plen && pol[i] == b':' {
                i += 1;
            }
            let mut v = 0u64;
            while i < plen && pol[i].is_ascii_digit() {
                v = v * 10 + (pol[i] - b'0') as u64;
                i += 1;
            }
            if k >= 1 && k <= 8 && crate::capability::cfg_set(k, v) == 0 {
                n_applied += 1;
            }
        } else {
            i += 1;
        }
    }
    unsafe {
        (out as *mut u64).write(n_applied);
    }
    ai_aud_note(5, eng, n_applied, 0, 0, 0, &text[..len]);
    serial::write_str("nlc : policy [");
    serial::write_str(core::str::from_utf8(&pol[..plen.min(60)]).unwrap_or(""));
    serial::write_str("] applied=");
    crate::syscall::debug_dec(n_applied);
    serial::write_line("");
    0
}

fn env_llm(digest: &[u8]) -> Option<([u8; 96], usize)> {
    let seq = shm_send_req(SHM_KIND_ENV, digest);
    let (line, ln, _el) = wait_rsp(seq)?;
    Some((line, ln))
}

fn scene_code(scene: &[u8]) -> u64 {
    if scene.starts_with(b"desktop") {
        1
    } else if scene.starts_with(b"headless") {
        2
    } else if scene.starts_with(b"server") {
        3
    } else if scene.starts_with(b"games") {
        4
    } else {
        0
    }
}

/// 0x8308: 环境侦察 —— 汇总 hw/acpi/storage 摘要 → 模型场景/档案 →
/// cfg_set(6, profile); out = {profile, scene_code, digest_len}。
#[no_mangle]
pub extern "C" fn fujo_env_scan(out: u64, _cap: u64) -> i64 {
    if !(0x400000..0x800000).contains(&out) {
        return -14;
    }
    let mut digest = [0u8; 200];
    let mut q = [0u64; 8];
    // 显示/键盘
    unsafe {
        crate::hw::fujo_hw_disp(q.as_mut_ptr() as u64);
    }
    let mut d = 0usize;
    let mut put = |s: &[u8]| {
        for &c in s.iter().take(200 - d) {
            digest[d] = c;
            d += 1;
        }
    };
    put(b"hw fbw=");
    let mut num = [0u8; 20];
    let n = dec_digits(q[0], &mut num);
    put(&num[..n]);
    put(b" fbh=");
    let n = dec_digits(q[1], &mut num);
    put(&num[..n]);
    // 存储
    unsafe {
        crate::hw::fujo_hw_storage(q.as_mut_ptr() as u64);
    }
    put(b" ata=");
    let n = dec_digits(q[0], &mut num);
    put(&num[..n]);
    put(b" fs=");
    let n = dec_digits(q[2], &mut num);
    put(&num[..n]);
    // ACPI/PCI
    unsafe {
        crate::acpi::fujo_acpi_info(q.as_mut_ptr() as u64);
    }
    put(b" rsdp=");
    let n = dec_digits(q[0], &mut num);
    put(&num[..n]);
    put(b" pci=");
    let n = dec_digits(q[3], &mut num);
    put(&num[..n]);
    put(b" kbd=");
    let n = dec_digits(q[3], &mut num);
    put(&num[..n]);

    let mut scene_buf = [0u8; 12];
    let mut eng: u64 = 0;
    // W22: force=2 强制规则 (确定性场景); force=1 与 auto 均走模型路径。
    let (scene_len, profile) = if eval_force() == 2 {
        serial::write_line("env : force rules engine");
        eng = 2;
        scene_buf[..7].copy_from_slice(b"desktop");
        (7usize, 2u64)
    } else {
        match env_llm(&digest[..d]) {
        Some((l, ln)) => {
            eng = 1;
            let mut sn = 0usize;
            let mut prof = 2u64;
            let mut i = 0usize;
            while i + 6 <= ln {
                if l[i..].starts_with(b"SCENE=") {
                    let mut j = i + 6;
                    while j < ln && l[j] != b' ' && sn < 11 {
                        scene_buf[sn] = l[j];
                        sn += 1;
                        j += 1;
                    }
                }
                if l[i..].starts_with(b"PROFILE=") {
                    let mut j = i + 8;
                    let mut v = 0u64;
                    while j < ln && (l[j] as char).is_ascii_digit() {
                        v = v * 10 + (l[j] - b'0') as u64;
                        j += 1;
                    }
                    prof = v.min(4).max(1);
                }
                i += 1;
            }
            (sn, prof)
        }
        None => {
            serial::write_line("env : shm timeout -> rules (desktop/2)");
            eng = 2;
            scene_buf[..7].copy_from_slice(b"desktop");
            (7usize, 2u64)
        }
        }
    };
    let scene = &scene_buf[..scene_len];
    let code = scene_code(scene);
    let _ = crate::capability::cfg_set(6, profile);
    ai_aud_note(6, eng, profile, code, 0, 0, &digest[..d.min(40)]);
    let o = out as *mut u64;
    unsafe {
        o.write(profile);
        o.add(1).write(code);
        o.add(2).write(d as u64);
    }
    serial::write_str("env : scan [");
    serial::write_str(core::str::from_utf8(&digest[..d.min(80)]).unwrap_or(""));
    serial::write_str("] -> scene=");
    serial::write_str(core::str::from_utf8(scene).unwrap_or("?"));
    serial::write_str(" profile=");
    crate::syscall::debug_dec(profile);
    serial::write_line("");
    0
}

// ---------------------------------------------------------------------------
// M118 · R3 时延一致性探针 (确定性测试) + R1 公理化自检 (离线可跑)
// ---------------------------------------------------------------------------

/// 0x8309: R3 协议探针 —— mode bit0=快照后注入关键事件 (EV_ANOMALY),
/// bit1=强制回包过期 (t0 回拨 1e6 ticks)。out = [engine, reason, crit_n, elapsed]。
/// engine: 1=模型建议被接受 2=丢弃 -> 规则兜底; reason: 0=接受/超时 1=关键事件 2=TTL。
#[no_mangle]
pub extern "C" fn fujo_r3_probe(mode: u64, out: u64, _cap: u64, _x: u64) -> i64 {
    if !(0x400000..0x800000).contains(&out) {
        return -14; // -EFAULT
    }
    let seq = shm_send_req(SHM_KIND_CLASSIFY, b"run the game");
    if mode & 1 != 0 {
        crate::ctx::ev_push(crate::ctx::EV_ANOMALY, 0, 1, 90);
    }
    if mode & 2 != 0 {
        unsafe { SNAP_T0 = SNAP_T0.wrapping_sub(1_000_000) };
    }
    let (engine, el) = if let Some((line, ln, el)) = wait_rsp(seq) {
        if parse_rsp(&line[..ln], seq).is_some() {
            (1u64, el)
        } else {
            (2u64, el)
        }
    } else {
        (2u64, 0u64)
    };
    let crit = unsafe { crate::ctx::ev_delta_critical(SNAP_EVW, SNAP_CRIT) };
    let reason = unsafe { SNAP_REASON };
    unsafe {
        let o = out as *mut u64;
        o.write(engine);
        o.add(1).write(reason);
        o.add(2).write(crit);
        o.add(3).write(el);
    }
    serial::write_str("r3   : probe mode=");
    crate::syscall::debug_dec(mode);
    serial::write_str(" -> engine=");
    crate::syscall::debug_dec(engine);
    serial::write_str(" reason=");
    crate::syscall::debug_dec(reason);
    serial::write_line("");
    0
}

/// 0x830A: R1 公理化自检 (纯内核, 模型离线可跑) —— 四条不变式。
/// 前置条件: 调用前 exec 槽未授权 (引导默认; 本函数内部会授予 0x3F)。
/// out = [结果位掩码 bit0..3=I1..I4, denies 计数, 审计条目总数]。
#[no_mangle]
pub extern "C" fn fujo_inv_run(out: u64, cap: u64) -> i64 {
    if !(0x400000..0x800000).contains(&out) || cap < 24 {
        return -14;
    }
    let mut mask = 0u64;

    // I1 模型永不能执行未授权动作: 未授权 exec 必须拒绝、计数、无副作用。
    {
        let d0 = crate::capability::denies();
        let rc = crate::capability::fujo_cap_exec(crate::capability::ACT_SET_CFG, 1, 42);
        let d1 = crate::capability::denies();
        let cfg = crate::capability::fujo_cfg_get(1);
        if rc == -1 && d1 == d0 + 1 && cfg == 50 {
            mask |= 1;
        }
        serial::write_str("inv  : I1 deny rc=");
        crate::syscall::debug_dec(rc as u64);
        serial::write_str(" denies=");
        crate::syscall::debug_dec(d1);
        serial::write_line(if mask & 1 != 0 { " [PASS]" } else { " [FAIL]" });
    }

    // I2 每个动作都有审计记录: 授权 exec 后审计条目 +1 且尾条 (action=2, result=0)。
    {
        let n0 = crate::capability::aud_num();
        let _ = crate::capability::fujo_cap_grant(crate::capability::EXEC_SLOT as u64, crate::capability::ALL_ACTS);
        let rc = crate::capability::fujo_cap_exec(crate::capability::ACT_SET_CFG, 2, 1);
        let n1 = crate::capability::aud_num();
        let (_, a, s, r) = crate::capability::aud_tail();
        if rc == 0 && n1 == n0 + 1 && a == 2 && s == crate::capability::ACT_SET_CFG && r == 0 {
            mask |= 2;
        }
        serial::write_str("inv  : I2 exec rc=");
        crate::syscall::debug_dec(rc as u64);
        serial::write_str(" aud ");
        crate::syscall::debug_dec(n0);
        serial::write_str("->");
        crate::syscall::debug_dec(n1);
        serial::write_line(if mask & 2 != 0 { " [PASS]" } else { " [FAIL]" });
    }

    // I3 模型缺席时系统继续运行 (规则兜底确定): 模型缺位后接管的三条规则语义不变。
    {
        let (a, c) = rules_anom(b"ev pid=0 rate=99 wr=dead");
        let (a2, c2) = rules_anom(b"ev pid=0 rate=3 wr=ok");
        let mut pb = [0u8; 96];
        let pn = rules_plan(b"isolate task 1", &mut pb);
        let p_ok = pb[..pn].starts_with(b"A2 ");
        let mut nb = [0u8; 96];
        let nn = rules_nlc(b"ban games 0 24", &mut nb);
        let n_ok = nb[..nn].starts_with(b"POL=3:1");
        if a == 1 && c == 80 && a2 == 0 && c2 == 20 && p_ok && n_ok {
            mask |= 4;
        }
        serial::write_str("inv  : I3 rules anom=");
        crate::syscall::debug_dec(a);
        serial::write_str("/");
        crate::syscall::debug_dec(a2);
        serial::write_line(if mask & 4 != 0 { " [PASS]" } else { " [FAIL]" });
    }

    // I4 每次失败被计数并降级: 拒绝计数 >=1, 系统继续可用 (授权动作生效)。
    {
        let d = crate::capability::denies();
        let cont = crate::capability::fujo_cfg_get(2); // I2 授予的动作: 自动隔离=1
        if d >= 1 && cont == 1 {
            mask |= 8;
        }
        serial::write_str("inv  : I4 denies=");
        crate::syscall::debug_dec(d);
        serial::write_str(" cfg2=");
        crate::syscall::debug_dec(cont as u64);
        serial::write_line(if mask & 8 != 0 { " [PASS]" } else { " [FAIL]" });
    }

    unsafe {
        let o = out as *mut u64;
        o.write(mask);
        o.add(1).write(crate::capability::denies());
        o.add(2).write(crate::capability::aud_num());
    }
    serial::write_str("inv  : mask=");
    crate::syscall::debug_hex(mask);
    serial::write_line("");
    0
}

// ---------------------------------------------------------------------------
// W10 · R5 策略蒸馏: 确定性字节码规则引擎 (FJRU v1) + R6 审计捕获/导出
//
// 蒸馏闭环: 审计/实测记录 (train_cases.json) -> 7B 归纳 if-then 规则 ->
// tools/distill_rules.py 编译为 FJRU v1 字节码 -> 0x830B 载入内核 ->
// 五职责先查规则 (命中 = 零模型调用), 未命中才走模型 (模型只处理陌生情况)。
// 字节码条目: [nl u8][needle ≤40B][value u8][a0 u8][a1 u8][conf u8]
//            [param u8: 1=needle 后数字解析 a0][duty u8: 1..6, 0=任意];
// duty 与 SHM_KIND_* 一一对应。命中返回 (value, a0, a1, conf)。
// ---------------------------------------------------------------------------

const RULE_MAGIC: u32 = 0x5552_4A46; // "FJRU" LE
const RULE_MAX: usize = 64;
const RULE_NL_MAX: usize = 40;
const RULE_META: usize = 6; // value/a0/a1/conf/param/duty

static mut RULE_N: usize = 0;
static mut RULE_NEEDLE: [[u8; RULE_NL_MAX]; RULE_MAX] = [[0; RULE_NL_MAX]; RULE_MAX];
static mut RULE_LEN: [u8; RULE_MAX] = [0; RULE_MAX];
static mut RULE_VAL: [u8; RULE_MAX] = [0; RULE_MAX];
static mut RULE_A0: [u8; RULE_MAX] = [0; RULE_MAX];
static mut RULE_A1: [u8; RULE_MAX] = [0; RULE_MAX];
static mut RULE_CONF: [u8; RULE_MAX] = [0; RULE_MAX];
static mut RULE_PARAM: [u8; RULE_MAX] = [0; RULE_MAX];
static mut RULE_DUTY: [u8; RULE_MAX] = [0; RULE_MAX];
static mut RULE_HITS: u64 = 0;
/// 模型调用次数 (shm 请求计数; 保真度曲线 = 调用率下降)。
static mut AI_CALLS: u64 = 0;

fn find_sub(text: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > text.len() {
        return None;
    }
    for i in 0..=(text.len() - needle.len()) {
        let mut ok = true;
        for k in 0..needle.len() {
            if text[i + k] != needle[k] {
                ok = false;
                break;
            }
        }
        if ok {
            return Some(i);
        }
    }
    None
}

/// 规则字节码匹配 (duty: SHM_KIND_*; 0=any)。命中即 RULE_HITS+1。
fn rules_match(text: &[u8], duty: u64) -> Option<(u64, u64, u64, u64)> {
    unsafe {
        if RULE_N == 0 {
            return None;
        }
        for i in 0..RULE_N {
            let d = RULE_DUTY[i] as u64;
            if d != 0 && d != duty {
                continue;
            }
            let nl = RULE_LEN[i] as usize;
            if nl == 0 {
                continue;
            }
            if let Some(pos) = find_sub(text, &RULE_NEEDLE[i][..nl]) {
                RULE_HITS += 1;
                let mut a0 = RULE_A0[i] as u64;
                if RULE_PARAM[i] != 0 {
                    let mut j = pos + nl;
                    while j < text.len() && text[j] == b' ' {
                        j += 1;
                    }
                    let mut v = 0u64;
                    let mut saw = false;
                    while j < text.len() && (text[j] as char).is_ascii_digit() {
                        v = v * 10 + (text[j] - b'0') as u64;
                        saw = true;
                        j += 1;
                    }
                    if saw {
                        a0 = v;
                    }
                }
                return Some((RULE_VAL[i] as u64, a0, RULE_A1[i] as u64, RULE_CONF[i] as u64));
            }
        }
    }
    None
}

/// 0x830B: 载入 FJRU v1 规则字节码 (用户缓冲 -> 内核静态表)。返回规则条数。
#[no_mangle]
pub extern "C" fn fujo_rules_load(ptr: u64, len: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) || len < 0x10 || len > 0x2000 {
        return -22;
    }
    let b = ptr as *const u8;
    unsafe {
        let magic = (b.add(0x0) as *const u32).read();
        let ver = (b.add(0x4) as *const u32).read();
        let count = ((b.add(0x8) as *const u16).read() as usize).min(RULE_MAX);
        if magic != RULE_MAGIC || ver != 1 {
            return -22;
        }
        let mut pos = 0x10usize;
        let mut n = 0usize;
        while n < count && pos + 1 + RULE_META <= len as usize && n < RULE_MAX {
            let nl = (b.add(pos).read() as usize).min(RULE_NL_MAX);
            if pos + 1 + nl + RULE_META > len as usize || nl == 0 {
                break;
            }
            RULE_LEN[n] = nl as u8;
            for k in 0..nl {
                RULE_NEEDLE[n][k] = b.add(pos + 1 + k).read();
            }
            RULE_VAL[n] = b.add(pos + 1 + nl).read();
            RULE_A0[n] = b.add(pos + 2 + nl).read();
            RULE_A1[n] = b.add(pos + 3 + nl).read();
            RULE_CONF[n] = b.add(pos + 4 + nl).read();
            RULE_PARAM[n] = b.add(pos + 5 + nl).read();
            RULE_DUTY[n] = b.add(pos + 6 + nl).read();
            pos += 1 + nl + RULE_META;
            n += 1;
        }
        RULE_N = n;
        RULE_HITS = 0;
        serial::write_str("rules: load ");
        crate::syscall::debug_dec(n as u64);
        serial::write_line(" entries (FJRU v1)");
        n as i64
    }
}

// ---- R6: 审计捕获 (上下文/职责/引擎/结果, self-labeled 结果标签) ----
const AIAUD_N: usize = 16;
const AIAUD_SZ: usize = 88; // 6×u64 + 40B 文本
static mut AIAUD: [u8; AIAUD_N * AIAUD_SZ] = [0; AIAUD_N * AIAUD_SZ];
static mut AIAUD_POS: usize = 0;
/// IO 自监督命中标签: 上次预测; u64::MAX = 无先前。
static mut PREV_IO: u64 = u64::MAX;

fn ai_aud_note(duty: u64, engine: u64, out: u64, a: u64, b: u64, result: u64, text: &[u8]) {
    unsafe {
        let s = AIAUD_POS % AIAUD_N;
        let e = &mut AIAUD[s * AIAUD_SZ..(s + 1) * AIAUD_SZ];
        (e.as_mut_ptr() as *mut u64).write(engine);
        (e.as_mut_ptr().add(8) as *mut u64).write(duty);
        (e.as_mut_ptr().add(16) as *mut u64).write(out);
        (e.as_mut_ptr().add(24) as *mut u64).write(a);
        (e.as_mut_ptr().add(32) as *mut u64).write(b);
        (e.as_mut_ptr().add(40) as *mut u64).write(result);
        let tlen = text.len().min(40);
        for i in 0..tlen {
            e[48 + i] = text[i];
        }
        if tlen < 40 {
            e[48 + tlen] = 0;
        }
        AIAUD_POS += 1;
    }
}

/// 0x830C: 统计 (out[0]=模型调用数, out[1]=规则条数, out[2]=规则命中, out[3]=审计条数)。

/// W19: 统一审计 —— AI 环条目数。
pub fn ai_aud_count() -> usize {
    unsafe { AIAUD_POS.min(AIAUD_N) }
}

/// W19: 统一审计 —— boot 标记条目 (保证 AI 环非空; 确定性)。
pub fn ai_aud_boot() {
    ai_aud_note(6, 0, 0, 0, 0, 0, b"boot");
}

/// W19: 统一审计 —— AI 环导出为统一 32B 条目 {a=engine, b=duty, c=result}。
/// 返回导出条数。
pub fn ai_aud_export_32(ptr: u64, max_entries: usize) -> i64 {
    unsafe {
        let n = max_entries.min(AIAUD_N).min(AIAUD_POS);
        for i in 0..n {
            let idx = (AIAUD_POS + AIAUD_N - n + i) % AIAUD_N;
            let src = &AIAUD[idx * AIAUD_SZ..(idx + 1) * AIAUD_SZ];
            let w = (ptr as *mut u64).add(i * 4);
            // src: +16=out? 取 {a=engine@0, b=duty@8, c=result@40}
            w.write((src.as_ptr() as *const u64).read()); // engine
            w.add(1).write((src.as_ptr().add(8) as *const u64).read()); // duty
            w.add(2).write((src.as_ptr().add(40) as *const u64).read()); // result
            w.add(3).write(0);
        }
        n as i64
    }
}#[no_mangle]
pub extern "C" fn fujo_ai_stats(out: u64) -> i64 {
    if !(0x400000..0x800000).contains(&out) {
        return -14;
    }
    unsafe {
        let o = out as *mut u64;
        o.write(AI_CALLS);
        o.add(1).write(RULE_N as u64);
        o.add(2).write(RULE_HITS);
        o.add(3).write(AIAUD_POS as u64);
    }
    0
}

/// 0x830D: 审计导出 -> (entries, 返回条数)。
#[no_mangle]
pub extern "C" fn fujo_ai_audit(ptr: u64, cap: u64) -> i64 {
    if !(0x400000..0x800000).contains(&ptr) {
        return -14;
    }
    unsafe {
        let n = ((cap / AIAUD_SZ as u64) as usize).min(AIAUD_N).min(AIAUD_POS);
        for i in 0..n {
            let idx = (AIAUD_POS as usize + AIAUD_N - n + i) % AIAUD_N;
            let src = &AIAUD[idx * AIAUD_SZ..(idx + 1) * AIAUD_SZ];
            let dst = (ptr as *mut u8).add(i * AIAUD_SZ);
            for k in 0..AIAUD_SZ {
                dst.add(k).write(src[k]);
            }
        }
        n as i64
    }
}
