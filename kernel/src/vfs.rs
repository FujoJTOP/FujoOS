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
pub const F_KIND_RAM: u8 = 2; // tmpfs 内存盘读写 (file 保留: data_ptr=entry.data, kslot=entry+1)
pub const F_KIND_DISK: u8 = 3; // fujofs 磁盘文件 (M16)
pub const F_KIND_PIPE: u8 = 4; // IPC 管道 (M18; data_ptr=Pipe*)
pub const F_KIND_MODEL: u8 = 5; // W12: 模型设备 /dev/model0 (写=请求, 读=响应)
pub const F_KIND_DIR: u8 = 6; // W18: 目录 fd (getdents64 枚举游标在 pos)

pub const MAX_OPEN: usize = 16;

#[derive(Clone, Copy)]
pub struct File {
    pub name: &'static str,
    pub kind: u8,
    pub data_ptr: *const u8,
    pub data_len: u64,
    pub pos: u64,
    /// M20: 登记的内核对象槽 (pipe 端点; 0=none);
    /// W12: tmpfs 条目槽 +1 (0=none)。
    pub kslot: usize,
    /// W20 p5: 磁盘文件短名 (F_KIND_DISK; close 刷盘按真实文件名,
    /// 取代 M16 占位 hello.txt)。
    pub disk_name: [u8; 16],
}

