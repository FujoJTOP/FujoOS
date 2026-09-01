//! infer.rs — M93: 推理执行器插槽 (宿主链路 → 内核量化评估)
//!
//! 执行器模式: 0=宿主链路 (COM2, 模型服务) 1=本地内核评估;
//! 本地 v0: 确定性响应 `fujo-infer-local: recv=N tokens intent=X`
//! (X 取规则意图); 计时 (跨 tick 计量 ms 级)。
//! 接口: 0x8301 infer_run(ptr,len,out,cap) → 长度 /
//!       0x8302 infer_slot(ptr) → (mode, calls, tokens, last_ms) /
//!       0x8303 infer_set(mode)。

use crate::serial;

static mut MODE: u64 = 1; // 默认本地
static mut CALLS: u64 = 0;
static mut TOKENS: u64 = 0;
static mut LAST_MS: u64 = 0;

/// 0x8303
pub fn fujo_infer_set(m: u64) -> i64 {
    unsafe {
        MODE = m.min(1);
        serial::write_str("infer: mode=");
        serial::write_str(if MODE == 0 { "host-link" } else { "local-kernel" });
        serial::write_line("");
    }
    0
}

fn rule_intent(lower: &str) -> i64 {
    if lower.contains("run") || lower.contains("exec") {
        1
    } else if lower.contains("open") {
        3
    } else if lower.contains("hello") || lower.contains("info") || lower.contains("?") || lower.contains("status") {
        2
    } else {
        0
    }
}

/// 0x8301: 执行推理请求。
pub fn fujo_infer_run(ptr: u64, len: u64, out: u64, cap: u64) -> i64 {
    let len = len.min(256) as usize;
    let src = ptr as *const u8;
    let mut text = [0u8; 256];
    unsafe {
        for i in 0..len {
            text[i] = src.add(i).read();
        }
    }
    let s = core::str::from_utf8(&text[..len]).unwrap_or("");
    let mut lower = [0u8; 256];
    for (i, &b) in s.as_bytes().iter().enumerate() {
        lower[i] = if b.is_ascii_uppercase() { b + 32 } else { b };
    }
    let it = rule_intent(core::str::from_utf8(&lower[..s.len()]).unwrap_or(""));

    unsafe {
        CALLS += 1;
        TOKENS += len as u64;
        let t0 = crate::interrupts::ticks();
        // 本地执行 (确定性; 宿主链路由 COM2 模型服务, v0 同面)
        let resp = match MODE {
            0 => b"fujo-infer-host: (link relay stub)\n".as_slice(),
            _ => b"fujo-infer-local: recv=%L tokens intent=%I\n"
                .as_slice(),
        };
        let _ = t0;
        LAST_MS = crate::interrupts::ticks() - t0;
        // 组装响应
        let b = out as *mut u8;
        let cap = cap as usize;
        let mut pos = 0usize;
        let head = b"fujo-infer-local: recv=";
        for &c in head.iter() {
            if pos < cap {
                b.add(pos).write(c);
                pos += 1;
            }
        }
        // 数字
        let mut num = [0u8; 20];
        let mut ni = 20usize;
        let mut x = len as u64;
        if x == 0 {
            if pos < cap {
                b.add(pos).write(b'0');
                pos += 1;
            }
        } else {
            while x > 0 {
                ni -= 1;
                num[ni] = b'0' + (x % 10) as u8;
                x /= 10;
            }
            for i in ni..20 {
                if pos < cap {
                    b.add(pos).write(num[i]);
                    pos += 1;
                }
            }
        }
        let tail1 = b" tokens intent=";
        for &c in tail1.iter() {
            if pos < cap {
                b.add(pos).write(c);
                pos += 1;
            }
        }
        let itn = match it {
            1 => b"RUN\n".as_slice(),
            3 => b"OPEN\n".as_slice(),
            2 => b"QUERY\n".as_slice(),
            _ => b"UNKNOWN\n".as_slice(),
        };
        for &c in itn.iter() {
            if pos < cap {
                b.add(pos).write(c);
                pos += 1;
            }
        }
        serial::write_str("infer: run len=");
        crate::syscall::debug_dec(len as u64);
        serial::write_str(" intent=");
        crate::syscall::debug_dec(it as u64);
        serial::write_line("");
        pos as i64
    }
}

/// 0x8302: (mode, calls, tokens, last_ms)。
pub fn fujo_infer_slot(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(MODE);
        w.add(1).write(CALLS);
        w.add(2).write(TOKENS);
        w.add(3).write(LAST_MS);
    }
    0
}
