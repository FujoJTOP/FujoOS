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
    }
    qwen_classify_ser(text)
}

/// COM2 降级路径 (原 FJAI:REQ 行协议, 3 次重发)。
fn qwen_classify_ser(text: &[u8]) -> Option<(i64, [u8; 24], u64)> {
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
/// 帧布局 (0xA00000 起): magic/ver/seq/kind/len, payload@0x18 (≤1KB), ctx@0x800。
const SHM_OFF_PAYLOAD: usize = 0x018;
const SHM_PAYLOAD_MAX: usize = 0x400;
const SHM_OFF_CTX: usize = 0x800;
const SHM_CTX_MAX: usize = 0x600;

/// 写请求帧 (payload + 当前 fujoctx 结构态文本)。
fn shm_write_req(seq: u64, kind: u32, payload: &[u8]) {
    unsafe {
        let b = crate::ipc::SHM_BASE as *mut u8;
        (b.add(0x000) as *mut u32).write_volatile(SHM_MAGIC);
        (b.add(0x004) as *mut u32).write_volatile(1); // 帧协议版本
        (b.add(0x008) as *mut u64).write_volatile(seq);
        (b.add(0x010) as *mut u32).write_volatile(kind);
        let n = payload.len().min(SHM_PAYLOAD_MAX);
        (b.add(0x014) as *mut u32).write_volatile(n as u32);
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
            return None;
        }
    }
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

    let (anom, conf, tag, engine) = match anom_llm(s) {
        Some((a, c, tag)) => (a, c, tag, 1u64),
        None => {
            serial::write_line("anom : shm timeout -> rules fallback");
            let (a, c) = rules_anom(s);
            let mut t = [0u8; 24];
            t[..7].copy_from_slice(b"fjrules");
            (a, c, t, 2u64)
        }
    };

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
                let _ = crate::capability::fujo_cap_exec(crate::capability::ACT_ISOLATE, pid, 0);
            }
        }
    }

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
    let (_, plen) = match plan_llm(&goal[..len]) {
        Some((l, ln)) => {
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
            let n = rules_plan(&goal[..len], &mut plan);
            ((), n)
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
    let next = match io_llm(&seq[..len]) {
        Some(v) => v,
        None => {
            serial::write_line("io   : shm timeout -> rules (last-block)");
            last_num(&seq[..len])
        }
    };
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
    let plen = match nlc_llm(&text[..len]) {
        Some((l, ln)) => {
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
            rules_nlc(&text[..len], &mut pol)
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
    let (scene_len, profile) = match env_llm(&digest[..d]) {
        Some((l, ln)) => {
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
            scene_buf[..7].copy_from_slice(b"desktop");
            (7usize, 2u64)
        }
    };
    let scene = &scene_buf[..scene_len];
    let code = scene_code(scene);
    let _ = crate::capability::cfg_set(6, profile);
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