// 文件表: 0..=2 保留 (0=/dev/null, 1/2=/dev/tty 串口), 3.. 由 open 分配
static mut FILES: [File; MAX_OPEN] = [
    File { name: "/dev/null", kind: 0, data_ptr: core::ptr::null(), data_len: 0, pos: 0, kslot: 0, disk_name: [0; 16] };
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
/// W12: tmpfs 命名内存盘 (open /tmp/<name> 即建/开; 8 槽 × 2KiB)。
pub const TMPFS_N: usize = 8;
pub const TMPFS_MAX: usize = 2048;
#[derive(Clone, Copy)]
pub struct TmpEntry {
    pub name: [u8; 16],
    pub len: usize,
    pub data: [u8; TMPFS_MAX],
}
static mut TMPFS: [TmpEntry; TMPFS_N] = [
    TmpEntry { name: [0; 16], len: 0, data: [0; TMPFS_MAX] };
    TMPFS_N
];
/// W12: 模型设备响应缓冲 (读 /dev/model0 返回; 写时由 model 后端回填)。
static mut MODEL_BUF: [u8; 64] = [0; 64];
static mut MODEL_BUF_LEN: usize = 0;
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

/// 初始化: tmpfs 种子 (/tmp/hello.txt) + 生成 /proc/meminfo 内容。
pub fn init() {
    unsafe {
        // /tmp/hello.txt 种子 (tmpfs 槽 0)
        let seed = b"hello from FujoOS ramdisk (M15)\n";
        let mut nm = [0u8; 16];
        for (k, b) in b"hello.txt".iter().enumerate().take(15) {
            nm[k] = *b;
        }
        TMPFS[0].name = nm;
        TMPFS[0].len = seed.len();
        for (i, &b) in seed.iter().enumerate() {
            TMPFS[0].data[i] = b;
        }
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
        serial::write_str("vfs  : mounted [memory-backed + tmpfs] /boot/module len=");
        print_dec(blen);
        serial::write_line("; /proc/meminfo; /tmp/* (tmpfs); /dev/tty; /dev/model0");
    }
}

/// W12: tmpfs 查找/创建 (name 16B 内); 返回槽索引或 -1 表满。
fn tmpfs_find_or_create(name: &[u8]) -> i64 {
    unsafe {
        for k in 0..TMPFS_N {
            // 存在且同名
            let mut same = true;
            for i in 0..16 {
                let a = TMPFS[k].name[i];
                let b = if i < name.len() { name[i] } else { 0 };
                if a != b {
                    same = false;
                    break;
                }
            }
            if same {
                return k as i64;
            }
        }
        // 创建: 首个空槽
        for k in 0..TMPFS_N {
            if TMPFS[k].name[0] == 0 {
                let n = name.len().min(15);
                let mut nm = [0u8; 16];
                for i in 0..n {
                    nm[i] = name[i];
                }
                TMPFS[k].name = nm;
                TMPFS[k].len = 0;
                return k as i64;
            }
        }
    }
    -1
}

/// W18: tmpfs 只读查找 (stat 路径不得创建!); 返回槽索引或 -1。
fn tmpfs_lookup(name: &[u8]) -> i64 {
    unsafe {
        for k in 0..TMPFS_N {
            let mut same = true;
            for i in 0..16 {
                let a = TMPFS[k].name[i];
                let b = if i < name.len() { name[i] } else { 0 };
                if a != b {
                    same = false;
                    break;
                }
            }
            if same && TMPFS[k].name[0] != 0 {
                return k as i64;
            }
        }
    }
    -1
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

/// W12: /dev/model0 响应文本 "intent=<n>\n"。
fn format_intent(i: i64) -> [u8; 64] {
    let mut r = [0u8; 64];
    let t = b"intent=";
    let mut n = 0usize;
    for &b in t.iter() {
        r[n] = b;
        n += 1;
    }
    let v = i.max(0) as u64;
    let mut num = [0u8; 20];
    let mut di = 20usize;
    if v == 0 {
        r[n] = b'0';
        n += 1;
    } else {
        let mut x = v;
        while x > 0 {
            di -= 1;
            num[di] = b'0' + (x % 10) as u8;
            x /= 10;
        }
        while di < 20 {
            r[n] = num[di];
            n += 1;
            di += 1;
        }
    }
    r[n] = b'\n';
    r
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
/// M89: 文件写计数 (fujoctx 摘要面)。
static mut WRITES: u64 = 0;

pub fn fs_writes() -> u64 {
    unsafe { WRITES }
}

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

/// W15: 应用管理器 —— 注册表查询 (name -> (phys addr, len)); shell `os run NAME` 用。
pub fn lib_find(name: &str) -> Option<(u64, u64)> {
    let b = name.as_bytes();
    if let Some((a, l)) = fujo_lib_find(b) {
        return Some((a as u64, l as u64));
    }
    None
}

/// W15: 应用管理器 —— 列表导出 (用户态; 0x8B01)。
/// out 布局: +0 u64 count; 每项 24B = name[16] (NUL 终止) + addr u64。
#[no_mangle]
pub extern "C" fn fujo_app_list(out: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&out) {
        return -14;
    }
    unsafe {
        let o = out as *mut u8;
        let mut cnt = 0usize;
        (out as *mut u64).write(0u64);
        for i in 0..LIB_COUNT {
            let (nm, addr, len) = LIBS[i];
            if len == 0 {
                continue;
            }
            let e = o.add(8 + cnt * 24);
            for k in 0..16 {
                e.add(k).write(nm[k]);
            }
            (e.add(16) as *mut u64).write(addr);
            cnt += 1;
        }
        (out as *mut u64).write(cnt as u64);
        cnt as i64
    }
}

/// W15: tmpfs 名称列表 (shell ls 用)。
pub fn tmpfs_count() -> usize {
    unsafe {
        let p = core::ptr::addr_of!(TMPFS);
        let mut n = 0usize;
        for i in 0..TMPFS_N {
            if (*p)[i].len > 0 {
                n += 1;
            }
        }
        n
    }
}

pub fn tmpfs_name(i: usize) -> &'static str {
    unsafe {
        let p = core::ptr::addr_of!(TMPFS);
        if i < TMPFS_N && (*p)[i].len > 0 {
            let nm = &(*p)[i].name;
            let mut end = 16usize;
            for k in 0..16 {
                if nm[k] == 0 {
                    end = k;
                    break;
                }
            }
            core::str::from_utf8(&nm[..end]).unwrap_or("?")
        } else {
            ""
        }
    }
}

/// W15: 注册表访问 (shell app list 用)。
pub fn lib_count() -> usize {
    unsafe { LIB_COUNT }
}

pub fn lib_name_at(i: usize) -> &'static str {
    unsafe {
        static mut BUF: [u8; 16] = [0; 16];
        if i < LIB_COUNT {
            let (nm, _a, _l) = LIBS[i];
            let mut end = 16usize;
            for k in 0..16 {
                if nm[k] == 0 {
                    end = k;
                    break;
                }
            }
            for k in 0..end {
                BUF[k] = nm[k];
            }
            BUF[end] = 0;
            core::str::from_utf8(&BUF[..end]).unwrap_or("?")
        } else {
            static EMPTY: &str = "";
            EMPTY
        }
    }
}

