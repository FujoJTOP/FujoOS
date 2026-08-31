//! FUJR（`.run`）容器格式 v0.1 读写。
//!
//! # 磁盘布局（小端）
//!
//! ```text
//! +------------------+------ 0
//! | Header (64 B)    |
//! +------------------+------ 64
//! | Section table    |      32 B × n
//! +------------------+------ 64 + 32n
//! | Section 0 ...    |
//! | Section n-1      |      （默认按 4096 对齐）
//! +------------------+
//! ```
//!
//! # Header
//! | off  | size | 含义                                            |
//! |------|------|--------------------------------------------------|
//! | 0    | 4    | magic = "FUJR"                                  |
//! | 4    | 2    | version_major（当前 0）                          |
//! | 6    | 2    | version_minor（当前 1）                          |
//! | 8    | 4    | section_count                                   |
//! | 12   | 8    | header_size（当前 64）                           |
//! | 20   | 8    | total_size（文件总长）                           |
//! | 28   | 16   | uid（构建随机标识）                              |
//! | 44   | 4    | flags（bit0: 签名后锁定）                        |
//! | 48   | 4    | manifest_index                                 |
//! | 52   | 2    | target_arch（1=x86_64 2=aarch64 3=i386 4=arm）  |
//! | 54   | 2    | base_arch（Embedded 原始格式的架构）            |
//! | 56   | 4    | source_format（1=PE 2=ELF 3=Mach-O 0=raw）     |
//! | 60   | 4    | reserved                                       |
//!
//! # Section entry (32 B)
//! | off  | size | 含义                                            |
//! |------|------|--------------------------------------------------|
//! | 0    | 4    | tag（见 TAG_*）                                 |
//! | 4    | 4    | flags                                            |
//! | 8    | 8    | offset（相对文件头）                             |
//! | 16   | 8    | size                                             |
//! | 24   | 4    | hash（FNV-1a-32 of data）                        |
//! | 28   | 4    | reserved                                         |

use crate::{le_u32, le_u64};

pub const MAGIC: [u8; 4] = *b"FUJR";
pub const VERSION_MAJOR: u16 = 0;
pub const VERSION_MINOR: u16 = 1;
pub const HEADER_SIZE: usize = 64;
pub const SECTION_ENTRY_SIZE: usize = 32;

pub const TAG_MANIFEST: u32 = 1;
pub const TAG_CODE: u32 = 2; // 已翻译/原生 FujoOS 代码单元（M7 AOT）
pub const TAG_IR: u32 = 3; // 可移植中间表示（跨架构 JIT 源，M7）
pub const TAG_EMBED: u32 = 4; // 原始二进制（PE/ELF/Mach-O）
pub const TAG_DATA: u32 = 5; // 附属资源/共享库
pub const TAG_SIGN: u32 = 6; // 签名与公钥（M8）
pub const TAG_ICON: u32 = 7;

pub fn tag_name(tag: u32) -> &'static str {
    match tag {
        TAG_MANIFEST => "MANIFEST",
        TAG_CODE => "CODE",
        TAG_IR => "IR",
        TAG_EMBED => "EMBED",
        TAG_DATA => "DATA",
        TAG_SIGN => "SIGN",
        TAG_ICON => "ICON",
        _ => "PAD",
    }
}

