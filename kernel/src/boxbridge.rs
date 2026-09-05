//! box.rs — W36/B-2 · BOX-BRIDGE v0 盒面（内核侧）
//!
//! 盒 = 内核外供应商（宿主 box_server，跑在它自己的世界里）：FujoOS 不装载其
//! 代码、不实现其语义。协议与模型通道同构（shm 帧请求 + COM2 触发 + COM2 行
//! 应答），内核侧只做五件事：
//!   1) 注册表状态（per-provider：up/hit/total/schema）
//!   2) 动词路由（白名单 1..=4；LEGO > BOX 决策在调用方，内核只拒未知动词）
//!   3) 产物检疫门（帧级：ascii 白名单 + 禁执行魔数 + 动词 schema —— A1' 扩展）
//!   4) 双列台账（duty7 = 契约履约率 / duty8 = 下游可判定谓词，进 0x8314 族）
//!   5) 域门（act7 = BOX_CMD，cap_exec/域宽 与模型同构）
//!
//! 协议帧（应答方向，COM2 行，均为 ASCII）:
//!   FJBOX:RSP  <seq> <status>          状态行 (status: 1=ok 0=busy)
//!   FJBOX:DATA <seq> <off> <n> <text>  产物块 (off 必须连续, n ≤ 64B)
//!   FJBOX:END  <seq>                   完成
//! 请求方向: 共享页 0xA00000 帧 (kind=0xB0, payload = verb(1B)+arg) +
//!           触发线 "FJBOX:REQ <seq> <len>"。
//!
//! ponytail: v0 单 provider 槽 (盒 = 宿主 box_server); 大产物/像素流 = v1
//! (BOXXFR 块流经 tmpfs/带外通道, 需先过 load_end 检查 — docs/106 坑 #5)。

use crate::serial;

pub const BOX_PROV_MAX: usize = 1;
pub const BOX_VERB_HASH: u64 = 1;
pub const BOX_VERB_INFO: u64 = 2;
pub const BOX_VERB_SIZE: u64 = 3;
pub const BOX_VERB_ECHO: u64 = 4;
pub const BOX_VERB_MAX: u64 = 4;
pub const BOX_ARG_MAX: usize = 128;
pub const BOX_BUF_MAX: usize = 512;
pub const BOX_DATA_MAX: usize = 64;
pub const BOX_TTL_TICKS: u64 = 800; // 8s @100Hz (命令型 TTL, 盒慢 — 不套 hint 级)

const BOX_KIND: u32 = 0xB0;
const BOX_LINE_MAX: usize = 128;

static mut BOX_SEQ: u64 = 0;
static mut BOX_BUF: [u8; BOX_BUF_MAX] = [0; BOX_BUF_MAX];
static mut BOX_BUF_N: usize = 0;
static mut BOX_ARG: [u8; BOX_ARG_MAX] = [0; BOX_ARG_MAX];
static mut BOX_ARG_N: usize = 0;
/// per-provider 注册表槽 0 (host box_server; 域 = 1):
static mut PROV_UP: u64 = 0;
static mut PROV_HIT: u64 = 0;
static mut PROV_TOTAL: u64 = 0;
static mut PROV_SCHEMA: u64 = 0;
static mut PROV_LAST_RC: i64 = 0;

/// 行解析辅助: 找第 k 个空格分隔 token 的 (start, len)。
fn tok(line: &[u8], k: usize) -> Option<(usize, usize)> {
    let mut i = 0usize;
    let mut cur = 0usize;
    while i < line.len() {
        // 吞前置空格
        while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
            i += 1;
        }
        if i >= line.len() {
            break;
        }
        let s = i;
        while i < line.len() && line[i] != b' ' && line[i] != b'\t' {
            i += 1;
        }
        if cur == k {
            return Some((s, i - s));
        }
        cur += 1;
    }
    None
}