pub fn lib_addr_at(i: usize) -> u64 {
    unsafe {
        if i < LIB_COUNT {
            LIBS[i].1
        } else {
            0
        }
    }
}

/// W16: 内核态写 tmpfs 文件 (shell mbuild 用; 覆盖/新建)。
pub fn write_kernel_file(path: &str, data: &[u8]) -> i64 {
    unsafe {
        if let Some(tname) = path.strip_prefix("/tmp/") {
            let idx = tmpfs_find_or_create(tname.as_bytes());
            if idx < 0 {
                return -28;
            }
            let n = data.len().min(TMPFS_MAX);
            let e = &mut *core::ptr::addr_of_mut!(TMPFS[idx as usize]);
            for k in 0..n {
                e.data[k] = data[k];
            }
            e.len = n;
            return n as i64;
        }
    }
    -2
}

/// W16: lseek(fd, off, whence) — 文件 pos 调整 (tcc 等静态 glibc 程序需要)。
/// (M29 已有 fujo_lseek; W16 仅补 syscall 分发 8 => 原函数)
pub fn read_kernel(fd: u64, out: &mut [u8]) -> usize {
    unsafe {
        if fd < 3 || (fd as usize) >= MAX_OPEN {
            return 0;
        }
        let f = &mut *core::ptr::addr_of_mut!(FILES[fd as usize]);
        if f.data_ptr as u64 == 0 {
            return 0;
        }
        let remaining = f.data_len.saturating_sub(f.pos);
        let n = out.len().min(remaining as usize);
        for k in 0..n {
            out[k] = core::ptr::read_volatile(f.data_ptr.add(f.pos as usize + k));
        }
        f.pos += n as u64;
        n
    }
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
        } else if let Some(tname) = name.strip_prefix("/tmp/") {
            // W12: tmpfs —— open 即建/开 (命名内存文件)
            let idx = tmpfs_find_or_create(tname.as_bytes());
            if idx < 0 {
                NEXT_FD -= 1;
                serial::write_line("vfs  : tmpfs full -ENOSPC");
                return -28;
            }
            f.name = "/tmp/";
            f.kind = F_KIND_RAM;
            f.data_ptr = core::ptr::addr_of!(TMPFS[idx as usize].data[0]);
            f.data_len = TMPFS[idx as usize].len as u64;
            f.pos = 0;
            f.kslot = idx as usize + 1;
            serial::write_str("vfs  : tmfs open /tmp/");
            serial::write_str(tname);
            serial::write_str(" idx=");
            print_dec(idx as u64);
            serial::write_line("");
            return fd as i64;
        } else if name == "/dev/tty" {
            f.name = "/dev/tty";
            f.kind = F_KIND_GEN; // 仅作为写通道; data_ptr 无意义
            f.data_len = 0;
            f.pos = 0;
            serial::write_line("vfs  : open /dev/tty (serial)");
            return fd as i64;
        } else if name == "/dev/model0" {
            // W12: 模型即设备 —— open 即就绪; 写=请求 (阻塞), 读=响应文本。
            f.name = "/dev/model0";
            f.kind = F_KIND_MODEL;
            f.data_ptr = core::ptr::addr_of!(MODEL_BUF) as *const u8;
            f.data_len = MODEL_BUF_LEN as u64;
            f.pos = 0;
            serial::write_line("vfs  : open /dev/model0 (model device)");
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
            // W20 p5: 记录短名 (close 按真实文件名刷盘)
            for k in 0..16 {
                f.disk_name[k] = if k < fname_bytes.len() { fname_bytes[k] } else { 0 };
            }
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
        } else if name == "/" || name == "/tmp" || name == "/dev" || name == "/proc" || name == "/boot"
                  || name == "/lib" || name == "/runres" || name == "/disk"
        {
            // W18: 目录 open (busybox opendir; 枚举游标在 pos)
            let dname: &'static str = match name {
                "/" => "/",
                "/tmp" => "/tmp",
                "/dev" => "/dev",
                "/proc" => "/proc",
                "/boot" => "/boot",
                "/lib" => "/lib",
                "/runres" => "/runres",
                _ => "/disk",
            };
            f.name = dname;
            f.kind = F_KIND_DIR;
            f.data_ptr = core::ptr::null();
            f.data_len = 0;
            f.pos = 0;
            serial::write_str("vfs  : open dir ");
            serial::write_str(name);
            serial::write_line("");
            return fd as i64;
        }
        NEXT_FD -= 1; // 回滚
        serial::write_str("vfs  : open unknown '");
        serial::write_str(name);
        serial::write_line("' -ENOENT");
        -2 // -ENOENT
    }
}

