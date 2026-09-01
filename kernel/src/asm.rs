//! asm.rs — M71: 系统内汇编器 (最小 .s 子集, 两遍)
//!
//! 支持: 指令 nop/ret/int3/syscall/mov(r64,imm64|r64)/add/sub(r64,imm8|r64)/
//! xor/inc/dec(r64)/push/pop(r64)/cmp/je/jne/jmp(rel32)/label:/伪指令
//! .byte/.word/.quad/.text。寄存器 rax=0 rcx=1 rdx=2 rbx=3 rsp=4 rbp=5
//! rsi=6 rdi=7。立即: 0x.. / 十进制 / $ 前缀。
//!
//! 接口: 0x7001 asm_assemble(src,n,dst,cap) → 字节数 (负=err) /
//!       0x7002 asm_verify(ptr,n) → 解码指令数 (遇 ret 停)。

use crate::serial;

const MAX_INSNS: usize = 64;
const MAX_LABELS: usize = 16;

static mut INSNS: [u64; MAX_INSNS] = [0; MAX_INSNS]; // 字节 off (pass1)
static mut INSNS_N: usize = 0;
static mut LABEL_AT: [u64; MAX_LABELS] = [0; MAX_LABELS]; // 字节地址
static mut LABEL_N: usize = 0;

fn is_space(c: u8) -> bool {
    c == b' ' || c == b'\t' || c == b'\r'
}

fn parse_num(tok: &[u8]) -> u64 {
    let mut t = tok;
    if !t.is_empty() && t[0] == b'$' {
        t = &t[1..];
    }
    let mut neg = false;
    if !t.is_empty() && t[0] == b'-' {
        neg = true;
        t = &t[1..];
    }
    let mut v: u64 = 0;
    if t.len() >= 2 && t[0] == b'0' && (t[1] == b'x' || t[1] == b'X') {
        for &c in &t[2..] {
            let d = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => 0,
            };
            v = v * 16 + d as u64;
        }
    } else {
        for &c in t {
            if c >= b'0' && c <= b'9' {
                v = v * 10 + (c - b'0') as u64;
            }
        }
    }
    if neg {
        (0u64).wrapping_sub(v)
    } else {
        v
    }
}

fn reg_of(tok: &[u8]) -> Option<u8> {
    // 要求 tok 前缀 "r" 或 "e" (rax/rcx/...) — v0 仅 64 位名
    let name = tok;
    let n = name.len();
    if n >= 3 {
        let lo = name[n - 1];
        let idx = match lo {
            b'x' => 0, // rax
            _ => 9,    // not sure
        };
        let _ = idx;
    }
    // 直接表匹配
    let m = match name {
        b"rax" => 0,
        b"rcx" => 1,
        b"rdx" => 2,
        b"rbx" => 3,
        b"rsp" => 4,
        b"rbp" => 5,
        b"rsi" => 6,
        b"rdi" => 7,
        _ => 99,
    };
    if m < 8 {
        Some(m)
    } else {
        None
    }
}

