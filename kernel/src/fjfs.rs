//! fjfs.rs — M16 FJFS v0: 极简 FAT-like 本地文件系统 (LBA28, ATA PIO)
//!
//! 布局 (4 MiB 卷 = 8192 扇区; 簇 = 4 扇区 = 2 KiB; 2244 簇... 实际: 2048 簇):
//!   sector 0      superblock: "FJFS01" + ver + total_clusters + bitmap_lba + root_lba
//!   sector 1      cluster bitmap (2048 bits = 256 B)
//!   sector 2..3   root dir (32 x 32B 目录项; 文件连续分配 v0)
//!   sector 4+     数据簇 (cluster 0 = sector 4)
//!
//! 文件: 根目录项 { name[16], size u32, first_cluster u32, attr u8 }。
//! 写入 = 分配连续簇 -> 写数据扇区 -> 更新位图与目录项 (写穿)。
//! 验收: QEMU 同一磁盘镜像两次启动 —— write 后重启 read 回读。

use crate::ata;
use crate::serial;

pub const SECTOR: usize = 512;
pub const CLUSTER_SECTORS: u32 = 4;
pub const CLUSTER_SIZE: usize = (CLUSTER_SECTORS as usize) * SECTOR; // 2048
pub const TOTAL_CLUSTERS: u32 = 2048;
pub const DATA_LBA: u32 = 4;
pub const MAX_FILE: usize = 64 * 1024; // v0 连续分配上限

#[derive(Clone, Copy)]
#[repr(C)]
struct DirEntry {
    name: [u8; 16],
    size: u32,
    first_cluster: u32,
    attr: u8,
    _pad: [u8; 7],
}

static mut VOLUME_OK: bool = false;
#[allow(static_mut_refs)]
static mut SECTOR0: [u8; SECTOR] = [0; SECTOR];
#[allow(static_mut_refs)]
static mut BITMAP: [u8; 512] = [0; 512]; // 512B (位图仅用前 256B; 512 防扇区读溢出)
#[allow(static_mut_refs)]
static mut ROOT: [DirEntry; 32] = [DirEntry {
    name: [0; 16],
    size: 0,
    first_cluster: 0,
    attr: 0,
    _pad: [0; 7],
}; 32];

fn rd_sector(lba: u32) {
    unsafe {
        let _ = ata::read_sectors(lba, 1, SECTOR0.as_mut_ptr());
    }
}

fn wr_sector(lba: u32) {
    unsafe {
        let _ = ata::write_sectors(lba, 1, SECTOR0.as_ptr());
    }
}