/// W18: stat 路径语义 (size 需真实; busybox ls 靠 st_mode 区分文件/目录)。
/// 返回 (st_mode, st_size); None = -ENOENT。
pub fn fujo_stat_path(name: &str) -> Option<(u32, u64)> {
    unsafe {
        // W18: "."/".." 后缀规范化 (busybox ls 对每条目 stat 相对路径)
        if let Some(b) = name.strip_suffix("/.").or_else(|| name.strip_suffix("/..")) {
            let base = if b.is_empty() { "/" } else { b };
            return fujo_stat_path(base);
        }
        match name {
            "/" | "/tmp" | "/dev" | "/proc" | "/boot" | "/lib" | "/runres" | "/disk" => {
                Some((0o040755, 0)) // S_IFDIR|0755
            }
            "/boot/module" => Some((0o100644, MODULE_COPY_LEN as u64)),
            "/proc/meminfo" => Some((0o100644, MEMINFO_LEN as u64)),
            "/dev/tty" | "/dev/model0" | "/dev/null" => Some((0o020666, 0)), // chr
            _ => {
                if let Some(tname) = name.strip_prefix("/tmp/") {
                    let idx = tmpfs_lookup(tname.as_bytes());
                    if idx >= 0 {
                        let t = core::ptr::addr_of!(TMPFS[idx as usize]);
                        Some((0o100644, (*t).len as u64))
                    } else {
                        None
                    }
                } else if let Some(lname) = name.strip_prefix("/lib/") {
                    if fujo_lib_find(lname.as_bytes()).is_some() {
                        Some((0o100644, 0))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }
}

/// W18: fstat 模式 (目录 fd -> DIR; 其余沿用 REG)。
pub fn fujo_fstat_mode(fd: u64) -> Option<u32> {
    unsafe {
        if fd >= 3 && fd < MAX_OPEN as u64 {
            let f = &*core::ptr::addr_of!(FILES[fd as usize]);
            if f.kind == F_KIND_DIR {
                return Some(0o040755);
            }
        }
    }
    None
}

/// W18: getdents64 (nr 217) —— 目录枚举 → linux_dirent64 流。
/// 条目: "." / ".." / 目录内容; f.pos 为游标 (下一次 2 时索引)。
/// dirent64: d_ino u64 | d_off i64 | d_reclen u16 | d_type u8 | d_name[]\0,
/// reclen = align8(19 + name_len + 1)。
#[no_mangle]
pub extern "C" fn fujo_getdents(fd: u64, buf: u64, len: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&buf) {
        return -14; // -EFAULT
    }
    unsafe {
        if fd < 3 || fd >= MAX_OPEN as u64 {
            return -9; // -EBADF
        }
        let f = &mut *core::ptr::addr_of_mut!(FILES[fd as usize]);
        if f.kind != F_KIND_DIR {
            return -20; // -ENOTDIR
        }
        let dir = f.name;
        let mut idx = f.pos as usize;
        let base = buf as *mut u8;
        let mut written = 0usize;
        loop {
            // 生成第 idx 项: (name_len, d_type, size); name 写入 nmbuf
            let mut nmbuf = [0u8; 32];
            let item = next_entry(dir, idx, &mut nmbuf);
            let (nlen, ty, _size): (usize, u8, u64) = match item {
                Some(x) => x,
                None => break,
            };
            let reclen = ((19 + nlen + 1 + 7) / 8) * 8;
            if written + reclen > len as usize {
                break;
            }
            let p = base.add(written);
            (p as *mut u64).write((idx + 1) as u64); // d_ino (非零)
            ((p.add(8)) as *mut i64).write((idx + 1) as i64); // d_off
            ((p.add(16)) as *mut u16).write(reclen as u16); // d_reclen
            ((p.add(18)) as *mut u8).write(ty); // d_type
            let dp = p.add(19);
            for i in 0..nlen {
                dp.add(i).write(nmbuf[i]);
            }
            dp.add(nlen).write(0u8);
            written += reclen;
            idx += 1;
        }
        f.pos = idx as u64;
        written as i64
    }
}

/// W18: 第 n 项 (0="." 1="..") 内容枚举; name 写入 out, 返回 (len, d_type, size)。
fn next_entry(dir: &str, n: usize, out: &mut [u8; 32]) -> Option<(usize, u8, u64)> {
    if n == 0 {
        out[0] = b'.';
        return Some((1, 4, 0)); // DT_DIR
    }
    if n == 1 {
        out[0] = b'.';
        out[1] = b'.';
        return Some((2, 4, 0));
    }
    let k = n - 2;
    unsafe {
        if dir == "/" {
            let roots: [(&[u8], u8); 7] = [
                (b"tmp", 4),
                (b"dev", 4),
                (b"proc", 4),
                (b"boot", 4),
                (b"lib", 4),
                (b"runres", 4),
                (b"disk", 4),
            ];
            if k < roots.len() {
                copy_into(out, roots[k].0);
                return Some((roots[k].0.len(), roots[k].1, 0));
            }
            return None;
        }
        if dir == "/tmp" {
            if k >= TMPFS_N {
                return None;
            }
            let t = &*core::ptr::addr_of!(TMPFS[k]);
            if t.name[0] == 0 || t.len == 0 {
                return None;
            }
            let mut end = 0usize;
            while end < 16 && t.name[end] != 0 {
                end += 1;
            }
            for i in 0..end {
                out[i] = t.name[i];
            }
            return Some((end, 8, t.len as u64)); // DT_REG
        }
        if dir == "/dev" {
            match k {
                0 => {
                    copy_into(out, b"tty");
                    Some((3, 2, 0)) // DT_CHR
                }
                1 => {
                    copy_into(out, b"model0");
                    Some((6, 2, 0))
                }
                2 => {
                    copy_into(out, b"null");
                    Some((4, 2, 0))
                }
                _ => None,
            }
        } else if dir == "/proc" {
            if k == 0 {
                copy_into(out, b"meminfo");
                return Some((7, 8, MEMINFO_LEN as u64));
            }
            None
        } else if dir == "/boot" {
            if k == 0 {
                copy_into(out, b"module");
                return Some((6, 8, MODULE_COPY_LEN as u64));
            }
            None
        } else if dir == "/lib" {
            if k < lib_count() {
                let nm = lib_name_at(k);
                let by = nm.as_bytes();
                let n = by.len().min(31);
                for i in 0..n {
                    out[i] = by[i];
                }
                return Some((n, 8, 0)); // DT_REG, size 未记 (lib 条目少用)
            }
            None
        } else {
            None // /runres /disk 枚举留空
        }
    }
}

fn copy_into(out: &mut [u8; 32], s: &[u8]) {
    for i in 0..s.len().min(31) {
        out[i] = s[i];
    }
}

/// 系统调用 read(nr0): fd -> 用户缓冲区。
#[no_mangle]
pub extern "C" fn fujo_read(fd: u64, buf: u64, len: u64) -> i64 {
    if len == 0 {
        return 0; // W16: read(fd, NULL, 0) 必须返回 0 (tcc .o 解析)
    }
    if !(0x400000..0xC00000).contains(&buf) {
        return -14; // -EFAULT
    }
    unsafe {
        if fd < 3 || fd >= MAX_OPEN as u64 {
            return -9; // -EBADF
        }
        let f = &mut *core::ptr::addr_of_mut!(FILES[fd as usize]);
        if f.kind == F_KIND_DIR {
            return -21; // -EISDIR (W18: 目录读)
        }
        if f.kind == F_KIND_PIPE {
            // M18: 管道读 (data_ptr = Pipe*)
            return crate::ipc::pipe_read(f.data_ptr as *const crate::ipc::Pipe, buf, len);
        }
        if f.kind == F_KIND_MODEL {
            // W12: 模型设备读 —— 返回最近一次请求的响应文本。
            if MODEL_BUF_LEN == 0 {
                return 0;
            }
            let n = (len.min(MODEL_BUF_LEN as u64)) as usize;
            for k in 0..n {
                core::ptr::write_volatile(
                    (buf as *mut u8).add(k),
                    core::ptr::read_volatile(MODEL_BUF.as_ptr().add(k)),
                );
            }
            return n as i64;
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
            // M16: 磁盘文件脏刷盘 (写穿; W20 p5: 真实短名, 取代占位 hello.txt)
            if f.kind == F_KIND_DISK && DISK_DIRTY {
                let mut end = 16usize;
                while end > 0 && f.disk_name[end - 1] == 0 {
                    end -= 1;
                }
                if end > 0
                    && crate::fjfs::write_file(
                        &f.disk_name[..end],
                        core::ptr::addr_of!(DISK_CACHE).cast::<u8>(),
                        DISK_CACHE_LEN,
                    )
                {
                    serial::write_str("vfs  : disk flush ok (/disk/");
                    for &c in f.disk_name.iter().take(end) {
                        // 短名回显 (仅 ASCII)
                        serial::write_str(if c < 0x80 {
                            unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(&c as *const u8, 1)) }
                        } else {
                            "?"
                        });
                    }
                    serial::write_line(")");
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
        WRITES += 1;
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
        if FILES[fd as usize].kind == F_KIND_MODEL {
            // W12: 模型设备写 = 请求 (R5 规则优先 -> 模型 -> 兜底; 阻塞一次往返)
            let mut text = [0u8; 64];
            let n = (len.min(64)) as usize;
            for k in 0..n {
                text[k] = (ptr as *const u8).add(k).read_volatile();
            }
            let intent = crate::ai::model_classify_intent(&text[..n]);
            let resp = format_intent(intent);
            MODEL_BUF_LEN = resp.len();
            for (k, &b) in resp.iter().enumerate() {
                MODEL_BUF[k] = b;
            }
            // 同步该 fd 的响应视图 (read 直接读 MODEL_BUF, data_len 仅提示)
            FILES[fd as usize].data_len = MODEL_BUF_LEN as u64;
            return Some(n as i64);
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
            // W12: tmpfs 条目追加写 (entry.len 推进; fd 视图同步)
            if FILES[fd as usize].kslot == 0 {
                return None;
            }
            let entry = &mut *core::ptr::addr_of_mut!(TMPFS[FILES[fd as usize].kslot - 1]);
            let mut n = 0usize;
            while (n as u64) < len && entry.len < TMPFS_MAX {
                entry.data[entry.len] = (ptr as *const u8).add(n).read();
                entry.len += 1;
                n += 1;
            }
            FILES[fd as usize].data_len = entry.len as u64;
            serial::write_str("vfs  : tmpfs write fd=");
            print_dec(fd);
            serial::write_str(" +");
            print_dec(n as u64);
            serial::write_line(" bytes");
            return Some(n as i64);
        }
    }
    None
}