/// 编码一条指令到 buf; 返回字节数 (0 = 不支持)。 label 引用经
/// pass1 已填 LABEL_AT (jmp/je/jne 用 rel32)。
fn emit_insn(out: *mut u8, op: &[u8], args: &[&[u8]], pc: u64, bytes: &mut usize) -> bool {
    let o = op;
    if o == b"nop" {
        unsafe { (out as *mut u8).write(0x90) };
        *bytes += 1;
        return true;
    }
    if o == b"ret" {
        unsafe { (out as *mut u8).write(0xC3) };
        *bytes += 1;
        return true;
    }
    if o == b"int3" {
        unsafe { (out as *mut u8).write(0xCC) };
        *bytes += 1;
        return true;
    }
    if o == b"syscall" {
        unsafe {
            (out as *mut u8).write(0x0F);
            (out as *mut u8).add(1).write(0x05);
        }
        *bytes += 2;
        return true;
    }
    if o == b"mov" && args.len() == 2 {
        let rd = reg_of(args[0]);
        let c32 = args[1].len() >= 2 && args[1][0] == b'0' && (args[1][1] == b'x' || args[1][1] == b'X')
            || (args[1][0] >= b'0' && args[1][0] <= b'9');
        let is_imm = c32 || args[1][0] == b'$' || args[1][0] == b'-';
        if let Some(r) = rd {
            if is_imm {
                // mov r64, imm64: 48 B8+r [imm64]
                unsafe {
                    (out as *mut u8).write(0x48);
                    (out as *mut u8).add(1).write(0xB8 + r);
                    let v = parse_num(args[1]);
                    for i in 0..8 {
                        (out as *mut u8).add(2 + i).write((v >> (8 * i)) as u8);
                    }
                }
                *bytes += 10;
                return true;
            } else if let Some(rs) = reg_of(args[1]) {
                // mov r64, r64: 48 89 /r
                unsafe {
                    (out as *mut u8).write(0x48);
                    (out as *mut u8).add(1).write(0x89);
                    (out as *mut u8).add(2).write(0xC0 + r + rs * 8);
                }
                *bytes += 3;
                return true;
            }
        }
        return false;
    }
    // add/sub/xor/cmp: r64, imm8 (83 /0..7) 或 r64, r64 (01/29/31/39)
    let two_byte_op = match o {
        b"add" => Some((0x83u8, 0x00u8, 0x01u8)),
        b"sub" => Some((0x83u8, 0x00u8, 0x29u8)),
        b"xor" => Some((0x83u8, 0x00u8, 0x31u8)),
        b"cmp" => Some((0x83u8, 0x00u8, 0x39u8)),
        _ => None,
    };
    if let Some((_, _, rr_op)) = two_byte_op {
        if args.len() == 2 {
            if let Some(r) = reg_of(args[0]) {
                if let Some(rs) = reg_of(args[1]) {
                    unsafe {
                        (out as *mut u8).write(0x48);
                        (out as *mut u8).add(1).write(rr_op);
                        (out as *mut u8).add(2).write(0xC0 + r + rs * 8);
                    }
                    *bytes += 3;
                    return true;
                } else {
                    // imm8 形式: 48 83 /digit(0..7) imm8
                    let d = match o[0] {
                        b'a' => 0u8,
                        b's' => 5u8,
                        b'x' => 6u8,
                        b'c' => 7u8,
                        _ => 0,
                    };
                    unsafe {
                        (out as *mut u8).write(0x48);
                        (out as *mut u8).add(1).write(0x83);
                        (out as *mut u8).add(2).write(0xC0 + r + d * 8);
                        (out as *mut u8).add(3).write(parse_num(args[1]) as u8);
                    }
                    *bytes += 4;
                    return true;
                }
            }
        }
        return false;
    }
    if (o == b"inc" || o == b"dec") && args.len() == 1 {
        if let Some(r) = reg_of(args[0]) {
            unsafe {
                (out as *mut u8).write(0x48);
                (out as *mut u8).add(1).write(0xFF);
                (out as *mut u8).add(2).write(if o[0] == b'i' { 0xC0 + r } else { 0xC8 + r });
            }
            *bytes += 3;
            return true;
        }
    }
    if (o == b"push" || o == b"pop") && args.len() == 1 {
        if let Some(r) = reg_of(args[0]) {
            unsafe {
                (out as *mut u8).write(if o[0] == b'p' && o[2] == b's' { 0x50 + r } else { 0x58 + r });
            }
            *bytes += 1;
            return true;
        }
    }
    // 跳转: jmp/je/jne rel32
    let br = match o {
        b"jmp" => Some(0xE9u8),
        b"je" => Some(0x84u8),
        b"jne" => Some(0x85u8),
        _ => None,
    };
    if let Some(kind) = br {
        if args.len() == 1 {
            // 目标 label: 在 LABEL_AT 中查找
            let mut target: Option<i64> = None;
            let name = args[0];
            let mut num: i64 = -1;
            if name.len() >= 2 && name[0] == b'L' {
                let mut v: i64 = 0;
                for &c in &name[1..] {
                    if c >= b'0' && c <= b'9' {
                        v = v * 10 + (c - b'0') as i64;
                    }
                }
                num = v;
            }
            unsafe {
                if num >= 0 && (num as usize) < MAX_LABELS && (num as usize) < LABEL_N {
                    let size_after = if kind == 0xE9 { 5 } else { 6 };
                    target = Some(LABEL_AT[num as usize] as i64 - (pc + size_after) as i64);
                }
            }
            // pass1 (label 未填) 或未知 label: 长度固定, rel 填 0
            let rel = target.unwrap_or(0) as u32;
            unsafe {
                if kind == 0xE9 {
                    // E9 [rel32]
                    (out as *mut u8).write(0xE9);
                    for i in 0..4 {
                        (out as *mut u8).add(1 + i).write((rel >> (8 * i)) as u8);
                    }
                    *bytes += 5;
                } else {
                    // 0F 8x [rel32]
                    (out as *mut u8).write(0x0F);
                    (out as *mut u8).add(1).write(kind);
                    for i in 0..4 {
                        (out as *mut u8).add(2 + i).write((rel >> (8 * i)) as u8);
                    }
                    *bytes += 6;
                }
            }
            return true;
        }
    }
    false
}

