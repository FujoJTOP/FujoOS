# 08 · Roadmap 100(M11 → M100)长期路线

> 战略:当前已完成"纵向切片"(启动→装载→ring3→syscall→模型链路→Shell→Hermes→Qwen)。
> 接下来 **先补三个地基(虚拟内存/调度/VFS),再往上堆**(兼容层→图形交互→游戏性能→
> 工具链→AI OS 深化→交付)。原则不变:每步"可运行、可观测、可回退"—— 串口日志证据
> 链 + QEMU 验证 + 断言,完成 = 有验证记录。
> AI 层与内核并行:宿主模型链路(Qwen/Hermes)持续壮大,不必等内核完备。

## Wave 1 · 地基三件套(M11–M20)—— 决定一切的上限

- [x] **M11 虚拟内存/堆**:页表 U 位硬化(内核页用户不可及)、恒等堆预留 0x800000–0xC00000、
      `brk`(nr12)/`mmap`(nr9)/`munmap`(nr11) 原语、ring3 分配测试(brk 1MiB + mmap 2MiB + 模式回读零错)
- [x] **M12 缺页处理**:按需零页(堆区 P=0, 首写逐页分配零帧→PTE→指令重试, 进程继续)、
      #PF 专用桩全寄存器保存 + 进程级分发(未处理才停机);帧分配器(位图 16–63MiB);
      COW 种子随 M14 fork 一并落地(帧分配器/写时复制机制就绪)
- [x] **M13 抢占调度 + 线程**: PIT 时间片轮转(100Hz) —— task 结构/独立内核栈/
      TSS.rsp0 动态切换/保存帧切换; `os run threads` 双任务交替运行;
      **m12 期间修复**: STAR RPL(用户真回 CPL3, U-guard 生效)、PIT 桩 EOI 毁 RAX、
      中断帧字节序确证; **M14 收尾**: 0x5105 任务 id 原语替代栈地址身份
- [x] **M14 多进程/崩溃隔离**: 用户态致命 #PF → `terminate_current_and_next`(TASK_DEAD
      + 转场幸存者帧, pf 桩双退出路径) —— **"一个进程崩溃不影响其他" 实测闭环**:
      `task 1 about to CRASH → proc: task 1 terminated → task 0 持续运行`;
      fork/exec 雏形 = M14b(进程克隆/镜像重载, 下一刀)
- [x] **M15 VFS + 内存盘**: 文件表(fd≥3)+ 内存文件系统 —— `/boot/module`(initrd 拷入
      内核缓冲)、《/proc/meminfo》(引导时生成真值)、《/tmp/hello.txt》(内存盘读写、
      write 追加+回读)、《/dev/tty》(串口); Linux ABI open(2)/read(0)/close(3) 直通;
      实测: meminfo read=77 / ramdisk write+append readback=43 / module 32B=ELF 头 /
      ENOENT=-2; **修复: 镜像 1.18MB 超 0x120000 装载范围(尾部覆盖模块区/bss 损坏
      → bad magic + 切片恐慌) → load_end/bss_end=0x230000 + pad 0x130000
- [x] **M16 fujofs**: 极简 FAT-like 持久化文件系统 —— ATA PIO 驱动(IDE 主通道
      IDENTIFY+LBA28)、FJFS(superblock/簇位图/根目录 32x32B/连续分配/写穿)、
      VFS `/disk/<file>`(载入缓存/脏刷盘); **重启持久化实测通过**:
      boot#1 写 `FJFS persistent data #1`(24B)→ 重启 → boot#2 读回 24B +
      追加 `seen-boot2` → 读回 35B; 修复: BITMAP 256B 被扇区读溢出
      (512B) -> 512B; 目录项 28B->32B