fn parse_num(t: &[u8]) -> Option<u64> {
    if t.is_empty() {
        return None;
    }
    let mut v: u64 = 0;
    for &c in t {
        if c < b'0' || c > b'9' {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((c - b'0') as u64)?;
    }
    Some(v)
}

/// 产物 ascii 白名单 (检疫: 只收可打印文本; 空字节/控制符即拒)。
fn ascii_ok(d: &[u8]) -> bool {
    for &c in d {
        if c < 0x20 && c != b'\n' && c != b'\r' && c != b'\t' {
            return false;
        }
        if c > 0x7E {
            return false;
        }
    }
    true
}

/// 禁执行魔数: 盒产物进系统前不得携带可执行镜像头 (A1': 产物 == 不可信数据)。
fn exec_magic(d: &[u8]) -> bool {
    if d.len() >= 4 && d[0] == 0x7F && d[1] == b'E' && d[2] == b'L' && d[3] == b'F' {
        return true;
    }
    if d.len() >= 2 && d[0] == b'M' && d[1] == b'Z' {
        return true;
    }
    false
}

fn is_hex(s: &[u8]) -> bool {
    for &c in s {
        if !((c >= b'0' && c <= b'9') || (c >= b'a' && c <= b'f') || (c >= b'A' && c <= b'F')) {
            return false;
        }
    }
    true
}

fn is_digits(s: &[u8]) -> bool {
    if s.is_empty() {
        return false;
    }
    for &c in s {
        if c < b'0' || c > b'9' {
            return false;
        }
    }
    true
}

/// 动词 schema (列2a: 机器可判定谓词; LLM 观察不进域宽 — v0 无列2b)。
/// 返回 true = 产物通过 schema; false = 契约违约 (adapter-fail)。
fn schema_ok(verb: u64, d: &[u8]) -> bool {
    match verb {
        BOX_VERB_HASH => d.len() == 64 && is_hex(d),
        BOX_VERB_INFO => d.len() >= 4,
        BOX_VERB_SIZE => d.len() <= 20 && is_digits(d),
        BOX_VERB_ECHO => unsafe { d.len() == BOX_ARG_N && d == &BOX_ARG[..BOX_ARG_N] },
        _ => false,
    }
}

/// 写请求帧 (共享页 0xA00000, 与模型通道同窗 — 内核单线程, 无并发复用;
/// 布局 v2 同 ai.rs: magic/ver/seq/kind/len/t0/evw/crit, payload @0x30)。
fn box_write_frame(seq: u64, payload: &[u8]) {
    unsafe {
        let b = crate::ipc::SHM_BASE as *mut u8;
        const MG: u32 = 0x4853_4A46; // "FJSH"
        (b.add(0x000) as *mut u32).write_volatile(MG);
        (b.add(0x004) as *mut u32).write_volatile(2u32);
        (b.add(0x008) as *mut u64).write_volatile(seq);
        (b.add(0x010) as *mut u32).write_volatile(BOX_KIND);
        let n = payload.len().min(0x400);
        (b.add(0x014) as *mut u32).write_volatile(n as u32);
        (b.add(0x018) as *mut u64).write_volatile(crate::interrupts::ticks());
        (b.add(0x020) as *mut u64).write_volatile(0);
        (b.add(0x028) as *mut u32).write_volatile(0u32);
        for i in 0..n {
            b.add(0x030 + i).write_volatile(payload[i]);
        }
    }
}

fn box_tx_line(s: &[u8]) {
    serial::ser2_tx_line(s);
}

/// trigger "FJBOX:REQ <seq> <len>\n"
fn box_trigger(seq: u64, len: u64) {
    let mut line = [0u8; 48];
    let mut n = 0;
    for &b in b"FJBOX:REQ ".iter() {
        line[n] = b;
        n += 1;
    }
    let mut nb = [0u8; 24];
    let mut i = 24;
    let mut x = seq;
    if x == 0 {
        i -= 1;
        nb[i] = b'0';
    }
    while x > 0 {
        i -= 1;
        nb[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    while i < 24 {
        line[n] = nb[i];
        n += 1;
        i += 1;
    }
    line[n] = b' ';
    n += 1;
    let mut i = 24;
    let mut x = len;
    if x == 0 {
        i -= 1;
        nb[i] = b'0';
    }
    while x > 0 {
        i -= 1;
        nb[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    while i < 24 {
        line[n] = nb[i];
        n += 1;
        i += 1;
    }
    line[n] = b'\n';
    n += 1;
    box_tx_line(&line[..n]);
}

/// 审计: 盒调用 (action=9) 与产物检疫 (action=10) 进统一审计环。
fn aud_box_call(verb: u64, rc: i64) {
    crate::capability::aud_note(9, verb, if rc == 0 { 0 } else { 1 });
}

fn aud_box_gate(verb: u64, bad: u64) {
    crate::capability::aud_note(10, verb, bad);
}

fn prov_note(hit: u64) {
    unsafe {
        PROV_HIT += hit;
    }
}

/// 0x8316 box_run(verb, arg_ptr, arg_len): 命令进 → 产物回 (同步等待, TTL 超时
/// = 缺席声明 -4; 检疫拒收 = -2; schema 违约 = -3; 域门拒绝 = -1)。
#[no_mangle]
pub extern "C" fn fujo_box_run(verb: u64, argptr: u64, arglen: u64) -> i64 {
    if verb == 0 || verb > BOX_VERB_MAX {
        return -22;
    }
    if arglen as usize > BOX_ARG_MAX {
        return -22;
    }
    if arglen > 0 && !(0x400000..0xC00000).contains(&argptr) {
        return -14;
    }
    // 域门: act7 = BOX_CMD (per-provider 域宽与模型同构; 域 0 走全局 exec 槽)。
    if !crate::capability::exec_authorized(7) {
        aud_box_call(verb, -1);
        serial::write_str("box  : deny (no act7 grant) verb=");
        crate::syscall::debug_dec(verb);
        serial::write_line("");
        return -1;
    }
    unsafe {
        PROV_TOTAL += 1;
        BOX_BUF_N = 0;
        BOX_ARG_N = arglen as usize;
        for i in 0..arglen as usize {
            BOX_ARG[i] = ((argptr as *const u8).add(i)).read_volatile();
        }
        if BOX_ARG_N < BOX_ARG_MAX {
            BOX_ARG[BOX_ARG_N] = 0;
        }
    }
    let seq = unsafe {
        BOX_SEQ = BOX_SEQ.wrapping_add(1);
        BOX_SEQ
    };
    // payload = verb(1B) + arg
    let mut payload = [0u8; 1 + BOX_ARG_MAX];
    payload[0] = verb as u8;
    unsafe {
        for i in 0..arglen as usize {
            payload[1 + i] = BOX_ARG[i];
        }
    }
    box_write_frame(seq, &payload[..1 + arglen as usize]);
    box_trigger(seq, 1 + arglen);

    // ---- 等待应答 (同 wait_rsp 模式: sti + COM2 行组装, TTL 超时 = 缺席) ----
    unsafe {
        core::arch::asm!("sti", options(nomem, nostack, preserves_flags));
    }
    let t0 = crate::interrupts::ticks();
    let mut line = [0u8; BOX_LINE_MAX];
    let mut ln = 0usize;
    let mut done = false;
    let mut ended = false;
    let mut gate_bad: u8 = 0;
    while !done && crate::interrupts::ticks().wrapping_sub(t0) < BOX_TTL_TICKS {
        if let Some(b) = serial::ser2_poll() {
            if b == b'\n' {
                let l = &line[..ln];
                // RSP / DATA / END 判定
                if l.starts_with(b"FJBOX:RSP ") {
                    if let Some(k) = tok(l, 2) {
                        let st = parse_num(&l[k.0..k.0 + k.1]).unwrap_or(0);
                        if st == 1 {
                            unsafe { PROV_UP = 1; }
                        }
                    }
                } else if l.starts_with(b"FJBOX:DATA ") {
                    // 行: FJBOX:DATA <seq> <off> <n> <text>
                    if let Some(k) = tok(l, 2) {
                        if let Some(k2) = tok(l, 3) {
                            if let Some(k3) = tok(l, 4) {
                                let off = parse_num(&l[k.0..k.0 + k.1]).unwrap_or(usize::MAX as u64);
                                let n = parse_num(&l[k2.0..k2.0 + k2.1]).unwrap_or(0) as usize;
                                // text = token4 到行尾 (产物含空格, 不截断)
                                let mut s = k3.0;
                                while s < ln && (l[s] == b' ' || l[s] == b'\t') {
                                    s += 1;
                                }
                                let d = &l[s..ln];
                                if off as usize == unsafe { BOX_BUF_N }
                                    && n <= BOX_DATA_MAX
                                    && n == d.len()
                                    && unsafe { BOX_BUF_N } + n <= BOX_BUF_MAX
                                    && ascii_ok(d)
                                    && !exec_magic(d)
                                {
                                    unsafe {
                                        for (i, &c) in d.iter().enumerate() {
                                            BOX_BUF[BOX_BUF_N + i] = c;
                                        }
                                        BOX_BUF_N += n;
                                    }
                                } else {
                                    gate_bad = 1;
                                }
                            }
                        }
                    }
                } else if l.starts_with(b"FJBOX:END ") {
                    ended = true;
                    done = true;
                }
                ln = 0;
            } else if ln < BOX_LINE_MAX {
                line[ln] = b;
                ln += 1;
            }
        }
    }
    if !done && !ended {
        // 超时 = 供应商缺席声明 (不是错误码: 状态 + 审计, 调用方走降级)
        unsafe {
            PROV_UP = 0;
            PROV_LAST_RC = -4;
        }
        aud_box_call(verb, -4);
        crate::ai::fujo_qual_feed(7, 0); // 列1 履约败
        return -4;
    }
    if gate_bad != 0 || unsafe { BOX_BUF_N } == 0 {
        // 检疫拒收 (A1'): 产物未过门, 不进任何工具链; 审计 + 列1 记败
        aud_box_gate(verb, 1);
        unsafe {
            BOX_BUF_N = 0;
            PROV_LAST_RC = -2;
        }
        crate::ai::fujo_qual_feed(7, 0);
        return -2;
    }
    if !schema_ok(verb, unsafe { &BOX_BUF[..BOX_BUF_N] }) {
        // 契约违约 (adapter): 文本可读但 schema 不符 —— 降权信号, 非健康
        aud_box_gate(verb, 2);
        unsafe {
            PROV_LAST_RC = -3;
        }
        crate::ai::fujo_qual_feed(7, 1); // 契约履约 (传输 OK)
        crate::ai::fujo_qual_feed(8, 0); // 列2a 谓词败
        return -3;
    }
    // 通过: 列1 履约 + 列2a 谓词 (机器可判定) 双记; 域宽 = f(质量) 由 dom_admit 消费
    aud_box_gate(verb, 0);
    unsafe {
        PROV_LAST_RC = 0;
        PROV_SCHEMA += 1;
    }
    prov_note(1);
    crate::ai::fujo_qual_feed(7, 1);
    crate::ai::fujo_qual_feed(8, 1);
    aud_box_call(verb, 0);
    0
}

/// 0x8317 box_stat(verb, out): out = u64×4 {prov_up, prov_hit, prov_total, prov_schema}。
#[no_mangle]
pub extern "C" fn fujo_box_stat(verb: u64, out: u64) -> i64 {
    if verb == 0 || verb > BOX_VERB_MAX || !(0x400000..0xC00000).contains(&out) {
        return -22;
    }
    unsafe {
        let b = out as *mut u64;
        b.add(0).write(PROV_UP);
        b.add(1).write(PROV_HIT);
        b.add(2).write(PROV_TOTAL);
        b.add(3).write(PROV_SCHEMA);
    }
    0
}

/// 0x8318 box_result(ptr, cap): 产物拷入用户区 (检疫后只读; 不可重入写)。
#[no_mangle]
pub extern "C" fn fujo_box_result(ptr: u64, cap: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&ptr) {
        return -14;
    }
    let n = (cap as usize).min(unsafe { BOX_BUF_N });
    unsafe {
        for i in 0..n {
            ((ptr as *mut u8).add(i)).write_volatile(BOX_BUF[i]);
        }
    }
    n as i64
}
