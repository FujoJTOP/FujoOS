//! editor.rs — M73: 迷你编辑器 (vi 子集 v0)
//!
//! 内核文本缓冲 2KiB (BSS), 行模型: '\n' 分隔; 游标 (row, col)。
//! 命令键: i=插入模式 (Escape 退出) / j=下行 k=上行 / x=删游标字符 /
//!         ^=行首 $=行尾 / o=下行插空行 (v0: 见下)。
//! 接口: 0x7401 ed_init() / 0x7402 ed_text(ptr,n) 插入全文 /
//!       0x7403 ed_key(c) 单键处理 / 0x7404 ed_dump(ptr,cap) /
//!       0x7405 ed_info(ptr) → (row, col, lines, len)。

use crate::serial;

const CAP: usize = 2048;

static mut BUF: [u8; CAP] = [0; CAP];
static mut LEN: usize = 0;
static mut INSERT: bool = false;
static mut ROW: usize = 0;
static mut COL: usize = 0;

fn line_count() -> usize {
    unsafe {
        let mut lines = 1usize;
        for i in 0..LEN {
            if BUF[i] == b'\n' {
                lines += 1;
            }
        }
        lines
    }
}

fn row_start(row: usize) -> usize {
    unsafe {
        let mut r = 0usize;
        let mut pos = 0usize;
        while r < row && pos < LEN {
            if BUF[pos] == b'\n' {
                r += 1;
            }
            pos += 1;
        }
        pos
    }
}

fn row_len(row: usize) -> usize {
    unsafe {
        let start = row_start(row);
        let mut n = 0usize;
        while start + n < LEN && BUF[start + n] != b'\n' {
            n += 1;
        }
        n
    }
}

/// 0x7401
pub fn fujo_ed_init() -> i64 {
    unsafe {
        LEN = 0;
        INSERT = false;
        ROW = 0;
        COL = 0;
    }
    0
}

/// 0x7402: 插入全文 (游标被置末行尾; v0 供 demo 速成)。
pub fn fujo_ed_text(ptr: u64, n: u64) -> i64 {
    unsafe {
        let m = (n as usize).min(CAP);
        for i in 0..m {
            BUF[i] = (ptr as *const u8).add(i).read();
        }
        LEN = m;
        COL = row_len(line_count() - 1);
        ROW = line_count() - 1;
        m as i64
    }
}

fn ins_char(c: u8) {
    unsafe {
        if LEN >= CAP {
            return;
        }
        let pos = row_start(ROW) + COL;
        let mut i = LEN;
        while i > pos {
            BUF[i] = BUF[i - 1];
            i -= 1;
        }
        BUF[pos] = c;
        LEN += 1;
        COL += 1;
        if c == b'\n' {
            ROW += 1;
            COL = 0;
        }
    }
}

fn del_char() {
    unsafe {
        let pos = row_start(ROW) + COL;
        if pos < LEN {
            let mut i = pos;
            while i + 1 < LEN {
                BUF[i] = BUF[i + 1];
                i += 1;
            }
            LEN -= 1;
            if BUF[pos.min(LEN.saturating_sub(1))] == b'\n' {
                // 删除的是换行: 行合并, 游标留在行尾
                if pos > 0 && pos < LEN {
                    ROW = ROW.saturating_sub(1);
                    COL = row_len(ROW);
                }
            }
        }
    }
}

/// 0x7403: 单键。
pub fn fujo_ed_key(c: u64) -> i64 {
    let k = c as u8;
    unsafe {
        if INSERT {
            if k == 0x1B {
                INSERT = false;
            } else {
                ins_char(k);
            }
            return 0;
        }
        match k {
            b'i' => INSERT = true,
            b'j' => {
                if ROW + 1 < line_count() {
                    ROW += 1;
                    if COL > row_len(ROW) {
                        COL = row_len(ROW);
                    }
                }
            }
            b'k' => {
                if ROW > 0 {
                    ROW -= 1;
                    if COL > row_len(ROW) {
                        COL = row_len(ROW);
                    }
                }
            }
            b'x' => del_char(),
            b'^' => COL = 0,
            b'$' => COL = row_len(ROW),
            _ => {}
        }
    }
    0
}

/// 0x7404
pub fn fujo_ed_dump(ptr: u64, cap: u64) -> i64 {
    unsafe {
        let m = (cap as usize).min(LEN);
        for i in 0..m {
            (ptr as *mut u8).add(i).write(BUF[i]);
        }
    }
    unsafe { LEN as i64 }
}

/// 0x7405: (row, col, lines, len)。
pub fn fujo_ed_info(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        w.write(ROW as u64);
        w.add(1).write(COL as u64);
        w.add(2).write(line_count() as u64);
        w.add(3).write(LEN as u64);
    }
    0
}

/// M73: 启动自检演示 (demo 由 syscall 面驱动; 本函数供内核侧回归)。
pub fn selftest() -> bool {
    crate::serial::write_line("ed   : vi-subset ready (i/Esc/j/k/x/^/$)");
    true
}
