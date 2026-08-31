//! vfs.rs — M15 VFS v0 (内存文件系统)
//!
//! 文件表 (fd -> File), 内存后端:
//!   /boot/module    只读: multiboot initrd 模块本体 (字节可读)
//!   /proc/meminfo   只读: 启动时生成的机器状态文本
//!   /tmp/hello.txt  读写: 内存盘 (预置内容; open(W) 后 write 追加)
//!   /dev/tty        写: 串口 (日志通道, fd 1/2 同路)
//! Linux ABI 直通: open(nr2) / read(nr0) / close(nr3); write(nr1) 对 fd>=3
//! 追加到内存盘。
//!
//! v0 注: 单套文件表 (进程级 fd 表属 M14b/M17 后续); 路径匹配固定表。

use crate::serial;

pub const F_KIND_BLOB: u8 = 0; // 静态内存 (模块)
pub const F_KIND_GEN: u8 = 1; // 生成内容 (proc)
pub const F_KIND_RAM: u8 = 2; // 内存盘读写
pub const F_KIND_DISK: u8 = 3; // fujofs 磁盘文件 (M16)
pub const F_KIND_PIPE: u8 = 4; // IPC 管道 (M18; data_ptr=Pipe*)

pub const MAX_OPEN: usize = 16;

#[derive(Clone, Copy)]
pub struct File {
    pub name: &'static str,
    pub kind: u8,
    pub data_ptr: *const u8,
    pub data_len: u64,
    pub pos: u64,
    /// M20: 登记的内核对象槽 (pipe 端点; 0=none)
    pub kslot: usize,
}

// 文件表: 0..=2 保留 (0=/dev/null, 1/2=/dev/tty 串口), 3.. 由 open 分配
static mut FILES: [File; MAX_OPEN] = [
    File { name: "/dev/null", kind: 0, data_ptr: core::ptr::null(), data_len: 0, pos: 0, kslot: 0 };
    MAX_OPEN
];
static mut NEXT_FD: usize = 3;