fn print_dec(v: u64) {
    let mut buf = [0u8; 24];
    let mut i = 24;
    let mut x = v;
    if x == 0 {
        serial::write_str("0");
        return;
    }
    while x > 0 {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    serial::write_str(core::str::from_utf8(&buf[i..]).unwrap());
}

fn print_hex(v: u64) {
    const HX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..16 {
        let d = ((v >> (4 * (15 - i))) & 0xF) as u8;
        buf[2 + i] = HX[d as usize];
    }
    serial::write_str(core::str::from_utf8(&buf).unwrap());
}

fn fmt_name(n: &[u8]) -> [u8; 16] {
    let mut r = [0u8; 16];
    let mut i = 0;
    while i < n.len() && i < 15 && n[i] != 0 {
        r[i] = n[i];
        i += 1;
    }
    r
}

/// 初始化: 探测卷; 无 FJFS 魔数则格式化 (清零+写入 superblock)。
pub fn init() -> bool {
    if !unsafe { ata::ATA_PRESENT } {
        serial::write_line("fjfs : no ATA drive - volume offline");
        return false;
    }
    let mut raw = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(SECTOR0)) };
    let ok = ata::read_sectors(0, 1, raw.as_mut_ptr());
    if !ok {
        serial::write_line("fjfs : superblock read failed");
        return false;
    }
    let magic_ok = raw[0] == b'F' && raw[1] == b'J' && raw[2] == b'F' && raw[3] == b'S';
    unsafe {
        if !magic_ok {
            // 格式化: 清 0..8 区, 写 superblock
            for lba in 0..8u32 {
                for i in 0..SECTOR {
                    SECTOR0[i] = 0;
                }
                let _ = ata::write_sectors(lba, 1, SECTOR0.as_ptr());
            }
            for i in 0..SECTOR {
                SECTOR0[i] = 0;
            }
            SECTOR0[0] = b'F';
            SECTOR0[1] = b'J';
            SECTOR0[2] = b'F';
            SECTOR0[3] = b'S';
            SECTOR0[4] = b'0';
            SECTOR0[5] = b'1';
            SECTOR0[8] = (TOTAL_CLUSTERS & 0xFF) as u8;
            SECTOR0[9] = ((TOTAL_CLUSTERS >> 8) & 0xFF) as u8;
            SECTOR0[10] = ((TOTAL_CLUSTERS >> 16) & 0xFF) as u8;
            SECTOR0[12] = 1; // bitmap_lba
            SECTOR0[16] = 2; // root_lba
            let _ = ata::write_sectors(0, 1, SECTOR0.as_ptr());
            serial::write_line("fjfs : volume formatted (4MiB, 2048 clusters)");
        } else {
            serial::write_line("fjfs : existing volume mounted");
        }
        VOLUME_OK = true;
        // 载入 bitmap + root
        let _ = ata::read_sectors(1, 1, BITMAP.as_mut_ptr());
        let _ = ata::read_sectors(2, 2, core::ptr::addr_of_mut!(ROOT).cast::<u8>());
        serial::write_str("fjfs : bitmap bits set=");
        let mut set = 0u64;
        for &b in BITMAP.iter() {
            set += b.count_ones() as u64;
        }
        print_dec(set);
        serial::write_line(" (volume ready)");
    }
    true
}

/// 查找根目录文件; 返回 (size, first_cluster)。
pub fn lookup(name: &[u8]) -> Option<(u32, u32)> {
    let n = fmt_name(name);
    unsafe {
        for e in ROOT.iter() {
            if e.name == n && e.size > 0 {
                return Some((e.size, e.first_cluster));
            }
        }
    }
    None
}

/// 分配连续簇 (n 个), 失败返回 None。
fn alloc_runs(n: u32) -> Option<u32> {
    unsafe {
        let mut i = 0u32;
        while i < TOTAL_CLUSTERS {
            // 找 n 个连续空位
            let mut ok = true;
            let mut j = 0u32;
            while j < n && i + j < TOTAL_CLUSTERS {
                let byte = ((i + j) / 8) as usize;
                let bit = ((i + j) % 8) as u8;
                if BITMAP[byte] & (1 << bit) != 0 {
                    ok = false;
                    break;
                }
                j += 1;
            }
            if ok && j == n {
                for k in 0..n {
                    let byte = ((i + k) / 8) as usize;
                    let bit = ((i + k) % 8) as u8;
                    BITMAP[byte] |= 1 << bit;
                }
                return Some(i);
            }
            i += 1;
        }
    }
    None
}

fn flush_bitmap() {
    unsafe {
        let _ = ata::write_sectors(1, 1, BITMAP.as_ptr());
    }
}

fn flush_root() {
    unsafe {
        let _ = ata::write_sectors(2, 2, core::ptr::addr_of!(ROOT).cast::<u8>());
    }
}

