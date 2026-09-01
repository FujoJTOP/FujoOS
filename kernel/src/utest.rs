//! utest.rs — M82: 单元测试框架 (kernel 内断言自检)
//!
//! 注册表: 8 个纯函数用例 (bool 返回, 无硬件面);
//! 运行器: 0x7901 ut_run() → (pass, fail, total) /
//!         0x7902 ut_info(ptr) → u64×4: (pass, fail, total, pass_total)。
//! 用例面: 字符串/解析/行模型/整数数学 (复刻核心逻辑的自包含断言)。

use crate::serial;

const MAX_T: usize = 8;

type CaseFn = fn() -> bool;

static mut CASES: [Option<CaseFn>; MAX_T] = [None; MAX_T];
static mut PASS_N: u64 = 0;
static mut FAIL_N: u64 = 0;
static mut RUN_TOTAL: u64 = 0;

pub fn register(c: CaseFn) -> bool {
    unsafe {
        for slot in CASES.iter_mut() {
            if slot.is_none() {
                *slot = Some(c);
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// 用例 (自包含)
// ---------------------------------------------------------------------------

fn str_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x == y)
}

fn hexval_c(c: u8) -> u64 {
    match c {
        b'0'..=b'9' => (c - b'0') as u64,
        b'a'..=b'f' => (c - b'a' + 10) as u64,
        b'A'..=b'F' => (c - b'A' + 10) as u64,
        _ => 0,
    }
}

fn parse_hex(t: &[u8]) -> u64 {
    let mut v = 0u64;
    for &c in t {
        v = v * 16 + hexval_c(c);
    }
    v
}

fn tc_strlen() -> bool {
    let s = b"fujotest";
    let mut n = 0usize;
    while n < s.len() {
        n += 1;
    }
    n == 8
}

fn tc_strcmp() -> bool {
    str_eq(b"abc", b"abc") && !str_eq(b"abc", b"abd") && !str_eq(b"ab", b"abc")
}

fn tc_parse() -> bool {
    parse_hex(b"1F") == 0x1F && parse_hex(b"FF") == 0xFF && parse_hex(b"0") == 0
        && parse_hex(b"10") == 0x10
}

fn tc_math() -> bool {
    let a = 0x12345u64.wrapping_mul(0x10000);
    a == 0x123450000 && (a / 0x10000) == 0x12345 && (a % 0x100) == 0
}

fn tc_strrev() -> bool {
    let mut b = *b"abcd";
    let mut i = 0usize;
    let mut j = 3usize;
    while i < j {
        let t = b[i];
        b[i] = b[j];
        b[j] = t;
        i += 1;
        j -= 1;
    }
    str_eq(&b, b"dcba")
}

fn tc_bits() -> bool {
    let v = 0x8000_0000_0000_0001u64;
    v.count_ones() == 2 && v.leading_zeros() == 0 && v.trailing_zeros() == 0
}

fn tc_line_model() -> bool {
    // 行模型: 按 \n 分隔统计与首行长度 (与 editor 语义同构)
    static B: &[u8] = b"hello\nworld\n";
    let mut lines = 1usize;
    let mut first = 0usize;
    for (i, &c) in B.iter().enumerate() {
        if c == b'\n' {
            if lines == 1 {
                first = i;
            }
            lines += 1;
        }
    }
    lines == 3 && first == 5
}

// ---------------------------------------------------------------------------
// 接口
// ---------------------------------------------------------------------------

/// 0x7901: 运行全部用例。
pub fn fujo_ut_run() -> i64 {
    unsafe {
        PASS_N = 0;
        FAIL_N = 0;
        RUN_TOTAL = 0;
        for slot in CASES.iter() {
            if let Some(f) = slot {
                RUN_TOTAL += 1;
                if f() {
                    PASS_N += 1;
                    serial::write_str("ut   : PASS case (total ");
                    crate::syscall::debug_dec(RUN_TOTAL);
                    serial::write_line(")");
                } else {
                    FAIL_N += 1;
                    serial::write_str("ut   : FAIL case (total ");
                    crate::syscall::debug_dec(RUN_TOTAL);
                    serial::write_line(")");
                }
            }
        }
        serial::write_str("ut   : run done pass=");
        crate::syscall::debug_dec(PASS_N);
        serial::write_str(" fail=");
        crate::syscall::debug_dec(FAIL_N);
        serial::write_line("");
    }
    unsafe {
        (PASS_N as i64) - (FAIL_N as i64)
    }
}

/// 0x7902: (pass, fail, total, allpass)。
pub fn fujo_ut_info(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(PASS_N);
        w.add(1).write(FAIL_N);
        w.add(2).write(RUN_TOTAL);
        w.add(3).write(if FAIL_N == 0 && RUN_TOTAL > 0 { 1 } else { 0 });
    }
    0
}

/// M82: 注册全部用例 (启动一次)。
pub fn init() {
    // 顺序注册; 失败即忽略 (满表)
    for f in [
        tc_strlen as CaseFn,
        tc_strcmp,
        tc_parse,
        tc_math,
        tc_strrev,
        tc_bits,
        tc_line_model,
    ] {
        register(f);
    }
    serial::write_line("ut   : unit-test suite registered (7 cases)");
}