/// FNV-1a 32-bit（section 完整性散列）。
pub fn fnv1a(data: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for &b in data {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

#[derive(Debug, Clone)]
pub struct RunPart {
    pub tag: u32,
    pub flags: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct RunMeta {
    /// 16 字节构建 uid（由打包器填时间戳+随机）
    pub uid: [u8; 16],
    pub target_arch: u16,
    pub base_arch: u16,
    pub source_format: u32,
    pub flags: u32,
    pub manifest_index: u32,
}

#[derive(Debug, Clone)]
pub struct RunSectionInfo {
    pub tag: u32,
    pub flags: u32,
    pub offset: u64,
    pub size: u64,
    pub hash: u32,
}

#[derive(Debug, Clone)]
pub struct RunInfo {
    pub version: (u16, u16),
    pub section_count: u32,
    pub total_size: u64,
    pub uid: [u8; 16],
    pub flags: u32,
    pub manifest_index: u32,
    pub target_arch: u16,
    pub base_arch: u16,
    pub source_format: u32,
    pub sections: Vec<RunSectionInfo>,
}

pub fn write_run(parts: &[RunPart], meta: &RunMeta) -> Vec<u8> {
    let n = parts.len() as u32;
    let mut out: Vec<u8> = Vec::new();
    // header
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION_MAJOR.to_le_bytes());
    out.extend_from_slice(&VERSION_MINOR.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    let header_size = HEADER_SIZE as u64 + (SECTION_ENTRY_SIZE as u64) * n as u64;
    out.extend_from_slice(&header_size.to_le_bytes());
    let total = header_size
        + parts
            .iter()
            .map(|p| (p.data.len() as u64 + 4095) / 4096 * 4096)
            .sum::<u64>()
        + (if parts.is_empty() { 0 } else { 0 });
    // total 先占位，稍后回填
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&meta.uid);
    out.extend_from_slice(&meta.flags.to_le_bytes());
    out.extend_from_slice(&meta.manifest_index.to_le_bytes());
    out.extend_from_slice(&meta.target_arch.to_le_bytes());
    out.extend_from_slice(&meta.base_arch.to_le_bytes());
    out.extend_from_slice(&meta.source_format.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(out.len(), HEADER_SIZE);

    // section table（占位，回填）
    let mut entries: Vec<(u64, u64, u32)> = Vec::new();
    for p in parts {
        entries.push((0, p.data.len() as u64, fnv1a(&p.data)));
        out.extend_from_slice(&p.tag.to_le_bytes());
        out.extend_from_slice(&p.flags.to_le_bytes());
        out.extend_from_slice(&0u64.to_le_bytes()); // offset 占位
        out.extend_from_slice(&(p.data.len() as u64).to_le_bytes());
        out.extend_from_slice(&fnv1a(&p.data).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
    }

    // payload（4096 对齐）
    let mut offsets: Vec<u64> = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        offsets.push(out.len() as u64);
        out.extend_from_slice(&p.data);
        while out.len() % 4096 != 0 {
            out.push(0);
        }
        entries[i].0 = offsets[i];
    }

    // 回填 offsets + total
    for (i, (off, _sz, _h)) in entries.iter().enumerate() {
        let base = HEADER_SIZE + i * SECTION_ENTRY_SIZE;
        out[base + 8..base + 16].copy_from_slice(&off.to_le_bytes());
    }
    let total_size = out.len() as u64;
    out[20..28].copy_from_slice(&total_size.to_le_bytes());

    debug_assert_eq!(total, 0, "placeholder"); // keep total consistent above
    let _ = total;
    out
}

pub fn read_run(bytes: &[u8]) -> Result<RunInfo, String> {
    if bytes.len() < HEADER_SIZE {
        return Err("run: file too small".into());
    }
    if bytes[0..4] != MAGIC {
        return Err("run: bad magic (not FUJR)".into());
    }
    let ver = (le_u32(bytes, 4)? as u16, le_u32(bytes, 6)? as u16);
    let section_count = le_u32(bytes, 8)?;
    let header_size = le_u64(bytes, 12)?;
    let total_size = le_u64(bytes, 20)?;
    let uid: [u8; 16] = bytes[28..44].try_into().unwrap();
    let flags = le_u32(bytes, 44)?;
    let manifest_index = le_u32(bytes, 48)?;
    let target_arch = le_u32(bytes, 52)? as u16;
    let base_arch = le_u32(bytes, 54)? as u16;
    let source_format = le_u32(bytes, 56)?;
    let _reserved = le_u32(bytes, 60)?;

    let expected = HEADER_SIZE as u64 + (SECTION_ENTRY_SIZE as u64) * section_count as u64;
    if header_size != expected {
        return Err(format!(
            "run: header size mismatch (field {header_size}, expected {expected})"
        ));
    }
    if total_size != bytes.len() as u64 {
        return Err(format!(
            "run: total size mismatch (field {total_size}, file {})",
            bytes.len()
        ));
    }
    if section_count as usize * SECTION_ENTRY_SIZE + HEADER_SIZE > bytes.len() {
        return Err("run: section table overruns file".into());
    }
    let mut sections = Vec::new();
    for i in 0..section_count as usize {
        let base = HEADER_SIZE + i * SECTION_ENTRY_SIZE;
        sections.push(RunSectionInfo {
            tag: le_u32(bytes, base)?,
            flags: le_u32(bytes, base + 4)?,
            offset: le_u64(bytes, base + 8)?,
            size: le_u64(bytes, base + 16)?,
            hash: le_u32(bytes, base + 24)?,
        });
    }
    for s in &sections {
        let end = s.offset + s.size;
        if s.offset < expected || end > bytes.len() as u64 {
            return Err(format!(
                "run: section {} out of bounds ({:#x}..{end:#x}, file {})",
                tag_name(s.tag),
                s.offset,
                bytes.len()
            ));
        }
        let data = &bytes[s.offset as usize..end as usize];
        let h = fnv1a(data);
        if h != s.hash {
            return Err(format!(
                "run: section {} hash mismatch (stored {:#010x}, computed {h:#010x})",
                tag_name(s.tag),
                s.hash
            ));
        }
    }
    Ok(RunInfo {
        version: ver,
        section_count,
        total_size,
        uid,
        flags,
        manifest_index,
        target_arch,
        base_arch,
        source_format,
        sections,
    })
}

pub fn section_data<'a>(bytes: &'a [u8], info: &RunInfo, tag: u32) -> Option<&'a [u8]> {
    let s = info.sections.iter().find(|s| s.tag == tag)?;
    Some(&bytes[s.offset as usize..s.offset as usize + s.size as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_known() {
        assert_eq!(fnv1a(b""), 0x811C_9DC5);
        assert_eq!(fnv1a(b"a"), 0xE40C_292C);
    }

    #[test]
    fn roundtrip_write_read() {
        let parts = vec![
            RunPart { tag: TAG_MANIFEST, flags: 0, data: br#"{"x":1}"#.to_vec() },
            RunPart { tag: TAG_EMBED, flags: 0, data: vec![0x7F; 5120] },
        ];
        let meta = RunMeta {
            uid: [0xAB; 16],
            target_arch: 1,
            base_arch: 1,
            source_format: 2,
            flags: 0,
            manifest_index: 0,
        };
        let bytes = write_run(&parts, &meta);
        let info = read_run(&bytes).expect("read_run ok");
        assert_eq!(info.version, (0, 1));
        assert_eq!(info.sections.len(), 2);
        assert_eq!(info.uid, [0xAB; 16]);
        assert_eq!(info.source_format, 2);
        let m = section_data(&bytes, &info, TAG_MANIFEST).unwrap();
        assert_eq!(m, b"{\"x\":1}");
        // 篡改一个字节 -> 校验必须失败
        let mut bad = bytes.clone();
        let emb = info.sections.iter().find(|s| s.tag == TAG_EMBED).unwrap();
        bad[emb.offset as usize] ^= 0xFF;
        assert!(read_run(&bad).is_err());
    }

    #[test]
    fn reject_bad_magic() {
        assert!(read_run(b"not a run").is_err());
    }
}
