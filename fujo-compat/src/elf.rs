//! ELF（ELF32/ELF64，小端/大端）轻量解析。

use crate::{Arch, BinaryInfo, Format, be_u16, be_u32, be_u64, le_u16, le_u32, le_u64};

const EM_386: u16 = 3;
const EM_ARM: u16 = 40;
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;

/// 解析 ELF 首部（含节/段头部由加载器再处理；这里取架构与入口）。
pub fn parse(bytes: &[u8]) -> Result<BinaryInfo, String> {
    if bytes.len() < 0x40 {
        return Err("elf: file too small".into());
    }
    if bytes[0] != 0x7F || bytes[1] != b'E' || bytes[2] != b'L' || bytes[3] != b'F' {
        return Err("elf: bad magic".into());
    }
    let class = bytes[4];
    let data = bytes[5];
    if class != 1 && class != 2 {
        return Err(format!("elf: invalid class {class:#x}"));
    }
    if data != 1 && data != 2 {
        return Err(format!("elf: invalid data encoding {data:#x}"));
    }
    let be = data == 2;
    let e_type = if be { be_u16(bytes, 0x10)? } else { le_u16(bytes, 0x10)? };
    let machine = if be { be_u16(bytes, 0x12)? } else { le_u16(bytes, 0x12)? };
    let (arch, bits) = match (machine, class) {
        (EM_X86_64, 2) => (Arch::X86_64, 64),
        (EM_386, 1) => (Arch::X86, 32),
        (EM_AARCH64, 2) => (Arch::AArch64, 64),
        (EM_ARM, 1) => (Arch::Arm, 32),
        _ => return Err(format!("elf: unsupported machine {machine:#x} class {class:#x}")),
    };
    let entry = if be {
        if class == 2 { be_u64(bytes, 0x18)? } else { be_u32(bytes, 0x18)? as u64 }
    } else {
        if class == 2 { le_u64(bytes, 0x18)? } else { le_u32(bytes, 0x18)? as u64 }
    };
    Ok(BinaryInfo {
        format: Format::Elf,
        arch,
        bits,
        entry,
        pie: e_type == ET_DYN, // ET_DYN = PIE/共享库；ET_EXEC = 固定地址
        endian: if be { "big" } else { "little" },
    })
}