/// 写入/覆盖文件: (name, data, len) -> Ok。连续分配, 写穿位图+目录。
pub fn write_file(name: &[u8], data: *const u8, len: usize) -> bool {
    if len == 0 || len > MAX_FILE {
        return false;
    }
    let clusters = ((len + CLUSTER_SIZE - 1) / CLUSTER_SIZE) as u32;
    let first = match alloc_runs(clusters) {
        Some(c) => c,
        None => return false,
    };
    // 写数据 (逐簇逐扇区)
    let mut off = 0usize;
    for c in 0..clusters {
        let mut buf = [0u8; SECTOR];
        for s in 0..CLUSTER_SECTORS {
            for i in 0..SECTOR {
                buf[i] = if off < len {
                    unsafe { data.add(off).read() }
                } else {
                    0
                };
                off += 1;
            }
            let lba = DATA_LBA + (first + c) * CLUSTER_SECTORS + s;
            let _ = ata::write_sectors(lba, 1, buf.as_ptr());
        }
    }
    // 目录项 (覆盖/新建)
    let n = fmt_name(name);
    unsafe {
        let mut slot: *mut DirEntry = core::ptr::null_mut();
        for e in ROOT.iter_mut() {
            if e.name == n {
                slot = e as *mut DirEntry;
                break;
            }
        }
        if slot == core::ptr::null_mut() {
            for e in ROOT.iter_mut() {
                if e.size == 0 {
                    slot = e as *mut DirEntry;
                    break;
                }
            }
        }
        if slot == core::ptr::null_mut() {
            return false; // 根目录满
        }
        (*slot).name = n;
        (*slot).size = len as u32;
        (*slot).first_cluster = first;
        (*slot).attr = 0;
    }
    flush_bitmap();
    flush_root();
    pure_stats();
    true
}

fn pure_stats() {
    serial::write_str("fjfs : written (");
    let mut files = 0u32;
    unsafe {
        for e in ROOT.iter() {
            if e.size > 0 {
                files += 1;
            }
        }
    }
    print_dec(files as u64);
    serial::write_line(" root entries)");
}

/// 读取文件内容 (v0: 连续簇) 到 buf; 返回长度。
pub fn read_file(name: &[u8], buf: *mut u8, max: usize) -> usize {
    match lookup(name) {
        None => 0,
        Some((size, first)) => {
            let n = (size as usize).min(max);
            let mut off = 0usize;
            let total_clusters = (size as usize + CLUSTER_SIZE - 1) / CLUSTER_SIZE;
            for c in 0..total_clusters {
                let mut sector = [0u8; SECTOR];
                for s in 0..CLUSTER_SECTORS {
                    let lba = DATA_LBA + (first + c as u32) * CLUSTER_SECTORS + s;
                    let _ = ata::read_sectors(lba, 1, sector.as_mut_ptr());
                    for i in 0..SECTOR {
                        if off < n {
                            unsafe { buf.add(off).write(sector[i]); }
                            off += 1;
                        }
                    }
                }
            }
            n
        }
    }
}

/// 命令行诊断: 列出根目录。
pub fn list() {
    unsafe {
        let mut n = 0u32;
        for e in ROOT.iter() {
            if e.size > 0 {
                serial::write_str("fjfs :    " );
                let name_len = core::str::from_utf8(&e.name[..])
                    .ok()
                    .map(|s| s.find('\0').unwrap_or(s.len()))
                    .unwrap_or(0);
                serial::write_str(core::str::from_utf8(&e.name[..name_len]).unwrap_or("?"));
                serial::write_str(" (size=");
                print_dec(e.size as u64);
                serial::write_str(", cl=");
                print_hex(e.first_cluster as u64);
                serial::write_line(")");
                n += 1;
            }
        }
        serial::write_str("fjfs : ");
        print_dec(n as u64);
        serial::write_line(" files in volume");
    }
}

/// M97: 卷就绪状态 + 文件数 (hw 汇总面)。
pub fn superblock_ok() -> bool {
    unsafe { VOLUME_OK }
}

pub fn file_count() -> u64 {
    unsafe {
        let mut n = 0u64;
        for e in ROOT.iter() {
            if e.size > 0 {
                n += 1;
            }
        }
        n
    }
}
