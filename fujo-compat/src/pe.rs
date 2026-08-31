//! PE / PE32+（Windows 可执行与 DLL）轻量解析。
//!
//! 目标：打包决策与加载器入口点/架构识别，不做完整重定位解析（那属于 M3 加载器）。

use crate::{Arch, BinaryInfo, Format, le_u16, le_u32, le_u64};

const MACHINE_X64: u16 = 0x8664;
const MACHINE_X86: u16 = 0x014C;
const MACHINE_ARM64: u16 = 0xAA64;
const MACHINE_ARM: u16 = 0x01C0;
const MACHINE_ARMNT: u16 = 0x01C4;

/// 解析 PE 文件首部，返回架构/入口/图像基址信息。
pub fn parse(bytes: &[u8]) -> Result<BinaryInfo, String> {
    if bytes.len() < 0x40 {
        return Err("pe: file too small".into());
    }
    let pe_off = le_u32(bytes, 0x3C)? as usize;
    if bytes.get(pe_off..pe_off + 4) != Some(&[b'P', b'E', 0, 0]) {
        return Err("pe: no PE signature".into());
    }
    let coff = pe_off + 4;
    let machine = le_u16(bytes, coff)?;
    let opt_size = le_u16(bytes, coff + 20)? as usize;
    let opt = coff + 20; // IMAGE_FILE_HEADER = 20 字节; optional header 紧随其后
    if opt_size < 0x60 || opt + opt_size > bytes.len() {
        return Err("pe: optional header truncated".into());
    }
    let magic = le_u16(bytes, opt)?;
    let entry_rva = le_u32(bytes, opt + 16)?;
    let dll_char = le_u16(bytes, opt + if magic == 0x20B { 70 } else { 68 })?;
    let (arch, bits, image_base) = match magic {
        0x10B => {
            // PE32
            let base = le_u32(bytes, opt + 28)? as u64;
            (machine_to_arch(machine), 32, base)
        }
        0x20B => {
            // PE32+
            let base = le_u64(bytes, opt + 24)?;
            (machine_to_arch(machine), 64, base)
        }
        m => return Err(format!("pe: unsupported optional-header magic {m:#x}")),
    };
    if arch == Arch::Unknown {
        return Err(format!("pe: unsupported machine {machine:#x}"));
    }
    let entry = image_base + entry_rva as u64;
    let pie = dll_char & 0x0040 != 0; // IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE
    Ok(BinaryInfo {
        format: Format::Pe,
        arch,
        bits,
        entry,
        pie,
        endian: "little",
    })
}

fn machine_to_arch(m: u16) -> Arch {
    match m {
        MACHINE_X64 => Arch::X86_64,
        MACHINE_X86 => Arch::X86,
        MACHINE_ARM64 => Arch::AArch64,
        MACHINE_ARM | MACHINE_ARMNT => Arch::Arm,
        _ => Arch::Unknown,
    }
}