/// 0x7001: 汇编 (两遍: 统计 label → 生成)。
/// 源码格式: 每行 `[label:] [insn [args...]]`, `#` 或 `;` 注释, `#` 行内。
/// label 名 = L0..L15 (跳转目标用同名)。
pub fn fujo_asm_assemble(src: u64, src_n: u64, dst: u64, cap: u64) -> i64 {
    unsafe {
        let sbuf = core::slice::from_raw_parts(src as *const u8, src_n as usize);
        let mut label_off = [0u64; MAX_LABELS];
        let mut labels_seen = [false; MAX_LABELS];
        let mut cur_label: i64 = -1;

        // ---- pass 1: 扫描 label 地址 ----
        // 按行扫描 (简化: 每行一个指令/label)
        let mut pc: u64 = 0;
        let mut pos = 0usize;
        while pos < sbuf.len() {
            let line_end = sbuf[pos..]
                .iter()
                .position(|&c| c == b'\n')
                .map(|i| pos + i)
                .unwrap_or(sbuf.len());
            let mut line = &sbuf[pos..line_end];
            // 去注释
            if let Some(h) = line.iter().position(|&c| c == b'#' || c == b';') {
                line = &line[..h];
            }
            let mut toks: [&[u8]; 4] = [&[]; 4];
            let mut tn = 0usize;
            let mut i = 0usize;
            while i < line.len() && tn < 4 {
                while i < line.len() && is_space(line[i]) {
                    i += 1;
                }
                if i >= line.len() {
                    break;
                }
                let st = i;
                while i < line.len() && !is_space(line[i]) {
                    i += 1;
                }
                let mut tk = &line[st..i];
                if tk.len() >= 1 && tk[tk.len() - 1] == b',' {
                    tk = &tk[..tk.len() - 1];
                }
                toks[tn] = tk;
                tn += 1;
            }
            if tn == 0 {
                pos = line_end + 1;
                continue;
            }
            // label 识别: toks[0] 以 ':' 结尾
            let mut insn_toks = toks;
            let mut insn_n = tn;
            if tn >= 1 && toks[0].len() >= 2 && toks[0][toks[0].len() - 1] == b':' {
                let nm = &toks[0][..toks[0].len() - 1];
                if nm.len() >= 2 && nm[0] == b'L' {
                    let mut v: i64 = 0;
                    for &c in &nm[1..] {
                        if c >= b'0' && c <= b'9' {
                            v = v * 10 + (c - b'0') as i64;
                        }
                    }
                    cur_label = v;
                }
                // 剩余 token 后移
                for k in 0..(tn - 1).min(3) {
                    insn_toks[k] = toks[k + 1];
                }
                insn_n = tn - 1;
            }
            if insn_n == 0 {
                if cur_label >= 0 && (cur_label as usize) < MAX_LABELS {
                    labels_seen[cur_label as usize] = true;
                    label_off[cur_label as usize] = pc;
                    cur_label = -1;
                }
                pos = line_end + 1;
                continue;
            }
            let op = insn_toks[0];
            if op == b".byte" || op == b".quad" || op == b".word" || op == b".text" {
                // 伪指令: .byte 1B / .word 2B / .quad 8B (首个操作数)
                let sz = match op {
                    b".byte" => 1u64,
                    b".word" => 2,
                    _ => 8,
                };
                if op != b".text" && insn_n >= 2 {
                    pc += sz;
                }
            } else {
                // 指令长度估算 (与 emit 一致)
                let mut nbytes = 0usize;
                let _ = emit_insn(core::ptr::null_mut(), op, &insn_toks[1..insn_n], pc, &mut nbytes);
                if nbytes == 0 {
                    serial::write_str("asm: unsupported op '");
                    serial::write_str(core::str::from_utf8(op).unwrap_or("?"));
                    serial::write_line("'");
                    return -22; // -EINVAL
                }
                pc += nbytes as u64;
            }
            if cur_label >= 0 && (cur_label as usize) < MAX_LABELS {
                labels_seen[cur_label as usize] = true;
                label_off[cur_label as usize] = pc;
                cur_label = -1;
            }
            pos = line_end + 1;
        }

        // ---- pass 2: 生成 ----
        let mut n: u64 = 0;
        if (cap as usize) < 64 {
            return -14; // -EFAULT
        }
        let obuf = dst as *mut u8;
        LABEL_N = 0;
        for i in 0..MAX_LABELS {
            if labels_seen[i] {
                LABEL_AT[i] = label_off[i];
                if (i + 1) > LABEL_N {
                    LABEL_N = i + 1;
                }
            }
        }
        let mut pc2: u64 = 0;
        pos = 0;
        while pos < sbuf.len() {
            let line_end = sbuf[pos..]
                .iter()
                .position(|&c| c == b'\n')
                .map(|i| pos + i)
                .unwrap_or(sbuf.len());
            let raw = &sbuf[pos..line_end];
            let mut line = raw;
            if let Some(h) = line.iter().position(|&c| c == b'#' || c == b';') {
                line = &line[..h];
            }
            let mut toks: [&[u8]; 4] = [&[]; 4];
            let mut tn = 0usize;
            let mut i = 0usize;
            while i < line.len() && tn < 4 {
                while i < line.len() && is_space(line[i]) {
                    i += 1;
                }
                if i >= line.len() {
                    break;
                }
                let st = i;
                while i < line.len() && !is_space(line[i]) {
                    i += 1;
                }
                let mut tk = &line[st..i];
                if tk.len() >= 1 && tk[tk.len() - 1] == b',' {
                    tk = &tk[..tk.len() - 1];
                }
                toks[tn] = tk;
                tn += 1;
            }
            if tn == 0 {
                pos = line_end + 1;
                continue;
            }
            let mut insn_toks = toks;
            let mut insn_n = tn;
            if tn >= 1 && toks[0].len() >= 2 && toks[0][toks[0].len() - 1] == b':' {
                for k in 0..(tn - 1).min(3) {
                    insn_toks[k] = toks[k + 1];
                }
                insn_n = tn - 1;
            }
            if insn_n == 0 {
                pos = line_end + 1;
                continue;
            }
            let op = insn_toks[0];
            if op == b".text" {
                // 无输出
            } else if op == b".byte" || op == b".word" || op == b".quad" {
                if insn_n >= 2 {
                    let v = parse_num(insn_toks[1]);
                    let sz = match op {
                        b".byte" => 1,
                        b".word" => 2,
                        _ => 8,
                    };
                    for k in 0..sz {
                        obuf.add((n + k) as usize).write((v >> (8 * k)) as u8);
                    }
                    n += sz;
                    pc2 += sz;
                }
            } else {
                let mut nb = 0usize;
                if !emit_insn(obuf.add(n as usize), op, &insn_toks[1..insn_n], pc2, &mut nb) {
                    serial::write_str("asm: pass2 unsupported '");
                    serial::write_str(core::str::from_utf8(op).unwrap_or("?"));
                    serial::write_line("'");
                    return -22;
                }
                n += nb as u64;
                pc2 += nb as u64;
            }
            pos = line_end + 1;
        }
        serial::write_str("asm  : assembled ");
        crate::syscall::debug_dec(n);
        serial::write_line(" bytes");
        n as i64
    }
}

