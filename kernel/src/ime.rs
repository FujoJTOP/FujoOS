//! ime.rs — 输入法框架骨架 v0 (M40)
//!
//! 骨架语义 (演示集音码): 拼音串 -> 汉字候选 的线性表解析 + 候选窗口
//! (用户态 GUI 展示: m40 demo 直接打印) + 提交缓冲 (内核 HANZI_OUT)。
//! fujo 原生: 0x5701 ime_begin() / 0x5702 ime_key(ch, str?) 逐字符输入
//!            0x5703 ime_candidates(ptr, n) 写候选 u32×n
//!            0x5704 ime_commit(i) 提交第 i 候选 (写 HANZI_OUT)
//!            0x5705 ime_reset()
//! 后续 (M48): 键盘流钩子 + 悬浮候选窗 + 码表文件 (fujopack 资源)。

use crate::serial;

/// 演示集: 拼音 -> 候选汉字 (最多 4 个)
static TABLE: &[(&str, &[&str])] = &[
    ("nihao", &["你好", "拟好"]),
    ("beijing", &["北京"]),
    ("zhongguo", &["中国", "众国"]),
    ("wo", &["我", "窝"]),
    ("ai", &["爱", "碍", "艾"]),
    ("meng", &["梦", "猛"]),
    ("he", &["和", "河", "何"]),
];

static mut IM_BUF: [u8; 16] = [0; 16];
static mut IM_LEN: usize = 0;
/// 提交缓冲 (UTF-8, 供 GUI/终端打印)。
static mut HANZI_OUT: [u8; 32] = [0; 32];

fn matches(buf: &[u8]) -> Option<&'static [&'static str]> {
    for (py, cands) in TABLE {
        if py.as_bytes() == buf {
            return Some(cands);
        }
    }
    None
}

/// 0x5701: begin (清输入缓冲)。
pub fn fujo_ime_begin() -> i64 {
    unsafe {
        IM_LEN = 0;
        for i in 0..16 {
            IM_BUF[i] = 0;
        }
    }
    0
}

/// 0x5702: ime_key(ch) — 追加拼音字符 ('a'..'z'); 若解析出候选返回 1。
pub fn fujo_ime_key(ch: u64) -> i64 {
    unsafe {
        if ch >= b'a' as u64 && ch <= b'z' as u64 && IM_LEN < 15 {
            IM_BUF[IM_LEN] = (ch as u8).to_ascii_lowercase();
            IM_LEN += 1;
            if matches(&IM_BUF[..IM_LEN]).is_some() {
                return 1;
            }
        }
        0
    }
}

/// 0x5703: ime_candidates(ptr, n) — 写候选字串地址 u32×n, 返回候选数。
pub fn fujo_ime_candidates(ptr: u64, n: u64) -> i64 {
    unsafe {
        let buf = &IM_BUF[..IM_LEN];
        match matches(buf) {
            Some(cands) => {
                // 候选字串放内核静态 (演示: 直接引用表内静态串地址)
                let count = cands.len().min(n as usize);
                for i in 0..count {
                    let p = cands[i].as_ptr() as u64;
                    ((ptr as *mut u64).add(i)).write(p);
                    // 拷贝内容 (用户可见区外的静态串由用户读时经用户指针检查?
                    // v0: 串地址在内核 .rodata, 用户不可读! -> 拷贝到 0x7E2000 用户区)
                }
                // 拷贝到用户区 0x7E2000 (简表, 每候选 ≤16B)
                let mut off = 0x7E2000u64;
                for i in 0..count {
                    let bytes = cands[i].as_bytes();
                    ((ptr as *mut u64).add(i)).write(off);
                    for (k, b) in bytes.iter().enumerate().take(15) {
                        ((off + k as u64) as *mut u8).write(*b);
                    }
                    ((off + bytes.len().min(15) as u64) as *mut u8).write(0);
                    off += 16;
                }
                count as i64
            }
            None => 0,
        }
    }
}

/// 0x5704: ime_commit(i) — 提交候选 i -> HANZI_OUT; 返回长度。
pub fn fujo_ime_commit(idx: u64) -> i64 {
    unsafe {
        let buf = &IM_BUF[..IM_LEN];
        match matches(buf) {
            Some(cands) if (idx as usize) < cands.len() => {
                let bytes = cands[idx as usize].as_bytes();
                let n = bytes.len().min(31);
                for k in 0..n {
                    HANZI_OUT[k] = bytes[k];
                }
                HANZI_OUT[n] = 0;
                0
            }
            _ => -22,
        }
    }
}

/// 0x5705: ime_reset。
pub fn fujo_ime_reset() -> i64 {
    unsafe {
        IM_LEN = 0;
    }
    0
}

/// 读提交缓冲 (用户读 0x5704 后经字符串打印用; 提供拷贝原语)。
pub fn fujo_ime_out(ptr: u64) -> i64 {
    unsafe {
        let mut n = 0usize;
        while n < 31 && HANZI_OUT[n] != 0 {
            n += 1;
        }
        for k in 0..=n {
            ((ptr + k as u64) as *mut u8).write(HANZI_OUT[k]);
        }
        serial::write_str("ime  : committed '");
        let s = core::str::from_utf8(&HANZI_OUT[..n]).unwrap_or("?");
        serial::write_str(s);
        serial::write_line("'");
        n as i64
    }
}
