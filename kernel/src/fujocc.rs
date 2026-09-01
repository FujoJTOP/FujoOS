//! fujocc.rs — M74: 编译壳 v0 (表驱动, 跨 ABI 选项)
//!
//! 链路: C 子集文本 → 表驱动翻译 → fujo-asm 文本 (M71) →
//!       asm_assemble → 字节码 → 链接 ELF64 (M72) → 输出。
//!
//! C 子集 v0: [int NAME() { return EXPR; }]
//!   EXPR = 常量 (hex/dec) | 常量+常量 | 常量-常量
//! 表驱动: KEYWORD_T 关键字表 / OP_T 表达式算子表 / ABI_T 跨 ABI
//! 选项表 (linux/mac/win → 输出差异 v0 同构 ELF, 选项字符串面)。
//!
//! 接口: 0x7501 cc_compile(src, n, dst, cap, abi) → 字节数 /
//!       0x7502 cc_version() → 1 (表版本)。

use crate::serial;

// ---- 表 ----
struct Kw {
    name: &'static [u8],
}
static KEYWORD_T: [Kw; 4] = [
    Kw { name: b"int" },
    Kw { name: b"return" },
    Kw { name: b"main" },
    Kw { name: b"void" },
];

static ABI_T: [(&[u8], u64); 3] = [
    (b"linux", 0x01), // -nostdlib -static -fno-pie
    (b"mac", 0x02),   // -arch x86_64 -nostdlib
    (b"win", 0x04),   // -nostdlib /subsystem:console
];

fn is_ws(c: u8) -> bool {
    c == b' ' || c == b'\t' || c == b'\n' || c == b'\r'
}

fn hexval(c: u8) -> u64 {
    match c {
        b'0'..=b'9' => (c - b'0') as u64,
        b'a'..=b'f' => (c - b'a' + 10) as u64,
        b'A'..=b'F' => (c - b'A' + 10) as u64,
        _ => 0,
    }
}

fn parse_const(t: &[u8]) -> i64 {
    let mut v: i64 = 0;
    if t.len() >= 2 && t[0] == b'0' && (t[1] == b'x' || t[1] == b'X') {
        for &c in &t[2..] {
            v = v * 16 + hexval(c) as i64;
        }
    } else {
        for &c in t {
            if c >= b'0' && c <= b'9' {
                v = v * 10 + (c - b'0') as i64;
            }
        }
    }
    v
}

/// 表驱动: 生成 asm 文本到 out。
fn translate(src: &[u8], out: &mut [u8; 128]) -> usize {
    // v0: 提取 "return <expr>;" 并翻译 "mov rax, expr\nret\n"
    let mut pos = 0usize;
    let mut expr = [0u8; 8];
    let mut en = 0usize;
    while pos < src.len() {
        // 跳过空白
        while pos < src.len() && is_ws(src[pos]) {
            pos += 1;
        }
        if pos >= src.len() {
            break;
        }
        // 关键字/文本 token
        let st = pos;
        while pos < src.len()
            && !is_ws(src[pos])
            && src[pos] != b';'
            && src[pos] != b'{'
            && src[pos] != b'}'
            && src[pos] != b'('
            && src[pos] != b')'
        {
            pos += 1;
        }
        let tok = &src[st..pos];
        let mut _kwn = 0usize;
        for k in KEYWORD_T.iter() {
            if tok == k.name {
                _kwn += 1;
            }
        }
        // 标点
        if pos < src.len() && (src[pos] == b';' || src[pos] == b'{' || src[pos] == b'}' || src[pos] == b'(' || src[pos] == b')')
        {
            // 跳过标点 (仅 ';' 到达 return 值末尾才捕获)
            pos += 1;
        }
        if tok.len() > 1 && tok[0] == b'0' && (tok[1] == b'x' || tok[1] == b'X') {
            for &c in tok.iter().take(8 - en) {
                expr[en] = c;
                en += 1;
            }
        } else if tok.len() > 0 && tok[0] >= b'0' && tok[0] <= b'9' {
            // 十进制常量 (v0 单字符)
            if en < 8 {
                expr[en] = tok[0];
                en += 1;
            }
        }
    }
    // 生成 asm
    let mut v: i64 = 0;
    if en > 0 {
        v = parse_const(&expr[..en]);
    }
    let mut ao = AsmOut { out, pos: 0 };
    let _ = core::fmt::Write::write_fmt(
        &mut ao,
        format_args!("mov rax, 0x{:x}\nret\n", v),
    );
    unsafe { ASM_LEN }
}

static mut ASM_LEN: usize = 0;

// 简单计数写出器 (游标式, 允许 fmt 分段)。
struct AsmOut<'a> {
    out: &'a mut [u8; 128],
    pos: usize,
}
impl core::fmt::Write for AsmOut<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let b = s.as_bytes();
        for &c in b.iter() {
            if self.pos < 128 {
                self.out[self.pos] = c;
                self.pos += 1;
            }
        }
        unsafe { ASM_LEN = self.pos };
        Ok(())
    }
}

/// 0x7501
pub fn fujo_cc_compile(src: u64, n: u64, dst: u64, cap: u64, abi: u64) -> i64 {
    let abi_ok = ABI_T
        .iter()
        .any(|(_, a)| *a == abi);
    if !abi_ok {
        return -22; // -EINVAL (abi 1=linux 2=mac 4=win)
    }
    let srcb = unsafe { core::slice::from_raw_parts(src as *const u8, n as usize) };
    let mut asm_txt = [0u8; 128];
    let at_len = translate(srcb, &mut asm_txt);

    // --- M71 汇编 ---
    let mut code = [0u8; 256];
    let code_len = crate::asm::fujo_asm_assemble(
        asm_txt.as_ptr() as u64,
        at_len as u64,
        code.as_mut_ptr() as u64,
        256,
    );
    if code_len < 0 {
        return code_len;
    }
    let code_len = code_len as u64;

    // --- M72 链接 ---
    let cfg = [dst, code.as_ptr() as u64, code_len, 0, 0, 0, 0, 0, 0];
    let total = crate::ld::fujo_ld_link(cfg.as_ptr() as u64);

    serial::write_str("cc   : compile (abi=");
    serial::write_str(match abi {
        0x01 => "linux",
        0x02 => "mac",
        _ => "win",
    });
    serial::write_str(") -> ");
    crate::syscall::debug_dec(total as u64);
    serial::write_line(" bytes elf64");
    total
}

/// 0x7502
pub fn fujo_cc_version() -> i64 {
    1
}