/// 0x7002: 解码校验 (计数指令直到 ret/末尾)。
pub fn fujo_asm_verify(ptr: u64, n: u64) -> i64 {
    let b = unsafe { core::slice::from_raw_parts(ptr as *const u8, n as usize) };
    let mut cnt = 0u32;
    let mut i = 0usize;
    while i < b.len() {
        let op = b[i];
        match op {
            0x90 | 0xC3 | 0xCC | 0x50..=0x57 | 0x58..=0x5F => {
                cnt += 1;
                if op == 0xC3 {
                    break;
                }
                i += 1;
            }
            0x0F if i + 1 < b.len() && b[i + 1] == 0x05 => {
                cnt += 1;
                i += 2;
            }
            0x48 => {
                // mov/add/sub/xor/cmp/inc/dec
                let m = b.get(i + 1).copied().unwrap_or(0);
                if (m & 0xF8) == 0xB8 {
                    cnt += 1;
                    i += 10;
                } else if m == 0x89 || m == 0x01 || m == 0x29 || m == 0x31 || m == 0x39 {
                    cnt += 1;
                    i += 3;
                } else if m == 0x83 {
                    cnt += 1;
                    i += 4;
                } else if m == 0xFF {
                    cnt += 1;
                    i += 3;
                } else {
                    break;
                }
            }
            0xE9 => {
                cnt += 1;
                i += 5;
            }
            0x0F if i + 1 < b.len() && (b[i + 1] == 0x84 || b[i + 1] == 0x85) => {
                cnt += 1;
                i += 6;
            }
            _ => break,
        }
    }
    cnt as i64
}
