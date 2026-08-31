//! # fujo-compat
//!
//! FujoOS 兼容层核心库（零依赖，纯 std）：
//! - `pe` / `elf` / `macho`: 三种主流可执行格式的轻量识别与头部解析
//! - `run`: FUJR（`.run`）容器格式的读写与校验
//! - `abi`: 三平台系统调用/API 表（兼容层声明式映射的起点）
//!
//! 设计原则：只读、无副作用、纯函数式解析；解析结果仅用于打包/装载决策，
//! 真正的执行由内核 syscall gate 与用户态垫片层（M2/M3）完成。

pub mod abi;
pub mod elf;
pub mod macho;
pub mod pe;
pub mod run;

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Pe,
    Elf,
    MachO,
    Unknown,
}

impl Format {
    pub fn as_str(&self) -> &'static str {
        match self {
            Format::Pe => "pe",
            Format::Elf => "elf",
            Format::MachO => "macho",
            Format::Unknown => "unknown",
        }
    }
    /// FUJR 容器 source_format 编码: 1=PE 2=ELF 3=Mach-O 0=raw
    pub fn code(&self) -> u32 {
        match self {
            Format::Pe => 1,
            Format::Elf => 2,
            Format::MachO => 3,
            Format::Unknown => 0,
        }
    }
    pub fn from_str(s: &str) -> Format {
        match s.to_ascii_lowercase().as_str() {
            "pe" => Format::Pe,
            "elf" => Format::Elf,
            "macho" | "mach-o" | "mach" => Format::MachO,
            _ => Format::Unknown,
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    AArch64,
    X86,
    Arm,
    Unknown,
}

impl Arch {
    pub fn as_str(&self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::AArch64 => "aarch64",
            Arch::X86 => "i386",
            Arch::Arm => "arm",
            Arch::Unknown => "unknown",
        }
    }
    pub fn from_str(s: &str) -> Arch {
        match s.to_ascii_lowercase().as_str() {
            "x86_64" | "x64" | "amd64" => Arch::X86_64,
            "aarch64" | "arm64" => Arch::AArch64,
            "i386" | "x86" | "i686" => Arch::X86,
            "arm" => Arch::Arm,
            _ => Arch::Unknown,
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct BinaryInfo {
    pub format: Format,
    pub arch: Arch,
    /// 指令位宽：32 / 64
    pub bits: u8,
    /// 入口地址（为解析值；PIE/共享库为相对偏移）
    pub entry: u64,
    /// 是否是位置无关 / 共享对象（需要加载器重定位基址）
    pub pie: bool,
    pub endian: &'static str,
}

/// 嗅探文件格式（仅看 magic）。
pub fn sniff(bytes: &[u8]) -> Format {
    if bytes.len() >= 2 && bytes[0] == b'M' && bytes[1] == b'Z' {
        return Format::Pe;
    }
    if bytes.len() >= 4
        && bytes[0] == 0x7F
        && bytes[1] == b'E'
        && bytes[2] == b'L'
        && bytes[3] == b'F'
    {
        return Format::Elf;
    }
    if bytes.len() >= 4 {
        let m = &bytes[0..4];
        // Magics: 32/64、大小端、fat/universal
        const MAGICS: [[u8; 4]; 6] = [
            [0xFE, 0xED, 0xFA, 0xCE], // 32 BE
            [0xCE, 0xFA, 0xED, 0xFE], // 32 LE
            [0xFE, 0xED, 0xFA, 0xCF], // 64 BE
            [0xCF, 0xFA, 0xED, 0xFE], // 64 LE
            [0xCA, 0xFE, 0xBA, 0xBE], // fat BE
            [0xBE, 0xBA, 0xFE, 0xCA], // fat LE
        ];
        if MAGICS.iter().any(|mm| &mm[..] == m) {
            return Format::MachO;
        }
    }
    Format::Unknown
}

/// 完整识别：格式 + 架构 + 入口等。失败返回错误描述。
pub fn inspect(bytes: &[u8]) -> Result<BinaryInfo, String> {
    match sniff(bytes) {
        Format::Pe => pe::parse(bytes),
        Format::Elf => elf::parse(bytes),
        Format::MachO => macho::parse(bytes),
        Format::Unknown => Err("unknown binary format (not PE/ELF/Mach-O)".into()),
    }
}

// ---- 小工具 ----

#[inline]
pub fn le_u16(b: &[u8], off: usize) -> Result<u16, String> {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| format!("read u16@{off:#x}: out of bounds"))
}

#[inline]
pub fn be_u16(b: &[u8], off: usize) -> Result<u16, String> {
    b.get(off..off + 2)
        .map(|s| u16::from_be_bytes([s[0], s[1]]))
        .ok_or_else(|| format!("read u16@{off:#x}: out of bounds"))
}

#[inline]
pub fn le_u32(b: &[u8], off: usize) -> Result<u32, String> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| format!("read u32@{off:#x}: out of bounds"))
}

#[inline]
pub fn be_u32(b: &[u8], off: usize) -> Result<u32, String> {
    b.get(off..off + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| format!("read u32@{off:#x}: out of bounds"))
}

#[inline]
pub fn le_u64(b: &[u8], off: usize) -> Result<u64, String> {
    b.get(off..off + 8)
        .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
        .ok_or_else(|| format!("read u64@{off:#x}: out of bounds"))
}

#[inline]
pub fn be_u64(b: &[u8], off: usize) -> Result<u64, String> {
    b.get(off..off + 8)
        .map(|s| u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
        .ok_or_else(|| format!("read u64@{off:#x}: out of bounds"))
}
