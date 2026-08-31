//! Mach-O（32/64、大小端、fat/universal）轻量解析。
//!
//! 入口点来自 LC_MAIN 的 entryoff（文件偏移），装载后加上 __TEXT 段 vmaddr。

use crate::{Arch, BinaryInfo, Format, be_u32, be_u64, le_u32, le_u64};

const CPU_TYPE_X86: u32 = 0x7;
const CPU_TYPE_X86_64: u32 = 0x0100_0007;
const CPU_TYPE_ARM: u32 = 0xC;
const CPU_TYPE_ARM64: u32 = 0x0100_000C;

const MH_EXECUTE: u32 = 0x2;
const LC_MAIN: u32 = 0x8000_0028; // LC_REQ_DYLD | 0x28

pub fn parse(bytes: &[u8]) -> Result<BinaryInfo, String> {
    if bytes.len() < 0x20 {
        return Err("macho: file too small".into());
    }
    let m = &bytes[0..4];
    let (le, bits, fat) = match m {
        [0xFE, 0xED, 0xFA, 0xCE] => (false, 32, false),
        [0xCE, 0xFA, 0xED, 0xFE] => (true, 32, false),
        [0xFE, 0xED, 0xFA, 0xCF] => (false, 64, false),
        [0xCF, 0xFA, 0xED, 0xFE] => (true, 64, false),
        [0xCA, 0xFE, 0xBA, 0xBE] => (false, 0, true),
        [0xBE, 0xBA, 0xFE, 0xCA] => (true, 0, true),
        _ => return Err("macho: bad magic".into()),
    };

    if fat {
        return parse_fat(bytes, le);
    }

    let cpu = if le { le_u32(bytes, 4)? } else { be_u32(bytes, 4)? };
    let filetype = if le { le_u32(bytes, 12)? } else { be_u32(bytes, 12)? };
    let ncmds = if le { le_u32(bytes, 16)? } else { be_u32(bytes, 16)? };
    let (arch, arch_bits) = match cpu {
        CPU_TYPE_X86_64 => (Arch::X86_64, 64),
        CPU_TYPE_X86 => (Arch::X86, 32),
        CPU_TYPE_ARM64 => (Arch::AArch64, 64),
        CPU_TYPE_ARM => (Arch::Arm, 32),
        _ => return Err(format!("macho: unsupported cputype {cpu:#x}")),
    };
    if filetype != MH_EXECUTE {
        return Err(format!(
            "macho: not an executable (filetype {filetype:#x}; only MH_EXECUTE supported so far)"
        ));
    }
    let hdr: usize = if arch_bits == 64 { 0x20 } else { 0x1C };
    let mut off = hdr;
    let mut entry: Option<u64> = None;
    for _ in 0..ncmds {
        let cmd = if le { le_u32(bytes, off)? } else { be_u32(bytes, off)? };
        let size = if le { le_u32(bytes, off + 4)? } else { be_u32(bytes, off + 4)? } as usize;
        if size < 8 || off + size > bytes.len() {
            return Err("macho: load command overruns file".into());
        }
        if cmd == LC_MAIN {
            let entryoff = if le { le_u64(bytes, off + 8)? } else { be_u64(bytes, off + 8)? };
            // LC_MAIN 的 entryoff 是相对 __TEXT 文件偏移；简化: 以文件偏移记录
            entry = Some(entryoff);
        }
        off += size;
    }
    Ok(BinaryInfo {
        format: Format::MachO,
        arch,
        bits: arch_bits,
        entry: entry.unwrap_or(0),
        pie: true,
        endian: if le { "little" } else { "big" },
    })
}

/// fat/universal：从所有切片中选第一个 64 位切片递归解析。
fn parse_fat(bytes: &[u8], le: bool) -> Result<BinaryInfo, String> {
    let n = if le { le_u32(bytes, 4)? } else { be_u32(bytes, 4)? };
    if n == 0 || n > 512 {
        return Err("macho: unreasonable fat arch count".into());
    }
    for i in 0..n {
        let ent = 8 + i as usize * 20;
        let cpu = if le { le_u32(bytes, ent)? } else { be_u32(bytes, ent)? };
        let off = if le { le_u32(bytes, ent + 8)? } else { be_u32(bytes, ent + 8)? } as usize;
        if cpu == CPU_TYPE_X86_64 || cpu == CPU_TYPE_ARM64 {
            if let Ok(info) = parse(&bytes[off..]) {
                return Ok(info);
            }
        }
    }
    Err("macho: no usable fat slice (64-bit)".into())
}