/// M20: fd 槽分配 (扫描复用空闲槽, 取代只增的 NEXT_FD 计数器 -> 无泄漏)。
fn alloc_fd_slot() -> Option<usize> {
    unsafe {
        for fd in 3..MAX_OPEN {
            let f = &*core::ptr::addr_of!(FILES[fd]);
            if f.kind == 0 && f.name == "/dev/null" {
                return Some(fd);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// M18 · IPC 管道 fd 登记 (vfs 表复用, kind=F_KIND_PIPE)
// ---------------------------------------------------------------------------

/// 分配一个指向 Pipe 的 fd (fd 表记录 data_ptr = Pipe*, kslot = kobj 槽)。
pub fn alloc_pipe_fd(p: *mut crate::ipc::Pipe) -> Option<usize> {
    unsafe {
        let fd = match alloc_fd_slot() {
            Some(f) => f,
            None => return None, // -EMFILE (槽复用: 关闭后由 free_fd 复位)
        };
        NEXT_FD = NEXT_FD.max(fd + 1);
        // M19: 登记内核对象 (pipe 端点)。kslot 存 slot+1 (0=无登记哨兵,
        // 与 kobj 槽 0 冲突 -> M20 首端泄漏实证: free_fd 跳过了 slot 0!)
        let kslot = crate::kobj::alloc(crate::kobj::K_PIPE, fd as u64)
            .map(|s| s + 1)
            .unwrap_or(0);
        let f = &mut *core::ptr::addr_of_mut!(FILES[fd]);
        f.name = "/pipe";
        f.kind = F_KIND_PIPE;
        f.data_ptr = p as *const u8;
        f.data_len = crate::ipc::PIPE_SIZE as u64; // 容量提示 (read 走 pipe_read)
        f.pos = 0;
        f.kslot = kslot;
        Some(fd)
    }
}

/// 释放 fd (回滚路径 / close 复用)。
pub fn free_fd(fd: usize) {
    unsafe {
        if fd >= 3 && fd < MAX_OPEN {
            let f = &mut *core::ptr::addr_of_mut!(FILES[fd]);
            // M20: 回收登记的 kobj (kslot 编码 = slot+1; 0 = 无登记)
            if f.kslot != 0 {
                crate::kobj::free(f.kslot - 1);
                f.kslot = 0;
            }
            f.name = "/dev/null";
            f.kind = 0;
            f.data_ptr = core::ptr::null();
            f.data_len = 0;
            f.pos = 0;
        }
    }
}

static mut BOOT_MODULE: (u64, u64) = (0, 0); // (addr, len)
static mut MODULE_COPY: [u8; 4096] = [0; 4096];
static mut MODULE_COPY_LEN: usize = 0;

// ---- M16 fujofs 磁盘文件缓存 ----
static mut DISK_CACHE: [u8; 2048] = [0; 2048];
static mut DISK_CACHE_LEN: usize = 0;
static mut DISK_DIRTY: bool = false;
static mut RAMDISK: [u8; 4096] = [0; 4096];
static mut RAMDISK_LEN: usize = 0;
static mut MEMINFO: [u8; 256] = [0; 256];
static mut MEMINFO_LEN: usize = 0;

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

/// 记录引导模块 (initrd) 供 /boot/module 使用; 引导期拷入内核静态缓冲
/// (syscall 期直接读 0x221000 偶发 #GP —— 与模块区/填充区寻址相关, 拷贝绕开)。
pub fn set_boot_module(addr: u64, len: u64) {
    unsafe {
        BOOT_MODULE = (addr, len);
        let n = (len as usize).min(MODULE_COPY.len());
        for i in 0..n {
            MODULE_COPY[i] = (addr as *const u8).add(i).read_volatile();
        }
        MODULE_COPY_LEN = n;
    }
}

/// 初始化: 内存盘种子 + 生成 /proc/meminfo 内容。
pub fn init() {
    unsafe {
        // /tmp/hello.txt 种子
        let seed = b"hello from FujoOS ramdisk (M15)\n";
        for (i, &b) in seed.iter().enumerate() {
            RAMDISK[i] = b;
        }
        RAMDISK_LEN = seed.len();
        // /proc/meminfo 生成
        let mut n = 0usize;
        let tpl: &[u8] = b"mem_total=127MiB\nuser_heap=0x800000..0xC00000\nboot_module=";
        for &b in tpl.iter() {
            if n < 128 {
                MEMINFO[n] = b;
                n += 1;
            }
        }
        let (baddr, blen) = BOOT_MODULE;
        let hex = format_hex(baddr);
        for &b in hex.iter() {
            if n < 128 {
                MEMINFO[n] = b;
                n += 1;
            }
        }
        let tpl2: &[u8] = b"\n";
        for &b in tpl2.iter() {
            if n < 128 {
                MEMINFO[n] = b;
                n += 1;
            }
        }
        MEMINFO_LEN = n;
        serial::write_str("vfs  : mounted [memory-backed] /boot/module len=");
        print_dec(blen);
        serial::write_line("; /proc/meminfo; /tmp/hello.txt; /dev/tty");
    }
}

fn format_hex(v: u64) -> [u8; 18] {
    const HX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..16 {
        let d = ((v >> (4 * (15 - i))) & 0xF) as u8;
        buf[2 + i] = HX[d as usize];
    }
    buf
}

fn path_of(ptr: u64, len: u64) -> Option<[u8; 64]> {
    if !(0x400000..0xC00000).contains(&ptr) {
        return None;
    }
    let mut p = [0u8; 64];
    let mut n = 0usize;
    unsafe {
        // Linux open: 路径为 NUL 结尾字符串 (len 实参是 flags, 无长度语义)
        while n < 63 {
            let b = (ptr as *const u8).add(n).read();
            if b == 0 {
                break;
            }
            p[n] = b;
            n += 1;
        }
    }
    Some(p)
}

/// M26/M27: PE 程序启动时预打开 /boot/module -> fd=3 (kernel32 文件句柄家族
/// 可直读自身模块; Windows 语义: 程序模块即一个可读句柄)。
#[no_mangle]
pub extern "C" fn fujo_open_startup_module() -> i64 {
    unsafe {
        let fd = match alloc_fd_slot() {
            Some(f) => f,
            None => return -24, // -EMFILE
        };
        NEXT_FD = NEXT_FD.max(fd + 1);
        let f = &mut *core::ptr::addr_of_mut!(FILES[fd]);
        f.name = "/boot/module";
        f.kind = F_KIND_BLOB;
        f.data_ptr = core::ptr::addr_of!(MODULE_COPY) as *const u8;
        f.data_len = MODULE_COPY_LEN as u64;
        f.pos = 0;
        serial::write_str("vfs  : pre-open /boot/module fd=");
        print_dec(fd as u64);
        serial::write_line("");
        fd as i64
    }
}

/// M32: 多模块库表 (/lib/<name> -> 模块字节)。由 fujorun 多模块解析注册。
static mut LIBS: [([u8; 16], u64, u64); 8] = [([0; 16], 0, 0); 8];
static mut LIB_COUNT: usize = 0;

/// M32: 库模块注册 (多模块 initrd 解析器调用)。
pub fn fujo_lib_register(name: &str, addr: u64, len: u64) {
    unsafe {
        if LIB_COUNT >= 8 {
            return;
        }
        let mut nm = [0u8; 16];
        for (k, b) in name.as_bytes().iter().take(15).enumerate() {
            nm[k] = *b;
        }
        LIBS[LIB_COUNT] = (nm, addr, len);
        LIB_COUNT += 1;
    }
}

fn fujo_lib_find(name: &[u8]) -> Option<(*const u8, usize)> {
    unsafe {
        let n = name.len().min(15);
        for k in 0..LIB_COUNT {
            let (nm, addr, len) = LIBS[k];
            let mut same = true;
            for i in 0..n {
                if nm[i] != name[i] {
                    same = false;
                    break;
                }
            }
            if same {
                for i in n..15 {
                    if nm[i] != 0 {
                        same = false;
                        break;
                    }
                }
            }
            if same && len > 0 {
                return Some((addr as *const u8, len as usize));
            }
        }
    }
    None
}

/// 系统调用 open(nr2): 匹配固定表; flags: 0=RDONLY, 1=WRONLY, 2=RDWR。
#[no_mangle]
pub extern "C" fn fujo_open(ptr: u64, len: u64, flags: u64) -> i64 {
    let p = match path_of(ptr, len) {
        Some(p) => p,
        None => return -14, // -EFAULT
    };
    // 截断 NUL (路径为字符串, p 可能带 0 填充)
    let full = core::str::from_utf8(&p).unwrap_or("");
    let end = full.find('\0').unwrap_or(full.len());
    let name = &full[..end];
    fujo_open_name(name, flags)
}

/// M30: 统一对象路径打开 (linux/darwin/win32 垫片共用)。
pub fn fujo_open_name(name: &str, _flags: u64) -> i64 {
    unsafe {
        // 分配 fd (M20: 槽复用)
        let fd = match alloc_fd_slot() {
            Some(f) => f,
            None => return -24, // -EMFILE
        };
        NEXT_FD = NEXT_FD.max(fd + 1);
        let f = &mut *core::ptr::addr_of_mut!(FILES[fd]);
        if name == "/boot/module" {
            f.name = "/boot/module";
            f.kind = F_KIND_BLOB;
            f.data_ptr = core::ptr::addr_of!(MODULE_COPY) as *const u8;
            f.data_len = MODULE_COPY_LEN as u64;
            f.pos = 0;
            serial::write_line("vfs  : open /boot/module (initrd copy)");
            return fd as i64;
        } else if name == "/proc/meminfo" {
            f.name = "/proc/meminfo";
            f.kind = F_KIND_GEN;
            f.data_ptr = core::ptr::addr_of!(MEMINFO) as *const u8;
            f.data_len = MEMINFO_LEN as u64;
            f.pos = 0;
            serial::write_line("vfs  : open /proc/meminfo (generated)");
            return fd as i64;
        } else if name == "/tmp/hello.txt" {
            f.name = "/tmp/hello.txt";
            f.kind = F_KIND_RAM;
            f.data_ptr = core::ptr::addr_of!(RAMDISK) as *const u8;
            f.data_len = RAMDISK_LEN as u64;
            f.pos = 0;
            serial::write_line("vfs  : open /tmp/hello.txt (ramdisk)");
            return fd as i64;
        } else if name == "/dev/tty" {
            f.name = "/dev/tty";
            f.kind = F_KIND_GEN; // 仅作为写通道; data_ptr 无意义
            f.data_len = 0;
            f.pos = 0;
            serial::write_line("vfs  : open /dev/tty (serial)");
            return fd as i64;
        } else if let Some(lname) = name.strip_prefix("/lib/") {
            // M32: 多模块库目录 (fujorun 注册的库/资源模块)
            if let Some((ptr, len)) = fujo_lib_find(lname.as_bytes()) {
                f.name = "/lib/";
                f.kind = F_KIND_BLOB;
                f.data_ptr = ptr;
                f.data_len = len as u64;
                f.pos = 0;
                serial::write_str("vfs  : open /lib/");
                serial::write_str(lname);
                serial::write_line("");
                return fd as i64;
            }
            serial::write_str("vfs  : open /lib/");
            serial::write_str(lname);
            serial::write_line(" not found -ENOENT");
            NEXT_FD -= 1;
            return -2;
        } else if let Some(rname) = name.strip_prefix("/runres/") {
            // M17: FUJR 容器资源 (已由 fujr::load 拷入内核静态)
            if let Some((ptr, len)) = crate::fujr::resource(rname.as_bytes()) {
                f.name = "/runres/";
                f.kind = F_KIND_BLOB;
                f.data_ptr = ptr;
                f.data_len = len as u64;
                f.pos = 0;
                serial::write_str("vfs  : open /runres/");
                serial::write_str(rname);
                serial::write_line("");
                return fd as i64;
            }
            serial::write_str("vfs  : open /runres/");
            serial::write_str(rname);
            serial::write_line(" not found -ENOENT");
            NEXT_FD -= 1;
            return -2;
        } else if let Some(fname) = name.strip_prefix("/disk/") {
            // M16: fujofs 磁盘文件 (打开时载入缓存; 写穿在 close 刷盘)
            let fname_bytes = fname.as_bytes();
            let len = crate::fjfs::read_file(fname_bytes, core::ptr::addr_of_mut!(DISK_CACHE).cast::<u8>(), 2048);
            DISK_CACHE_LEN = len;
            DISK_DIRTY = false;
            f.name = "/disk/";
            // 记录短名: 存到 data_len 无意义 -> 用 name 的静态拷贝
            // (File.name 是 &'static str; 磁盘文件全走缓存与脏标记, 短名从路径再解析)
            f.kind = F_KIND_DISK;
            f.data_ptr = core::ptr::addr_of!(DISK_CACHE) as *const u8;
            f.data_len = DISK_CACHE_LEN as u64;
            f.pos = 0;
            serial::write_str("vfs  : open /disk/");
            serial::write_str(fname);
            serial::write_str(" (cache ");
            print_dec(len as u64);
            serial::write_line(" bytes)");
            return fd as i64;
        }
        NEXT_FD -= 1; // 回滚
        serial::write_str("vfs  : open unknown '");
        serial::write_str(name);
        serial::write_line("' -ENOENT");
        -2 // -ENOENT
    }
}

/// 系统调用 read(nr0): fd -> 用户缓冲区。
#[no_mangle]
pub extern "C" fn fujo_read(fd: u64, buf: u64, len: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&buf) {
        return -14; // -EFAULT
    }
    unsafe {
        if fd < 3 || fd >= MAX_OPEN as u64 {
            return -9; // -EBADF
        }
        let f = &mut *core::ptr::addr_of_mut!(FILES[fd as usize]);
        if f.kind == F_KIND_PIPE {
            // M18: 管道读 (data_ptr = Pipe*)
            return crate::ipc::pipe_read(f.data_ptr as *const crate::ipc::Pipe, buf, len);
        }
        if f.data_ptr as u64 == 0 {
            return 0;
        }
        let remaining = f.data_len.saturating_sub(f.pos);
        let n = (len.min(remaining)) as usize;
        let mut i = f.pos as usize;
        // volatile 逐字节拷贝 (防 LLVM 向量化为 SSE mov — 模块尾区 addr 偶发 #GP)
        for k in 0..n {
            core::ptr::write_volatile(
                (buf as *mut u8).add(k),
                core::ptr::read_volatile(f.data_ptr.add(i + k)),
            );
        }
        f.pos += n as u64;
        n as i64
    }
}

/// M26: 文件大小 (kernel32!GetFileSize 直通) —— fd -> data_len。
#[no_mangle]
pub extern "C" fn fujo_size(fd: u64) -> i64 {
    unsafe {
        if fd >= 3 && (fd as usize) < MAX_OPEN {
            let f = &*core::ptr::addr_of!(FILES[fd as usize]);
            return f.data_len as i64;
        }
    }
    -1 // -EBADF
}

/// 系统调用 close(nr3)。
#[no_mangle]
pub extern "C" fn fujo_close(fd: u64) -> i64 {
    if fd >= 3 && (fd as usize) < MAX_OPEN {
        unsafe {
            let f = &mut *core::ptr::addr_of_mut!(FILES[fd as usize]);
            if f.kind == F_KIND_PIPE {
                // M18/M20: 管道端点关闭
                let p = f.data_ptr as *mut crate::ipc::Pipe;
                let pi = crate::ipc::pipe_end_close(p);
                free_fd(fd as usize);
                // M20: 双端全关 -> 回收 Pipe 槽 (防泄漏)
                if let Some(idx) = pi {
                    crate::ipc::pipe_recycle(idx);
                }
                return 0;
            }
            // M16: 磁盘文件脏刷盘 (写穿)
            if f.kind == F_KIND_DISK && DISK_DIRTY {
                if crate::fjfs::write_file(b"hello.txt", core::ptr::addr_of!(DISK_CACHE).cast::<u8>(), DISK_CACHE_LEN) {
                    serial::write_line("vfs  : disk flush ok (/disk/hello.txt)");
                }
                DISK_DIRTY = false;
            }
            f.pos = 0;
            f.data_len = 0;
            f.data_ptr = core::ptr::null();
            f.name = "/dev/null";
            f.kind = 0;
        }
    }
    0
}

/// M29: lseek(fd, off, whence) — 文件位置跳转 (whence 0=SET, 1=CUR, 2=END)。
#[no_mangle]
pub extern "C" fn fujo_lseek(fd: u64, off: i64, whence: u64) -> i64 {
    unsafe {
        if fd < 3 || (fd as usize) >= MAX_OPEN {
            return -9; // -EBADF
        }
        let f = &mut *core::ptr::addr_of_mut!(FILES[fd as usize]);
        let base: i64 = match whence {
            0 => 0,
            1 => f.pos as i64,
            2 => f.data_len as i64,
            _ => return -22, // -EINVAL
        };
        let np = base + off;
        if np < 0 {
            return -22; // -EINVAL
        }
        f.pos = np as u64;
        np
    }
}

/// 文件写入 (fd>=3): 内存盘追加; /dev/tty 与 fd<3 由调用方走串口。
pub fn file_write(fd: u64, ptr: u64, len: u64) -> Option<i64> {
    unsafe {
        if fd < 3 || (fd as usize) >= MAX_OPEN {
            return None;
        }
        let name = FILES[fd as usize].name;
        if !name.as_bytes().contains(&b'/') && name.is_empty() {
            return None;
        }
        if name == "/dev/tty" {
            return None; // 调用方串口回退
        }
        if FILES[fd as usize].kind == F_KIND_PIPE {
            // M18: 管道写
            let p = FILES[fd as usize].data_ptr as *mut crate::ipc::Pipe;
            let n = crate::ipc::pipe_write(p, ptr, len);
            serial::write_str("ipc  : pipe write fd=");
            print_dec(fd);
            serial::write_str(" +");
            print_dec(n as u64);
            serial::write_line(" bytes");
            return Some(n);
        }
        if FILES[fd as usize].kind == F_KIND_DISK {
            // fujofs 缓存追加 (close 刷盘)
            let mut n = 0usize;
            while (n as u64) < len && DISK_CACHE_LEN < DISK_CACHE.len() {
                DISK_CACHE[DISK_CACHE_LEN] = (ptr as *const u8).add(n).read();
                DISK_CACHE_LEN += 1;
                n += 1;
            }
            DISK_DIRTY = true;
            serial::write_str("vfs  : disk cache fd=");
            print_dec(fd);
            serial::write_str(" +");
            print_dec(n as u64);
            serial::write_line(" bytes (dirty)");
            return Some(n as i64);
        }
        if FILES[fd as usize].kind == F_KIND_RAM {
            let mut n = 0usize;
            while (n as u64) < len && RAMDISK_LEN < RAMDISK.len() {
                RAMDISK[RAMDISK_LEN] = (ptr as *const u8).add(n).read();
                RAMDISK_LEN += 1;
                n += 1;
            }
            serial::write_str("vfs  : write fd=");
            print_dec(fd);
            serial::write_str(" +");
            print_dec(n as u64);
            serial::write_line(" bytes (ramdisk append)");
            return Some(n as i64);
        }
    }
    None
}