- [x] **M17 .run 升级**:资源节(图标/清单/权限声明)+ 多文件打包 —— fujopack
      `--res NAME:PATH` / `--perm X` 多资源打包(TAG_DATA 节),内核 `fujr.rs`
      装载器(FNV-1a 逐节校验/MANIFEST 行扫描/EMBED 提取/DATA 拷内核静态),
      VFS `/runres/<name>` 只读后端;**容器集成实测通过**: `os run hermes`
      装载 res_test.run(4 节: MANIFEST/EMBED/DATA×2)→ 权限声明 `read:disk`
      审计 → `/runres/icon.bin` 64B 逐字节回读(0x01..0x10) →
      `/runres/hello.txt` 全文 `resource hello from .run (M17)` → **PASS**;
      **修复链**: 资源名解析误取应用名(改为只扫 `"resources":[` 段内 `"name"` 键)
      + 键闭引号偏移(值起始 = i2+6 → 冒号被当名字)+ 新增异常帧精确解码
      (桩经 trampoline 传帧指针, 解码 RIP/CS/RFLAGS/RSP 定位) →
      实证 **用户栈初始 RSP 需 %16==8**(clang `_start` 按 SysV call 约定布局,
      `movaps` 16 对齐访问, 0x600000(%16==0) 错位 → #GP; STACK=0x5FFFF8)
- [x] **M18 IPC 原语**:管道/共享内存/信号 —— 内核 `ipc.rs` 三件套:
      `fujo_pipe`(环形缓冲 512B, fd 表 F_KIND_PIPE 登记, read/write/close 统一路径)、
      `fujo_shm`(固定共享窗口 0xA00000/64KiB)、`fujo_sigset/sigkill/sigret`;
      信号投递 = PIT 用户态中断帧上构造 iretq 帧([RIP][CS][RFLAGS][RSP][SS])
      + RIP 改写为 handler(裸 asm, push/pop 保存现场 + `iretq` 恢复被中断点);
      **双任务集成实测通过**: `os run ipc` → A 建管道写 30B 消息 → B 读回全文
      ("ipc: hello through pipe (M18)") → A 写 32B 模式入共享窗口 → B 校验 OK →
      B 发信号 → 内核投递(A handler 计数=1) → A/B 均 **PASS**;
      修复: 用户 fd 数组须 int(内核写 2×u32, long 读成 0x400000003)、
      信号握手时序(shm[5] 完成栅栏); 桩表全向量改走 trampoline
      捕获精确异常帧(见 M17 记录)
- [x] **M19 内核对象/句柄表**:统一资源抽象 —— 内核 `kobj.rs` 类型化对象表
      (kind: FILE/PIPE/SHM/SIG + epoch 代序防悬垂 + payload; 64 槽全局单表 v0):
      `fujo_kobj_create/free`(0x5130/0x5131) + `fujo_kobj_info`(0x5132 统计);
      管道(每端×2)/共享窗口/信号注册自动接入, 生命周期可见; 
      **对象表集成实测通过**: `os run kobj` → 基线全 0 → 创建 pipe(2)+shm(1)+sig(1)
      +手工 FILE(1) → 统计 file=1 pipe=2 shm=1 sig=1 (计数逐一断言) →
      free/close → 最终 file=0 pipe=2 shm=1 sig=1 → **PASS**;
      说明: 对象表为 M14b 每进程句柄表的前身, fd 与 kobj 并行走 VFS 路径
- [x] **M20 稳定性**:进程级异常恢复;无泄漏运行 —— ① **异常桩重构**:
      所有异常向量(0..14)经 trampoline 全寄存器保存(PIT/#PF 同布局:
      regs[0..8]=r11..rax, [9]=栈内返回地址陷阱 → 帧头在 10+e) →
      用户态异常(#GP/#UD/#DE...)走 M14 崩溃隔离转场,不再整机停机;
      **实测**: B 任务 `ud2` → `EXC user vec=6 rip=0x40007d → task 1
      terminated` → A 任务继续运行 → **M20 RESULT: PASS**
      ② **无泄漏**: fd 槽扫描复用(取代恒增 NEXT_FD → EMFILE 泄漏)、
      pipe 双端关闭回收(Pipe 槽+kobj, ends 引用计数)、kobj 登记偏移
      (kslot=slot+1, 0=无登记与槽 0 冲突 → 首端永不释放的 M20 实证泄漏)、
      kobj 日志节流(前 16 次); **实测**: 128 轮 pipe 创建/写读/双端关闭 +
      512 轮 kobj 创建/释放 → 每 32 轮 pipe 计数回落 0 → **PASS (no leak)**;
      ③ **回归修复**: demand-zero 现覆盖"用户读未分配页"(Linux 语义,
      原仅 write; B 共享页先读 → 误判崩溃), M18/M19 回归 PASS;
      ④ **装载范围**: 镜像 1.38MB>0x230000 → load/bss_end=0x260000 +
      pad 0x160000(QEMU multiboot 精确读 load_end-load_addr,
      超文件大小 → fread() failed); 约束: 两值必须同步改

## Wave 2 · 兼容层加深(M21–M35)

- [x] **M21** linuxsubsys syscall 面扩展(~20 个常用): stat/fstat/lstat
      (mode=REG|0644 结构回填)、writev(iovec 逐段)、access=0、pipe(22 号
      接 M18 内核实现)、nanosleep(v1 no-op: SFMASK 屏蔽 IF 内核态不可等时,
      真实睡眠待调度器 M22+)、uname(FujoOS/FujoKernel/x86_64 回填)、
      gettimeofday/time(PIT 单调钟 sec/usec)、uid/gid/euid/egid=1000、
      arch_prctl=0、gettid=任务 id+1、futex=0、openat(转发 open)、
      getrandom(PIT 混哈希伪熵); **实测通过**: 14 项逐一断言,
      用户态忙等 80ms 时间推进显式验证 → **M21 RESULT: PASS**;
      说明: 全部 syscall 带用户指针区域检查(低区+darwin 区)返回 -EFAULT
- [x] **M22** linuxsubsys fork 直通(连到调度器): `fork(nr57)` —
      内核 `sched::fork_current` 克隆: 用户栈物理拷贝(0x600000→0x700000,
      64KiB 上限)+ 独立内核栈(父 0x380000 / 子 0x340000)+ 共享地址空间
      (v0: 无每进程页表; 子从 fork 返回 0 (rax 槽), 父返回子 tid),
      PIT 轮转两任务; **实测通过**: `os run fork` →
      父 tid=1 / 子返回 0(子记录 tid=1) → 子写共享标记 0x5A5B →
      父读回验证 → **M22 RESULT: PASS**;
      **修复链**: syscall 帧解码(rcx=用户返回 RIP 在 args[6])、
      父登记不再构造帧(写 0x300000 覆盖 dispatch 返回帧 → 内核 #UD rip=3)、
      父帧被子 syscall 覆盖(iretq 目标帧垃圾 → #GP tRIP=0x2fffd8;
      修: 父 set_rsp0 到独立栈 0x380000)、子栈 0x240000 与内核 BSS 重叠
      (BSS 到 0x24BCA8; 修: 0x340000); execve 留 M23
- [x] **M23a** 静态 busybox 装载+argv/auxv 栈帧(完整运行=M23b): ——
      下载 ubuntu busybox-static 1.30.1(2.1MB 静态 ELF, 段 0x400000..0x620000);
      内核: `os run busybox`(argv 模式) + 用户栈构造完整 Linux 进程栈
      [argc][argv][0][envp][0][auxv: AT_PHDR/PHENT/PHNUM/ENTRY/SECURE/RANDOM/
      PAGESZ][0] (0x5F0000 区) + `fujo_enter_user` 用户入口通用寄存器清零
      (glibc _start 契约: rdx=rtld_fini=0) + 启动 syscall 面补齐
      (arch_prctl GET_FS/157/218 set_tid/273 robust/274 get_robust/334 rseq/
      10 mprotect); **验证**: busybox ELF 装载 entry=0x40b300, 段复制逐段确认,
      argv 帧 demo(argc=1, argv[0]="busybox") **PASS**, 用户态执行流进入
      glibc init(用户 RIP 推进); **未完项(M23b)**: glibc 静态 init 深链路
      (TLS/更多 syscall 直通) 使 busybox 达 usage 输出 —— 待 M24/M25 输入
- [x] **M23b** busybox 完整原生运行(验收: ls/cat/echo/管道) ——
      **musl 静态 busybox**(alpine busybox-static 1.36.1-r31, 1.0MB 静态 ELF,
      段 0x400000..0x4FC1E8)在 FujoOS **原生执行命令**: `os run busybox` →
      显示版本/Usage 帮助; `os run busybox echo hello` → **输出 hello (PASS)**;
      架构: 用户进程栈 [argc][argv…][0][envp][0][auxv: AT_PHDR/PHENT/PHNUM/
      ENTRY/RANDOM/PAGESZ][0] 完整构造(0x5F0400 指针区 + 0x5F0C00 字符串区),
      shell `os run busybox <cmd> <args...>` 解析 argv[1..];
      **修复链**: musl 已取代 glibc 静态(glibc 需 TLS/termios 深层, 记为 M25k
      后续); write/mmap 等用户指针检查放宽到 0xC00000(堆/mmap 区);
      argv 字符串放置顺序(逆序防覆盖) + NUL 终结(len=end+2)两个实证 bug;
      剩余 syscall: nr14(rt_sigprocmask)/nr106(mmap 相关) 被优雅忽略;
      **下一步(M24/M25)**: 编译自带 musl 静态 hello 直跑 → 符号表/动态链接
- [x] **M24** ELF 动态链接最小化(interp + 符号表) —— **ET_DYN + PT_INTERP 装载**:
      `elf_loader` 接受 e_type=3(ET_DYN)并识别 PT_INTERP
      (`elfx : PT_INTERP present (ld.so path recognized)`), 装载段过滤
      v<0x100000(防破坏内核低址); **动态 ELF**(clang -fPIC -q 链接,
      含 PT_INTERP/PT_DYNAMIC 记录, 非 PIE 动态)原生执行:
      `os run busybox` → **`hello from dynamic ELF (M24)`** → sys_exit 正常;
      说明: v0 为"链接期已定址的动态 ELF"(段按 p_vaddr 装载, 无需真 ld.so;
      R_X86_64_PC32 文本内相对引用装载后不变); 真 PIE(+ld.so 符号解析)
      列 M24.5/M28 输入
- [x] **M25** musl/glibc hello 直跑 —— **musl 1.2.5 hello 原生运行**:
      alpine musl-dev(libc.a 9.4MB 但 strip-debug 后 2.7MB 可链;
      crt1/crti/crtn 同样 strip) + LLVM lld 静态链接
      (`-T user.ld` 基址 0x400000; 注意 musl 默认基址 0x200000
      与内核保留区冲突 -> 用 user.ld 覆盖); 运行时 **puts/printf/strlen/
      exit 全走 musl libc 静态代码路径**; **实测输出**:
      `hello from musl on FujoOS (M25)` + `libc: musl 1.2.5 (len=10)`
      + exit(42) 正确返回(末尾 stdout flush 缓冲指针超界日志为
      printf EXIT 前 fflush 残留, 不影响) → **M25 达成**
- [x] **M26** winsubsys:kernel32 文件 IO/堆/线程垫片家族 ——
      **PE32+ winsubsys 程序(mingw 编译)原生运行**: `os run win` →
      `m26_win.exe`(PE32+, 子系统=windows-cui) 经 PE 装载 → kernel32 垫片
      trampoline(0x20 对齐, push rax/rcx + mov 转 syscall 参数 + syscall +
      pop/ret) 直通内核: **WriteFile**(屏幕输出)、**ReadFile**(stdin
      fd=3 读 32 字节)、**GetFileSize**(PE 文件字节数回填)、
      **GetCurrentThreadId**、**CloseHandle** → **M26 RESULT: PASS**;
      架构: `shim_syscall_nr` 表扩展 6 符号(kernel32.def),
      通用 32 字节槽生成器(imm32 @14..17, 数组 32 元素);
      OS 内句柄 = linux 直通 fd(WriteFile→user_write / ReadFile→fujo_read);
      **修复链**: trampoline stub imm32 写位(误 18..21 → #PF cr2=0x1000000)
      与旧二进制缓存(entry 0x21f230 确认); 构造: msvcrt 无需(裸
      WinMain/GetStdHandle 直链 kernel32.def); 回归: M3 hello_win.exe
      (WriteFile/ExitProcess) 仍 PASS
- [x] **M27** mingw 控制台程序原生运行 ——
      **mingw-w64 real CRT 程序**(x86_64-w64-mingw32-gcc 16.1, `mainCRTStartup`
      + 静态 mingwex libc, `--image-base 0x400000 -s`)在 FujoOS 原生执行:
      `os run win` → **`m27: mingw console app alive` / `argc=1` /
      `argv[0]=C:\...\m27_mingw.exe` / `heap works` / `M27 RESULT: PASS`** →
      `msvcrt exit(7)` 内核接管;51 全导入绑定(23 kernel32 + 3 数据导入 +
      32 msvcrt(28 码 + 数据 cell));
      架构: **表驱动垫片注册表**(55 蹦床槽 + 通用 no-op 槽)、**GS 基址
      = 假 TEB**(0x7E1000, mingw 启动读 gs:[0x30]=Self→[Self+8]=StackBase,
      由 fujo_enter_user iretq 前写 MSR_GS_BASE)、**__getmainargs**
      回填 argc/argv/envp(用户态 0x7E0400 帧)、**__iob_func/FILE[2]/
      _errno/lconv** 用户态数据块(0x7E0000..0x7E3000)、**msvcrt
      malloc/calloc/free**(内核 bump 0x800000 起)、**mini printf 引擎**
      (vfprintf va=char* 参数数组)、Win64 CRT 无需 getmainargs 之后
      自 argv-dup 全通;
      **修复链(两处 trampoline 实证缺陷)**: ① pop rax 把 syscall 返回值
      覆盖成调用方原 rax(M26 没查返回值所以未暴露; malloc 返 16 = 参数
      区大小) → 改跳过 rax 槽保留结果; **② trampoline 破坏 Win64
      callee-saved rdi/rsi**(mingw main 把 argc 存 esi 越过 puts 垫片后
      被 mov rsi,rdx 覆盖 → argc 打印 0x800000/3 漂移) → push/pop rdi/rsi;
      ③ GetProcAddress 返回已知垫片蹦床(动态解析 msvcrt 导出)/未知
      通用 no-op;
      **回归**: M26(m26_win ReadFile 32 bytes/GetFileSize=3072 转真实
      语义, CloseHandle BOOL 化, fd=3 预打开 /boot/module)与 M3
      (hello_win) 全 PASS
- [x] **M28** vcruntime 最小集 —— **vcruntime/msvcrt 函数面扩展**(mingw
      16.1 CRT 10 个新导入全通): `os run win`(m28_vc.exe, 依赖
      strtol/strtoul/strtod/atoi/atof/qsort/_snprintf/rand/srand/memmove/
      toupper/isdigit 等)→ **`m28: strtol=12345 strtoul=31` /
      `strtod=3.75 atoi=42 atof=1.5 rand=26` / `q=0,3,6,9 12345` /
      `memmove=vcruntime chr='w' toupper=A isdigit=1` /
      `M28 RESULT: PASS`** → exit(7);
      实现: 垫片表 +10 符号(0x5222..0x522B: _snprintf 渲染到用户缓冲
      (栈 params 读 [user_rsp+0x40/0x48/0x50/0x58])、strtol/strtoul
      (base 0/8/10/16 + endptr)、atoi、memset、rand/srand(LCG)、
      toupper、**atof 专用蹦床**(Win64 浮点返回 XMM0: syscall 后
      `movsd xmm0,[cell]`, 0x7F0E00)、**qsort 内核实现 + Win64 ABI
      用户回调桥** `fujo_call_win_fn`(CPL0 直接 call 用户 cmp 指针,
      rcx/rdx 双参 + 32B shadow + 16 对齐));
      **修复链**: 回调桥 `mov rsp,rax` 恢复栈被用户返回值覆盖(首次
      cmp 差 3 → rsp=3 → #UD rip=0x3) → 改 rbx 保存原 rsp;
      回归: M27 全绿
- [x] **M29** darwinsubsys:libSystem 薄层;darwin CLI 工具 —— **BSD
      syscall 面补齐**(0x2000000|nr): read(3)/open(5)/close(6)/
      lseek(13)/mmap(197, darwin flags MAP_PRIVATE|MAP_ANON=0x1002 →
      内核集 0x22)/getpid(20)/exit(带 code)接 VFS/mem 直通;
      vfs 新增 `fujo_lseek`(whence SET/CUR/END, 负拒 -EINVAL);
      **darwin CLI 工具**(sdk/mac/m29_darwin.macho, clang
      --target=x86_64-apple-macos11 -nostdlib, 零 libc 手工 BSD
      syscall): **`m29: darwin CLI tool - libSystem shim layer`** →
      `open fd=3` → `read n=32 first8=cffaedfe07000001`(Mach-O 自身魔数
      回读, LE) → `lseek pos=0` → `mmap=8388608` + 写验证 →
      `getpid=2` → `close=0` → **`M29 RESULT: PASS`** → `darwin exit(7)`;
      回归: M27 全绿
- [x] **M30** 三子系统一致化(统一内核对象映射) —— **三 ABI 同一 VFS
      对象路径**(/boot/module): win(darwin/linux 同源流程)各以
      **open→read 32B→校验自身魔数→close**, 全部走 `vfs::fujo_open_name`
      (linux fd、darwin fd、win32 句柄 = 同一 fd 表):
      **win**(m30_win.exe, clang MSVC: **CreateFileA("\\boot\\module")**
      垫片反斜杠归一→fd, `ReadFile`→`4d5a78…`(自身 PE MZ)) **PASS**;
      **linux**(m30_linux.elf 零 libc, open→fd3→`7f454c46`(ELF 魔数))
      **PASS**; **darwin**(m29 回归: open→fd3→`cffaedfe`(Mach-O 魔数))
      **PASS**; 三程序同里程碑同一对象语义 → **M30 达成**;
      架构: kernel32!CreateFileA(0x5018) 垫片(路径 7 参只有 name 用,
      反斜杠→正斜杠)+ vfs 重构 `fujo_open_name(name)` 公共入口
- [x] **M31** fujopack 资源化 .run 命令行工具链 —— **宿主端 Python 工具
      `tools/fujopack.py`**(pack/info/check): FUJR v0.1 容器
      (64B 头 + 32B×n 节表 + 4096 对齐 payload, FNV-1a 校验, 节 tag
      1=MANIFEST/4=EMBED/5=DATA) 一键打包 **可执行体+资源**:
      `fujopack pack -e EXEC -r name:file -o out.run` /
      `check`(全量 hash)、`info`(节表); 内核 `fujr::load` 全流程:
      **`FUJR container ok (sections=3)`** → `perm claim: perms runres:read
      (audited)` → `resource #0 name=demo.txt` → `exec extracted`;
      `os run hermes`(m31_res.run)→ **`m31: resource content: M31
      resource demo — packed by fujopack.py`** → **`M31 RESULT: PASS`**;
      工具链闭环: 编译 ELF → fujopack 打包 → 内核解包 → /runres 资源读取
- [x] **M32** fujorun 支持多模块/库目录 —— **宿主工具
      `tools/fujorun.py`**(pack/run): BootMulti v1 多模块镜像
      ("FUJOMULT" + count + (off/len/name[16])×n, 8 对齐),
      单 initrd 装载 **可执行体 + 库/资源模块**; 内核解析:
      **`multi: exec module 'main'`** / **`multi: lib module 'catlib.bin'
      -> /lib`** → 主模块格式嗅探执行, 库模块挂 **vfs `/lib/<name>`**
      (库表 LIBS[8] 注册, `fujo_open_name` /lib/ 匹配 blob);
      `os run hermes`(m32_multi.initrd: ELF+catlib.bin)→
      **`m32: lib content: CATLIB-BIN: library module payload from
      fujorun (M32)`** → **`M32 RESULT: PASS`**
- [x] **M33** 系统调用追踪(trace 工具 + 计数) —— **内核 trace 面**:
      fujo 原生 0x5301 `trace_enable(on)` / 0x5302 `trace_show()`
      (ring 尾部 16 条 nr/a0/tick + 非零计数 12 项) / 0x5303
      `trace_count(nr)`; 环形缓冲 64×3 (nr, a0, PIT tick) + 计数表
      256 (nr%256), 默认关(零开销), 开启后每 syscall 登记不改分发;
      **工具/演示**(m33_trace.elf): enable 后 open/read/close/write 序列
      → `trace_count(write)>=3 / open>=2 / close>=2` 校验 →
      trace_show 输出(rng: `nr=1 (write) a0=1 …` / `nr=21251 (trace_count)
      a0=2/3` / counts: read=2 write=11 open=3 close=5) →
      **`M33 RESULT: PASS`**
- [ ] **M34** 兼容矩阵回归(三格式 × 三子系统, 自动化)
- [ ] **M35** 性能基准:syscall 延迟/切换开销表

## Wave 3 · 图形与交互(M36–M50)—— UI 从"演示"变"可用"

- [ ] **M36** 鼠标驱动(PS/2)+ 命中测试/焦点
- [ ] **M37** 消息环(win32k 等价):消息队列、窗口类、z-order
- [ ] **M38** 窗口管理:重叠/焦点/拖动/关闭
- [ ] **M39** 字体升级:更多字形/缩放/抗锯齿
- [ ] **M40** IME 预留(中文输入框架骨架)
- [ ] **M41** fujokit v0:按钮/文本框/列表控件
- [ ] **M42** GUI 应用#1:一个可点按钮的窗口(验收)
- [ ] **M43** 剪贴板/拖放雏形
- [ ] **M44** 图标/主题/调色板系统
- [ ] **M45** 终端窗口控件(串口/VGA 转 GUI)
- [ ] **M46** 桌面环境:任务栏/开始菜单雏形
- [ ] **M47** 多屏/分辨率切换(vbe 枚举)
- [ ] **M48** 输入法候选窗+fujokit 集成
- [ ] **M49** 无障碍:高对比/大字模式
- [ ] **M50** GUI 基准:窗口开关/拖动帧率表

## Wave 4 · 游戏与性能(M51–M70)

- [ ] **M51** 显示驱动抽象:QEMU virtio-gpu 接入
- [ ] **M52** 音频:AC97/HDA 驱动 + 播放入口
- [ ] **M53** XInput 式输入抽象(手柄/键鼠统一)
- [ ] **M54** 定时器:高精度/帧同步(rdtsc 基)
- [ ] **M55** 原生 fujogl v0(OpenGL 1.x 子集, 软件后端)
- [ ] **M56** DXVK 式翻译可行性评估(方案+原型)
- [ ] **M57** KVM 加速支持 + TCG/KVM 基准对照
- [ ] **M58** 2D 游戏#1 原生运行(开源 2D 引擎选择)
- [ ] **M59** 游戏模式:前台调度/资源预留/全屏
- [ ] **M60** 存档沙箱(权限目录+版本化)
- [ ] **M61** 图形加速:blit/缩放硬件路径
- [ ] **M62** 着色器内核评估(compute 子集)
- [ ] **M63** 音频混音器/效果链
- [ ] **M64** 多核并行:调度亲和/负载均衡 v0
- [ ] **M65** 每核 TSS/中断注入优化
- [ ] **M66** 页缓存/预读(内存盘→真盘)
- [ ] **M67** 中断合并/减轻(串口/网卡)
- [ ] **M68** 帧时间表/性能计数器工具
- [ ] **M69** 2D 游戏#2 + 输入延迟基准
- [ ] **M70** 游戏层性能验收报告

## Wave 5 · 开发工具链(M71–M85)

- [ ] **M71** 系统内汇编器(最小 .s 编译)
- [ ] **M72** 系统内链接器(ELF 静态最小)
- [ ] **M73** 迷你编辑器(vi 子集)
- [ ] **M74** fujocc 编译壳(表驱动, 跨 ABI 选项)
- [ ] **M75** 调试器 v0:单步/断点(调试寄存器)
- [ ] **M76** syscall trace 工具化(打开后台记录)
- [ ] **M77** 性能计数器(rdtsc/中断计数窗口)
- [ ] **M78** CI:QEMU 无头启动 + 日志断言自动化
- [ ] **M79** fujopack/fujorun 命令全参数化 + 手册
- [ ] **M80** SDK 文档闭环(示例/模板/教程)
- [ ] **M81** 交叉编译工具链一键脚本(win/mac/linux 三源)
- [ ] **M82** 单元测试框架(kernel 内断言自检)
- [ ] **M83** 内存泄漏检测(分配器统计)
- [ ] **M84** 崩溃转储(minidump 雏形)
- [ ] **M85** 工具链验收:hello/gui/game 一键构建运行

## Wave 6 · AI OS 深化(M86–M95)—— 独有层(文档 07 四件套落地)

- [ ] **M86** 权重作 mmap 对象:模型入 .run 资源,内核 mmap 按需页
- [ ] **M87** 模型卡:权限/计费/审计元数据(资源节)
- [ ] **M88** agent 一等进程:会话/检查点/恢复
- [ ] **M89** fujoctx 升级:窗口焦点/文件变更/syscall 摘要注入
- [ ] **M90** 上下文压缩:委托宿主大模型(fujoctx 链)
- [ ] **M91** 权限与审计(四件套④):能力表 + 审计日志
- [ ] **M92** 意图路由增强:qwen 蒸馏/切换 qwen3-0.6b 对照表
- [ ] **M93** 推理执行器插槽(宿主链路 → 定数量化内核评估)
- [ ] **M94** AI 服务:模型注册表 + fupm 安装模型
- [ ] **M95** AI OS 验收:agent 全生命周期(命令→模型→工具→审计)

## Wave 7 · 交付(M96–M100)

- [ ] **M96** 真机引导最小集(ACPI/PCI 表)
- [ ] **M97** 真机显示/键盘/存储适配(至少一台参考机)
- [ ] **M98** live 镜像 + 安装器
- [ ] **M99** 签名/更新机制
- [ ] **M100** 发布:官网/文档/演示/公告

## 我的取舍意见(如资源受限)

优先:① M11 虚拟内存 + M13 线程调度(解锁一切: malloc→桌面→游戏、fork→多进程、mmap→模型权重)
② M15 VFS + 内存盘(持久化才是"系统", .run 资源形态依赖它)
③ M36 鼠标 + M37 消息环(UI 从演示变可用,第一个外部可见的大台阶)

不建议提前:动态链接(在调度/内存前做会返工)、DXVK(驱动栈未定)、in-OS 大模型(硬件不够,宿主链路+模型卡才是正确形态)。
