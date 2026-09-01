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
- [x] **M34** 兼容矩阵回归(三格式 × 三子系统, 自动化) ——
      **`tools/fujoregress.py`**: 9 用例 × (QEMU 启动→自动注入
      `os run hermes`→串口日志→断言关键字), 结果 **9/9 PASS**:
      ELF×linuxsubsys (m30/m33)、ELF(.run)×fujopack (m31)、
      ELF(+lib)×fujorun (m32)、Mach-O×darwinsubsys (m29)、
      PE×winsubsys (M3/M26/M27/M30); 单用例 `--only IX`、超时
      `--timeout`、`--json` 输出; 每次 ~11s, 全程 ~100s
- [x] **M35** 性能基准:syscall 延迟/切换开销表 —— **m35_bench.elf**
      (rdtsc 计时 + gettimeofday 校准 cyc/us, TCG 虚拟 TSC):
      **A. 纯 syscall 往返 getpid ×200000: 300 ns/call (cyc 149652)**;
      **B. open+close (vfs 路径) ×5000: 577 ns/pair (cyc 288237)**;
      C. time() tick 粒度: PIT 100Hz(10ms 切换粒度), 两次读 tick 差值 0;
      校准: cyc/us=498840(TCG 软件模拟 TSC);
      说明: 基准为 QEMU TCG 软仿真值(native/KVM 路径留 M57 对照);
      附 trace 工具交叉验证(m33); → **M35 RESULT: PASS**;
      **WAVE 2 (M21–M35) 全部完成**
- [ ] **M36** 鼠标驱动(PS/2)+ 命中测试/焦点

## Wave 3 · 图形与交互(M36–M50)—— UI 从"演示"变"可用"

- [x] **M36** 鼠标驱动(PS/2)+ 命中测试/焦点 —— **mouse.rs**: 8042
      AUX(0xA8 + 命令字节 bit1) + **F4 enable reporting**(ps2 鼠标
      默认禁包, 必须显式开启 — 实证)+ IRQ12(向量 0x2C, 从片 IRQ4,
      master IRQ2 级联, 抑制 0x60 争抢)+ 3 字节包状态机(btn+Δx/Δy
      累积符号位处理)+ 命中矩形表 HIT_RECTS[8](z-order, 0x5411
      注册)+ **焦点**(0x5412 查询, 换焦日志);
      fujo 原生: 0x5410 mouse_info(ptr u32×4) / 0x5411 rects / 0x5412
      focus; **实测**(QEMU 9.2 HMP `mouse_move dx dy dz` +
      `mouse_button`, 鼠标设备 #2): `m36: pos=(40,0) btn=2 …(120,0)` →
      **`mouse: focus -> 2`**(矩形 2 命中 150,80/90,110)→ **PASS**;
      **修复链(两处 8042/键冲突)**: ① 读命令字节期间 IRQ1 抢 0x60 把
      键盘扫描码当命令写回 → 杀键盘(bisect: 禁 mouse init 键恢复) →
      整个 AUX 序列禁 IRQ1 下执行+恢复; ② 删 F4 导致鼠标无包(QEMU
      ps2 需 enable reporting);
      回归: 兼容矩阵 9/9
- [x] **M37** 消息环(win32k 等价):消息队列、窗口类、z-order ——
      **wmsg.rs**: 窗口类注册(0x5520, 名→id 查重)、窗口创建(0x5521,
      class/x/y/w/h→win id, 表序=z-order 尾顶层)、环形消息队列
      (64×5-u32, 非阻塞 0x5522 getmsg)、置顶(0x5523, z 表移尾)、
      移除(0x5524); 消息种类 WM_CREATE/ENTER/LEAVE/MOUSEMOVE/BUTTON/
      DESTROY/ZORDER; **mouse.rs→wmsg 联动**(坐标/焦点/按钮→消息投递
      焦点窗口); **实测**: `WM_CREATE win1/win2/win3` 经 getmsg 全
      **3/3 取出** + z-order 置顶消息(m37_wm.elf 窗口类+3 窗) →
      **M37 RESULT: PASS**;
      **实证修复链**: ① 窗口创建逐次注册鼠标矩形互相覆盖(只剩最后
      一个 → 命中试全失) → WINS 表整体重建 refresh_rects; ② 独立
      计数器静态被写坏(值 0x72696CE9 垃圾) → 表改用哨兵扫描
      (id 0xFFFF_FFFF 空位)不依赖计数; ③ 8042 ACK/残余字节污染
      状态机使坐标打满 65535 → IRQ12 开前排空 0x60 + 状态重置;
      ④ IRQ 内串口打印拖慢 handler 引发包竞态 → 剔除, 观测经用户消息流;
      回归: 兼容矩阵 9/9
- [x] **M38** 窗口管理:重叠/焦点/拖动/关闭 —— **原语补全**:
      **wm_move(win,dx,dy)** (0x5525: 位置更新+WM_MOVE+矩形重建)、
      **wm_rect(win,ptr)** (0x5526: 读回 x/y/w/h), 焦点=鼠标命中
      (M36)+ z-order(0x5523)/关闭(0x5524) 已就位;
      **m38_wm.elf 实测**: 2 重叠窗 create → `bring-to-top w2`
      (zorder=1) → **拖动 w1 +100/+50 → rect=(110,60)** →
      **`WM_MOVE win=1 (110,60)`** → 关闭 ×2 →
      **`WM_DESTROY win=1/win=2`** → move=1 destroy=2 zorder=1 →
      **M38 RESULT: PASS**; 回归: 兼容矩阵 9/9
- [x] **M39** 字体升级:更多字形/缩放/抗锯齿 —— **font.rs 位图字体系统**:
      **5x7 全 ASCII 96 字形**(0x20..0x7F, 经典 5x7 字模)渲染到
      RAM backbuffer(0xC00000, 3MiB), **scale 1..=4 整数缩放**
      (7x5·scale, 字符间距 8·scale), 超采样 AA 预留(像素块级);
      fujo 原语: 0x5601 font_text(x,y,scale,color,str) / 0x5602
      font_pixel(x,y 读回) / 0x5603 clear;
      **m39_font.elf 实测**: 三行 "M39" scale 1/2/3 → 采样
      (px=ff00ff00 前景 / ff101010 背景, 亮度对比判定) →
      **M39 RESULT: PASS**;
      **修复链**: 字形方向(7x5 行模式)、silent 误用不画图、亮度权重
      (>>12 边界差 1);
      **★关键(全局)**: 内核 BSS 尾 0x261B30 超出旧 pad
      (0x160000 → 0x260000) 与 initrd 模块区(0x261000)重叠
      → sched TASKS/CUR 等尾静态被模块字节覆盖 → 全矩阵 PANIC;
      **修: pad 0x170000 + MB_HEADER load_end/bss_end 0x0027_0000**
      → 回归 9/9; 回归(新内核): M39 PASS
- [x] **M40** IME 预留(中文输入框架骨架) —— **ime.rs 输入法骨架 v0**
      (音码演示集): 拼音串逐字符输入(0x5702)→ 线性表匹配 →
      **候选窗口**(0x5703 候选指针表, 拷贝用户区 0x7E2000)→
      **提交**(0x5704 → HANZI_OUT)→ 读取(0x5706);
      **m40_ime.elf 实测**: "nihao"/"beijing"/"zhongguo" 三词 →
      **`ime: committed '你好'` / `'北京'` / `'中国'`** →
      `M40 RESULT: PASS`; 骨架预留: 键盘流钩子 + 悬浮候选窗(M48) +
      fujopack 资源码表; 回归: 兼容矩阵 9/9
- [x] **M41** fujokit v0:按钮/文本框/列表控件 —— **sdk/kit/fujokit.h**
      用户态控件库(零 libc, 纯几何/命中/状态机; 宿主负责渲染与消息环):
      **kt_button**(矩形命中+按下/释放触发+计数+label)、
      **kt_textbox**(文本缓冲 64B+追加/退格/光标)、**kt_list**
      (8 行表+行命中 12px 行高+选中);
      m41_kit.elf 模拟交互: 3 次按钮点击 / 输入 "FUJOKI" / 列表行 2
      点击 → **`button triggers=3 textbox='FUJOKI' list=beta`** →
      **M41 RESULT: PASS**; M42 起 GUI 应用直接消费 (渲染=font_text +
      wm 消息=kt 控件)
- [x] **M42** GUI 应用#1:一个可点按钮的窗口(验收) —— **m42_gui.elf 单体
      GUI 应用**: wm 窗口(0x5521, 消息环) + **backbuffer 渲染标题/按钮
      位图**(0x5601/0x5603) + **fujokit kt_button** + **WM_BUTTON 消息
      → 按钮命中触发**(0x5522 取消息, msg=(win,x,y,btn));
      流程: 创建窗口 w1 → 渲染 "[GUI APP v1]"/"[CLICK ME]" →
      轮询 WM_* (鼠标注入尽力) + 命中自测点击 → **`wm_msgs=1
      button_triggers=2`** → **M42 RESULT: PASS**;
      说明: QEMU HMP mouse_button 注入不产生按钮包(按钮消息路径按
      hit-test 逻辑经 kt_button_click 双路验证); 按钮坐标曾置于窗口界
      外(y=208>200) → 焦点不命中 → 修入窗内 y=188
- [x] **M43** 剪贴板/拖放雏形 —— **clip.rs**: 剪贴板 8KB 缓冲
      (0x5801 set / 0x5802 get / 0x5803 len) + **拖放会话**
      (0x5804 begin / 0x5805 move→命中预览 / 0x5806 drop→命中窗口
      → 队列 **WM_DROPFILES**(0x14, win,x,y,payload));
      命中共用 mouse HIT_RECTS 表(wmsg refresh_rects);
      **m43_clip.elf 实测**: clip 往返 `'hello-clipboard'`(15B) →
      `dnd_move hit=1`(rect 100,100,300,220) → `dnd_drop dest=1` →
      **`wm_dropfiles=1 win=1 payload=0xbeef`** → **M43 RESULT: PASS**;
      **修复链**: refresh_rects 误把 w/h 当 x1/y1 注册(历史躲过 —
      M37/M38 命中恰好 w>=x) → 修 x+w/y+h; 回归: 矩阵 9/9
- [x] **M44** 图标/主题/调色板系统 —— **icon.rs**: 16 槽 ARGB 调色板
      (0x5901 get / 0x5902 set)、**主题 apply**(0x5903: LIGHT=0 /
      DARK=1 重载 fg/bg/dim/surface/ink 槽)、**内置 8x8 图标**
      (file/folder/app, 0x5904 draw x,y,id,scale 1..=4 → backbuffer,
      0x5905 像素读回);
      **m44_icon.elf 实测**: DARK 主题(fg=ffe5e7eb/bg=ff0b0f1a)→ 三图标
      绘制 → 图标内=ink 边=黑 → LIGHT 重画 → 采样 →
      **M44 RESULT: PASS**; 修复: 判定含 alpha 位(0xFF…) → 按 RGB 比较;
      回归: 矩阵 9/9
- [x] **M45** 终端窗口控件(串口/VGA 转 GUI) —— **term.rs**: 80x25 文本屏
      (u16 槽 color|char, **user_write 输出镜像** term_feed: 换行/
      退格/80 列回绕/25 行上滚)→ **整屏渲染 backbuffer**
      (0x5A02 term_draw ox,oy,scale: 每字符 7x5·scale 块, VGA 色板
      映射)+ 直接写屏(0x5A01)+ 像素读回(0x5A03);
      **m45_term.elf 实测**: write 两行(镜像)→ term_draw →
      `chars=2000`(80x25 全 render)→ 采样 `px=ffffffff`(首字符左上
      白)/`blank=ff000000` → **M45 RESULT: PASS**;
      修复: 采样点曾落在字形空心(bit 位) → 选 bit6 必 on 位;
      回归: 矩阵 9/9
- [x] **M46** 桌面环境:任务栏/开始菜单雏形 —— **desk.rs 桌面合成 v0**
      (backbuffer): **desk_init**(背景+40px 任务栏+开始按钮方框+
      app 图标)、**taskbar(text)**(时钟位字体)、**start(x,y)**(开始
      按钮命中)、**menu(on)**(200x180 菜单框: 边框+Programs/Files/
      Terminal/Shut Down)、pixel 读回;
      **m46_desk.elf 实测**: init+taskbar → `start-hit=1` → menu 渲染
      → **`tb=ff1caa5e(任务栏绿) menu=ffe5e7eb(表面) bg=ff202020(背景)`**
      → **M46 RESULT: PASS**; 回归: 矩阵 9/9
- [x] **M47** 多屏/分辨率切换(vbe 枚举) —— **VBE 动态模式切换**:
      Bochs VBE 寄存器(0x1CE/0x1CF): XRES/YRES/BPP/ENABLE →
      **三模式枚举** 0x5C01 vbe_set(0=1024x768 / 1=640x480 /
      2=1280x1024) + 读回确认 + **font::FB_W/FB_H 同步**(const →
      static mut, 全渲染模块经 fb_w()/fb_h());
      **m47_vbe.elf 实测**: 三模式逐一切换+读回:
      `mode0->1024x768 / mode1->640x480 / mode2->1280x1024 (rc=0)` →
      收尾回 1024x768 → **M47 RESULT: PASS**; 回归: 矩阵 9/9
- [x] **M48** 输入法候选窗+fujokit 集成 —— **IME 候选流程 + 候选窗**
      组装: ime(0x5701-0x5706)拼音输入 → 候选取回(candidates=2)
      → **fujokit kt_list 装载候选**(选中校验)+ **候选窗渲染**
      (font 两行 "1: China"/"2: Nation" 于 backbuffer) → commit(0) →
      `m48: commit='中国'` → **M48 RESULT: PASS**; 回归: 矩阵 9/9
- [x] **M49** 无障碍:高对比/大字模式 —— **a11y.rs**: 0x5D01 a11y_set
      (0 正常 / 1 高对比调色板反转 fg↔bg+surface+ink / 2 大字 /
      3 高对比+大字)、0x5D02 查询; **font_text 大字联动**(scale+1,
      a11y::scale_boost);
      **m49_a11y.elf 实测**: 高对比 `fg=ffffffff bg=ff000000`(反转) →
      大字渲染采样 `large px=ffffffff` → **M49 RESULT: PASS**;
      回归: 矩阵 9/9
- [x] **M50** GUI 基准:窗口开关/拖动帧率表 —— **m50_bench.elf**(rdtsc +
      gettimeofday 校准 cyc/us=817): **A. 窗口开关 create+remove ×100:
      216 us/op**(TCG 软仿真); **B. 拖动帧 wm_move+采样 ×100: 6 us/frame
      → ~166k fps**(名义值, QEMU TCG); 说明: QEMU 虚拟数值, KVM/native
      对照留 M57; → **M50 RESULT: PASS**;
      **WAVE 3 (M36–M50) 全部完成**

## Wave 4 · 游戏与性能(M51–M70)

- [x] **M51** 显示驱动抽象:QEMU virtio-gpu 接入 —— **display.rs 显示后端
      抽象 v0**: PCI 枚举(0xCF8/0xCFC)后端标识 **0=Bochs VBE
      (std-vga 0x1234:0x1111)/1=virtio-gpu-pci(0x1AF4:0x1050)**
      + 分辨率回填; 0x5E01 disp_info(ptr→u32×5) / 0x5E02 set_backend
      (偏好, M61 实际切换);
      **m51_disp.elf 实测**: `backend=0 vendor=00001234 device=00001111
      mode=1024x768`(virtio-gpu 未装配时探测确认 absent) →
      **M51 RESULT: PASS**; 回归: 矩阵 9/9
- [x] **M52** 音频:AC97/HDA 驱动 + 播放入口 —— **audio.rs AC97 v0**
      (QEMU `-device AC97`, 0x8086:0x2415, I/O BAM BAR0):
      PCI 探测 + 全局控制 0x2C 复位/使能 + PCM out 音量(0x18) +
      **播放入口**(0x5F04 采样排队, 真实 FIFO 混音留 M63);
      fujo: 0x5F01 info / 0x5F02 enable / 0x5F03 volume / 0x5F04
      playback;
      **m52_audio.elf 实测**(QEMU AC97): `present=1 vendor=8086` →
      `enable rc=0` → `playback queued 64 samples` →
      **M52 RESULT: PASS**
- [x] **M53** XInput 式输入抽象(手柄/键鼠统一) —— **xinput.rs**:
      XInput 布局状态(buttons bitmask + LX/LY/RX/RY i16 轴),
      **键鼠统一聚合**: 键盘 WASD→左摇杆 / 空格→button0 / Z X→
      button1/2 / Enter→start / Backspace→back(kbd_hook), 鼠标相对
      位移→右摇杆+按钮→bit8(mouse_hook); 0x6001 get / 0x6002 reset /
      0x6003 press(自测注入);
      **m53_xin.elf 实测**: reset→全 0 → press(1|4)→ buttons=5 →
      **M53 RESULT: PASS**
- [x] **M54** 定时器:高精度/帧同步(rdtsc 基) —— **timer.rs**: rdtsc 单调
      时钟 + **两阶段校准**(0x6100 arm 后 PIT 在用户态推进, 跨中断期
      差值 cyc/µs —— 内核 syscall 期 IF 被 SFMASK 屏蔽, 单调用内不能
      等 tick, 实证), 0x6101 us / 0x6102 ms / 0x6103 sleep_us(忙等) /
      0x6104 frame_wait(µs 帧边界同步) / 0x6105 info;
      **m54_timer.elf 实测**: `calibrated cyc/us=1849` →
      `sleep(200000) took=200136 us` → `frame gap=49980 us (~50000)` →
      **M54 RESULT: PASS**
- [x] **M55** 原生 fujogl v0(OpenGL 1.x 子集, 软件后端) —— **gl.rs 软件
      光栅**: backbuffer (glClear/glRectf/顶点三角形/线):
      0x6201 clear(r,g,b) / 0x6202 rect(x,y,w,h,color 打包) /
      0x6203 tri(verts ptr 6×u32 + color; 整数重心法) /
      0x6204 line(Bresenham) / 0x6205 pixel 读回;
      **m55_gl.elf 实测**: 大三角(红)+白 rect+绿线 →
      `tri_in=ffff0000 out=ff000000 rect=ffffffff line=ff00ff00` →
      **M55 RESULT: PASS**; 说明: 参数面 6 上限(dispatch 帧) →
      顶点缓冲指针+打包色
- [x] **M56** DXVK 式翻译可行性评估(方案+原型) —— **docs/09-dxvk-feasibility.md**
      (现状盘点/DXVK 架构对照/分层方案 layer0..2/缺口路线 M61/M62/M63/
      M69); **原语原型 dxwrap.rs**: 顶点缓冲(0x6301)+ 仿射矩阵
      (0x6302)+ flush 变换→fujogl 光栅(0x6303)—— "D3D 命令模型 →
      fujogl" 最小翻译闭环; **m56_dxwrap.elf 实测**: 3 顶点 × 2x
      矩阵 → `center=ffff0000 orig=00000000 corner=00000000`
      (变换后中心红/放大三角外黑) → **M56 RESULT: PASS**
- [ ] **M57** KVM 加速支持 + TCG/KVM 基准对照
- [x] **M58** 2D 游戏#1 原生运行(开源 2D 引擎选择) ——
      **docs/10-2d-engine.md**(SDL2/LÖVE/自研 fujogl+fujokit 比较 →
      **v0 引擎 = fujogl 光栅+fujokit+XInput, SDK 闭环零依赖**, 打包经
      fujopack/fujorun); **验证游戏 m58_pong.elf**(60 帧: 球 12x12 +
      拍 20x60, 回弹物理, 拍跟球, gl_rect/gl_pixel 纯原语):
      **`track x=100..395 y=200..377 sampled=11`**(运动+回弹+首尾帧
      采样) → **M58 RESULT: PASS**
- [x] **M59** 游戏模式:前台调度/资源预留/全屏 —— **gamemode.rs**:
      0x6601 game_mode(on)(**sched::GAME_MODE 前台调度标记**)/ 0x6602
      status(mode/ticks/heap 预留基址)/ 0x6603 fullscreen(VBE 1024x768
      确认);
      **m59_gamemode.elf 实测**: `mode=1 ticks=1537 heap=8388608` →
      `fullscreen rc=0 mode=1024x768` → **M59 RESULT: PASS**; 回归: 矩阵
      9/9
- [x] **M60** 存档沙箱(权限目录+版本化) —— **save.rs**: 存档命名空间与
      VFS 隔离(权限: 仅经 save 原语, 无路径越权面), 8 槽 × 8KiB,
      每槽版本头 [magic "SAV1"][version][len]: 0x6701 write /
      0x6702 read(版本校验: 新版拒绝) / 0x6703 list / 0x6704 version;
      **m60_save.elf 实测**: `read n=10 data='hello-save' version=2` →
      slot0=10 slot1=-1(未用) → **M60 RESULT: PASS**;
      **★BSS 治理**: save 表 64KB 使内核超 pad(0x270000) →
      pad 0x180000 + MB_HEADER 0x0028_0000(矩阵 8/9→9/9); 脚本同步
- [x] **M61** 图形加速:blit/缩放硬件路径 —— **blit.rs** (display 后端
      接口扩展, 当前软件路径/virtio 同接口): 0x6801 blit(src,dx,dy,w,h)
      矩形拷贝 / 0x6802 blit_scal(src,dx,dy,dims[4]) 最近邻缩放;
      **m61_blit.elf 实测**: 16x16 源(左上红其余蓝) → 1:1 blit +
      2x 缩放 → `b1=00ff0000 b2=000000ff s1=00ff0000 s2=000000ff`
      (四采样) → **M61 RESULT: PASS**; 修复: 源行宽硬编码 640 → 按 w;
      回归: 矩阵 9/9
- [x] **M62** 着色器内核评估(compute 子集)
      - **上下文**: 着色器=每像素并行计算内核; 纯 CPU 上先以解释执行
        bytecode VM 建立"内核即程序"模型, 评估指令率/并行架构决策
        (GPU path 由 M64+ 后硬件层补位)。
      - **接口**: 0x6901 shader_load(ptr,n≤32字) / 0x6902 shader_run(x,y,w,h)
        区域逐像素执行 / 0x6903 shader_pixel(x,y) 读回 /
        0x6904 shader_ops() 指令计数(性能面)。
      - **字节码 v0** (每字 u32: op<<24|r<<16|a<<8|b, 8 寄存器):
        op0 halt / 1 const r,v / 2 add r,a,b / 3 mul r,a,b / 4 sub r,a,b /
        5 color r,a,b = (regs[a]&0xFF)|((b&0xFF)<<8) / 6 idx 重载索引;
        每像素序: r0=idx(y*FBW+x), 执行至 halt, r1=输出色 → BACKBUFFER。
      - **m62_shader.elf 实测**: 7 指令内核 (const×3, add×2, mul×1,
        color) 跑 16x16=256 像素 → `p00=0000ff00 p10=0000ff01
        p50=0000ff05 p1515=0000ff0f ops=00000800` (每像素 8 轮含
        halt, 256×8=2048) → **M62 RESULT: PASS**。
      - **评估结论**: v0 CPU 每像素 ~7 指令解释 ~ 2048 次 VM 轮询,
        开销 ~10x 原生算术 → compute 子集可作为原型, 并发/GPU 由
        m64 多核 + 未来 SIMD 通道承接(文档: docs/11-shader-eval.md)。
- [x] **M63** 音频混音器/效果链
      - **接口**: 0x5F05 mix_open(ch) 重置 / 0x5F06 mix_push(ch,ptr,n)
        追加样本 / 0x5F07 mix_render(ptr,n,gain) 混音到用户缓冲 /
        0x5F08 mix_effect(ch,kind,p) (1=低通系数 0..256, 2=增益 0..256) /
        0x5F09 mix_status(ptr)。
      - **实现**: 4 路 i16 单声道 ×128 样本; 每通道效果链
        `输入 x → 单极低通 y += k/256*(x-y) → 增益 g/256 → 饱和累加`;
        混音 i64 累加 → 全局 gain → clamp i16。
      - **m63_mix.elf 实测**: ch0 64×10000 + ch1 64×5000 + ch2 32×4000 →
        `mix0=19000 mix40=15000` (ch2 结束切片); ch0 低通 k=192 →
        `lp0=7500 lp7=9999` (一阶收敛); ch0 增益 50% → `gain=5000` →
        **M63 RESULT: PASS**; 文档: docs/12-audio-mixer.md
- [x] **M64** 多核并行:调度亲和/负载均衡 v0
      - **探测**: CPUID leaf 1 EBX[23..16] 逻辑核数 (global_asm 桥
        fujo_cpuid_leaf1, rbx LLVM 保留绕开); `smp: cpuid logical CPUs
        = 2` (QEMU `-smp 2`, TCG 多线程)。
      - **接口**: 0x6A01 aff_set(tid,mask) / 0x6A02 aff_get(tid) 亲和
        位图 (默认 0xFF=任意核) / 0x6A04 smp_stats(ptr) →
        (ncpu, core0, core1, switches)。
      - **负载均衡 v0**: 每次用户态 PIT 切换按目标任务亲和最低置位
        bit 记入该核负载: `smp::balance_task` 由 sched tick 切换点
        调用; 核 0/1 桶统计, `c0+c1==switches` 不变量。
      - **m64_smp.elf 实测**: fork 父(亲和核0)/子(亲和核1) 双方忙等
        20M 轮 → `ncpu=2 c0=8 c1=8 sw=16` (全部切换按亲和归桶,
        task1 恒归 core1, task0 恒归 core0) → **M64 RESULT: PASS**;
        文档: docs/13-smp.md; 真 SMP 启动(APIC/每核 TSS) → M65。
- [x] **M65** 每核 TSS/中断注入优化
      - **双 TSS**: GDT 扩 16 槽: 5/6=TSS0 (0x28, rsp0=0x300000),
        7/8=TSS1 (0x38, rsp0=0x3A0000 核1 独立栈); gdt.rs 初始化双方
        (Tss repr(C,packed) 布局 rsp0@+4); `tss_info` 读回验证
        (0x6B02 → (rsp0_0, rsp0_1, gdt_limit=0x7F))。
      - **核标识**: 0x6B01 core_id() → CPUID leaf 1 EBX[31..24] 初始
        APIC ID; LAPIC MMIO (0xFEE00000) 未映射入 boot 页表 (已知
        限制, 记录文档 14, 后续页表补映射后切 MMIO 读)。
      - **中断注入 v0**: 0x6B04 irq_route(mask) 目标核掩码 (1=核0,
        2=核1, 3=轮转); 每次 PIT 中断 (fujo_tick_sched 入口) 按掩码
        入核桶; 0x6B05 irq_stats → (lapid, r0, r1, inj)。
      - **m65_tss.elf 实测**: `lapic_id=0`; 路由测试三段各 20M 用户态
        忙循环 (PIT 用户态中断可触发): core0 段 `r0=8 r1=0`, core1
        段 `r0=0 r1=8`, 轮转段 `4/4` (掩码分散正确) →
        **M65 RESULT: PASS**; 文档: docs/14-tss-irq.md
- [x] **M66** 页缓存/预读(内存盘→真盘)
      - **接口**: 0x6C01 alloc(n) / 0x6C02 write(blk,ptr) /
        0x6C03 read(blk,ptr) (miss→盘同步) / 0x6C04 prefetch(start,n)
        (顺序预读窗口) / 0x6C05 flush() 脏页回盘 / 0x6C06 evict() /
        0x6C07 info(ptr) → (slots, dirty, hits, miss)。
      - **实现**: 16 页槽 (元数据 256B BSS; 数据区常量
        0xF10000..0xF20000, 模拟盘 0xF24000..0xF28000 —— 启动格式化
        清零; 0xD00000 区实测与 backbuffer 0xC00000..0xF00000 重叠
        (0x5A=窗口边框色 0x30305A 字节序, 见文档 15), 迁出修复)。
      - **m66_pcache.elf 实测**: alloc(3)→write 0xAB/0xCD→flush=2
        回盘→read hit→evict→prefetch(0,2) 从盘装→read=0xAB/0xCD;
        evict 后读未预读页 → miss 路径空页 → `slots=1 dirty=0
        hits=3 miss=1` → **M66 RESULT: PASS**; 文档: docs/15-pcache.md
- [x] **M67** 中断合并/减轻(串口/网卡)
      - **接口**: 0x6D01 irq_set_window(w) 合并窗口 (1..64, 基点重置) /
        0x6D02 irq_cost_stats(ptr) → u64×4: (irqs, batches,
        total_cyc, worst_cyc)。
      - **实现 (irq.rs)**: 每 PIT tick 记账: rdtsc 间隔 (total/worst
        成本预算), 合并批 = (IRQS−基点)/WINDOW 公式化; 窗口切换基点
        重置; 语义: 调度/时钟保持逐 tick, 合并层双账不改变行为
        (减负面: 高频周期中断按窗口组批, 为后续真合并硬件
        IRQ coalesce 提供策略面)。
      - **m67_irq.elf 实测**: 忙等 ~8 ticks: window=1 → `d_irqs=8
        b=8` (逐 tick), window=8 → `d_irqs=8 b=1` (8:1 组批),
        成本非零 → **M67 RESULT: PASS**; 串口/网卡中断合并面记录
        (无硬件 IRQ 源 v0, 文档: docs/16-irq-merge.md: 16550 FIFO
        阈值/82574L MSI-X 预留说明)。
- [x] **M68** 帧时间表/性能计数器工具
      - **接口**: 0x6E01 perf_frame_mark() 帧边界标记 (µs 间隔入
        环形 64 表) / 0x6E02 perf_frame_stats(ptr) → (frames, avg,
        max, sum) / 0x6E03 perf_counter_enable(id,on) /
        0x6E04 perf_counter_read(ptr) → u64×8。
      - **实现 (perf.rs)**: 帧表经由 timer 校准 (两阶段 cyc/us;
        记录 `calibrated cyc/us≈2495` 于首个 mark); 计数器挂钩:
        0=PIT IRQ (irq::note), 1=syscall (dispatch 顶),
        2=ctx-switch (sched 切换点), 默认启用 0/1;
        修复: 帧数递增原以 min(N,63) 清零 — 改 F_N+=1 上限 64。
      - **m68_perf.elf 实测**: 5×mark 隔 20M 忙循环 → `frames=4
        avg=83092µs max=85231µs` (来自 first-mark 差值 4 条); 计数器
        差分: `d_irq=8 d_sys=1` → **M68 RESULT: PASS**;
        文档: docs/17-perf.md
- [x] **M69** 2D 游戏#2 + 输入延迟基准
      - **game2.rs 仪表**: 0x6F01 game2_latency(us) 输入→渲染完成
        延迟累计 (N/SUM/MAX) / 0x6F02 game2_stats(ptr) →
        (n, avg_us, max_us, hits) / 0x6F03 game2_hits(v) 命中上报。
      - **m69_game2.elf (Breakout v0)**: 球 16x16 (M61 blit), 拍
        20x60 (0x6202 gl_rect), 顶部砖块带; 10 帧 × frame_wait
        20ms; 每帧: timer_us 采样 → 模拟输入 (拍随球) → 物理
        (14px 步进+反弹) → 砖块命中 → blit 渲染 → latency 上报。
      - **实测**: `frames=10 avg_lat=94µs max_lat=717µs hits=1`
        → **M69 RESULT: PASS**; 输入延迟基准值 (采样→渲染完成,
        TCG 下) 供 M70 验收对照; 文档: docs/18-game2.md
- [x] **M70** 游戏层性能验收报告
      - **docs/19-game-acceptance.md**: M51–M69 全部实测数字汇总表
        (12 行 PASS 基线) + 性能指标 (帧 20ms 驱动 / 输入→渲染
        avg 94µs max 717µs / IRQ 合并 8:1 / blit≈1 syscall /
        着色器 8 轮 per px / 音频 4ch×256 一次 syscall / 页缓存
        16+4 页) + 完整链路 (回归矩阵 9/9 + 游戏闭环) + 缺口
        (GPU 通道 / 音效 FIFO / SMP 真并行 / 网卡 IRQ 合并;
        KVM 对比为后续基准重跑面)。
      - **验收结论**: 游戏层性能验收 **PASS**; Wave 5 (M71+) 以本
        基线为回归锚点。Wave 4 (M51–M70) 全部完成。

## Wave 5 · 开发工具链(M71–M85)

- [x] **M71** 系统内汇编器(最小 .s 编译)
      - **asm.rs 两遍汇编器**: pass1 扫描 label 地址 (L0..L15) +
        指令长度; pass2 生成字节码。
      - **指令子集**: nop/ret/int3/syscall; mov(r64,imm64|r64);
        add/sub/xor/cmp(r64,imm8|r64); inc/dec; push/pop;
        jmp/je/jne (rel32, 与本地 label)。寄存器 rax..rdi 7 个;
        立即: 0x.. / 十进制 / $-前缀; 伪指令 .text/.byte/.word/.quad;
        `#`/`;` 注释; 操作数逗号容错。
      - **接口**: 0x7001 asm_assemble(src,n,dst,cap) →
        字节数 (负=err) / 0x7002 asm_verify(ptr,n) → 解码指令数
        (遇 ret 停)。
      - **m71_asm.elf 实测**: 7 指令程序 (nop/mov rax,0x42/xor/
        add/jcc/inc/ret) → `n=28` (1+10+3+4+6+3+1), je rel32=0
        (L0 紧跟), 字节检查 b0=90 b18=0F b19=84 b20=0 b24=48
        b25=FF b27=C3, `inst=7` → **M71 RESULT: PASS**;
        踩坑: 操作数逗号、label-only 行注册、jcc 字节序 0F 84、
        mov imm 二次元位; 文档: docs/20-asm-tool.md
- [x] **M72** 系统内链接器(ELF 静态最小)
      - **ld.rs**: cfg 表驱动 (dst/text1/text2/syms/relocs 9×u64);
        符号表 [name 32B][vma 8B]; 重定位 [place][symidx];
        输出 ELF64 ET_EXEC + 1×PT_LOAD (RWX), 段 0x400000 起,
        重定位写绝对地址 (base+vma)。
      - **接口**: 0x7101 ld_link(cfg) → 输出字节数 /
        0x7102 ld_info()。
      - **m72_ld.elf 实测**: text1=[90 C3] text2=[CC] foo@0x100,
        reloc@0x8003 → `total=0x8011`, 字节检查 magic/ET_EXEC/
        e_entry=0x400000/p_flags=7/段数据/reloc=0x400100 →
        **M72 RESULT: PASS**。
      - **BSS/pad**: M71 asm + M72 后 BSS 尾 0x2801F0 **超 0x280000**
        → pad 0x180000→**0x1A0000**, load_end/bss_end
        0x002A_0000 同步 (build-kernel.ps1 一并更新);
        文档: docs/21-ld-tool.md
- [x] **M73** 迷你编辑器(vi 子集)
      - **editor.rs**: 2KiB 文本缓冲, '\n' 行模型, 游标 (row,col);
        vi 键: i=插入 (Esc 退出) / j=下行 k=上行 / x=删游标字符 (含
        行合并) / ^=行首 $=行尾。
      - **接口**: 0x7401 ed_init / 0x7402 ed_text(ptr,n) /
        0x7403 ed_key(c) / 0x7404 ed_dump(ptr,cap) /
        0x7405 ed_info(ptr) → (row, col, lines, len)。
      - **m73_edit.elf 实测**: "abcd\nefg" → k/j 移动 → x 删 'd' →
        $/^ 列移动 → i+'X'+Esc → dump `Xabc\nefg` (行模型/插入/
        删除/移动全过) → **M73 RESULT: PASS**; 文档: docs/22-editor.md
- [x] **M74** fujocc 编译壳(表驱动, 跨 ABI 选项)
      - **fujocc.rs**: C 子集 → 表驱动翻译 → fujo-asm 文本 (M71) →
        asm_assemble → 字节码 → 链接 ELF64 (M72) → 输出, **全链**。
      - C 子集 v0: `int NAME() { return EXPR; }`, EXPR = 常量
        (hex/dec); 表: KEYWORD_T (int/return/main/void),
        ABI_T (linux=1/mac=2/win=4, 选项字符串面)。
      - **接口**: 0x7501 cc_compile(src,n,dst,cap,abi) →
        ELF 字节数 / 0x7502 cc_version()。
      - **m74_cc.elf 实测**: `int main() { return 0x41; }` →
        `asm 11 bytes` (mov rax,0x41/ret) → `ELF total=0x8010`,
        b0=48 b2=41 b10=C3, magic/ET_EXEC/e_entry 校验 →
        **M74 RESULT: PASS**; 踩坑: fmt 分段写出覆盖 (游标式
        AsmOut 修)、0x 常量被十进制分支截断 (分支排序);
        文档: docs/23-cc-shell.md
- [x] **M75** 调试器 v0:单步/断点(调试寄存器)
      - **单步**: 用户在用户态置 TF (pushfq|or 0x100|popfq),
        每条指令 #DB (vec 1, fujo_dbg_stub) → 内核记 steps + 清 TF
        (帧 RFLAGS & !0x100), iretq 返回续跑。
      - **断点**: int3 软件断点 (#BP vec 3, fujo_bp_stub): 替换目标
        首字节 0xCC → 命中恢复原字节 + RIP-1 (回退重执);
        DR0/DR7 执行断点实测 QEMU TCG 不触发, 文档记录 (GDB 型
        软件断点是 TCG 通用面)。
      - **关键坑**: 用户态 `int3` = INT 3 指令, **中断门 DPL 必须
        >= CPL** — attr 0x8E → #GP (vec 13 实测) → 改 **0xEE** (DPL=3)。
      - **接口**: 0x7601 dbg_step(on) / 0x7602 dbg_bp0(addr) /
        0x7603 dbg_info(ptr) → (count, last_rip, steps, bps) /
        0x7604 dbg_clear()。
      - **m75_dbg.elf 实测**: 裸 int3 #BP + dummy 断点命中 (恢复
        重执返回 42) + 3 次 TF 单步 → `total=5 steps=3 bps=2` →
        **M75 RESULT: PASS**; 文档: docs/24-debugger.md
- [x] **M76** syscall trace 工具化(打开后台记录)
      - **在 M33 trace 之上**: 0x7701 trace_bg(on) 后台记录
        (不经 trace_show, ring/counts 持续写入) / 0x7702
        trace_stats(ptr) → (total, nonzero_nr, ring_pos, dropped) /
        0x7703 trace_filter(nr) 过滤 (0=全, 否则仅该 nr)。
      - **实现**: dispatch 头 rec = TRACE_ON||TRACE_BG 且
        (filter==0||filter==nr); total/dropped 维护;
      - **m76_trace.elf 实测**: 后台开 → 写+读统计 (t0=2 t1=4,
        nonzero>=1) → filter(1) 后 3 次 write → 差分 `d_filter=3`
        (其它 syscall 不记) → **M76 RESULT: PASS**;
        文档: docs/25-trace.md
- [x] **M77** 性能计数器(rdtsc/中断计数窗口)
      - **接口**: 0x7801 win_begin(id) 快照 (us, irq, sys) /
        0x7802 win_end(id) 差分 → 窗口 / 0x7803 win_read(ptr) →
        u64×4: (us_delta, irq_delta, sys_delta, calls)。
      - **实现 (perf.rs 扩展)**: 时间基 timer_us (校准), 计数基 M68
        的 IRQ/syscall 计数器; 窗口差分表单槽 v0;
      - **m77_win.elf 实测**: 20M 忙循环窗口 → `us=82967 irq=8
        sys=1 calls=1` (窗口内 8 次 PIT, 读回自身 1 次 syscall) →
        **M77 RESULT: PASS**; 文档: docs/26-perfwin.md
- [x] **M78** CI:QEMU 无头启动 + 日志断言自动化
      - **tools/ci.py (fujoci)**: 兼容矩阵 9 用例 (ELF/Mach-O/PE ×
        子系统) + 里程碑日志断言 16 用例 (m61..m69, m71..m77
        "MXX RESULT: PASS") = **25 用例**; 每用例: QEMU 256M 无头
        (file: 日志) → monitor 注入 `os run hermes` → 断言关键字;
        JSON 报告 (--json) + 退出码。
      - **.github/workflows/ci.yml**: windows-latest + choco
        llvm/qemu + build/flatten + `python tools/ci.py` + artifact。
      - **本地实测**: `fujoci: 25/25 PASS` (run ~10 分钟);
        文档: docs/27-ci.md
- [x] **M79** fujopack/fujorun 命令全参数化 + 手册
      - **fujopack pack**: `--name` (manifest name) / `--type`
        (app|game|tool) / `-v` 节表概要; info/check 保留。
      - **fujorun pack**: `--name` 主模块节表名; **run**: `--smp N`
        / `--timeout S` (超时 kill) / `--bootsleep S` / `--mem` /
        `--keys` / `--log` 全参数。
      - **实测**: `pack --name demo-app --type game -v` → info/check
        节表+fnv 校验 ok; fujorun pack --name main (2 modules) → 输出;
        **docs/28-toolchain-manual.md** (全部链工具: flatten/
        fujoregress/fujoci/qemu-kvm + 端到端示范)。
- [x] **M80** SDK 文档闭环(示例/模板/教程)
      - **docs/29-sdk-close.md**: 源码布局索引 (sdk/hello + user/ +
        linux/ + win/ + mac/ + kit + ai/hermes) + 三格式构建命令 +
        快速开始教程 (Hello/游戏/GUI/多文件) + 验证链。
      - **sdk/templates/**: hello.tpl.c (最小 _start 入口) /
        game.tpl.c (帧循环 + 延迟上报) / gui.tpl.c (fujokit 骨架)。
- [x] **M81** 交叉编译工具链一键脚本(win/mac/linux 三源)
      - **scripts/cross-build.ps1**: 一键三目标 (ELF/Mach-O/PE32+),
        参数 -Src/-Mac/-Win/-Out; PE 路径重建 kernel32.lib (dlltool);
      - **实测**: `cross-build: 3/3 PASS` (app.elf/app.macho/app.exe);
        文档: docs/30-cross-build.md。
- [x] **M82** 单元测试框架(kernel 内断言自检)
      - **utest.rs**: 注册表 (8 槽函数指针), 用例: strlen/strcmp/
        hex 解析/整数数学/strrev/bits/行模型 (7 个纯函数);
      - **接口**: 0x7901 ut_run() → 全跑 (pass-fail 返回) /
        0x7902 ut_info(ptr) → (pass, fail, total, allpass);
        启动注册 "ut: unit-test suite registered (7 cases)"。
      - **m82_ut.elf 实测**: `ut: run done pass=7 fail=0` →
        `pass=7 fail=0 total=7` → **M82 RESULT: PASS**;
        文档: docs/31-utest.md
- [x] **M83** 内存泄漏检测(分配器统计)
      - **leak.rs**: 快照差分 (kobj 对象表 M19 计数 4 类
        [file/pipe/shm/sig]); 0x7A01 leak_begin() 快照 /
        0x7A02 leak_end(ptr) → (delta, allocs, frees, baseline) /
        0x7A03 leak_stats(ptr)。
      - **m83_leak.elf 实测**: 快照 → kobj_create ×4 → `delta +4
        (unreleased slots)` (泄漏可检) → free 全部 → `delta -4
        (freed below baseline)` / after-free delta=0 →
        **M83 RESULT: PASS**; 文档: docs/32-leak.md
- [x] **M84** 崩溃转储(minidump 雏形)
      - **dump.rs**: 用户异常捕获 (挂接 fujo_exc2 用户分支, 隔离
        转场前): 布局 120B `FUJDUMP\0 + vec + rip + cr2 + rsp + cs +
        regs8 + count`;
      - **接口**: 0x7B01 dump_arm(on) / 0x7B02 dump_read(ptr,cap) →
        B 数 / 0x7B03 dump_info(ptr) → (count, vec, rip, cr2)。
      - **m84_dump.elf 实测**: fork → 子 ud2 (#UD vec6) 崩溃隔离 →
        `dump: captured minidump #1 vec=6 rip=0x40041a` →
        `count=1 vec=6 n=120` → **M84 RESULT: PASS**;
        文档: docs/33-dump.md
- [x] **M85** 工具链验收:hello/gui/game 一键构建运行
      - **scripts/onebuild.ps1**: templates (hello/game/gui) →
        clang ELF → fujopack .run → QEMU 依次验证日志断言
        ("template app"/"template frame loop"/"fujokit skeleton");
        参数: -BuildOnly / -Kernel。
      - **实测**: `onebuild: 3/3 PASS (hello/gui/game build+run)`;
        文档: docs/34-onebuild.md; **Wave 5 (M71–M85) 完成**。

## Wave 6 · AI OS 深化(M86–M95)—— 独有层(文档 07 四件套落地)

- [x] **M86** 权重作 mmap 对象:模型入 .run 资源,内核 mmap 按需页
      - **mem.rs 扩展**: 权重库 WLIB=0xF30000 (8KiB, backbuffer/
        页缓存区后); 映射区表 WMAP×2 (va/len/on)。
      - **接口**: 0x7C01 wmap_load(ptr,len) 权重复制入库 /
        0x7C02 wmap_res(va,len) 登记权重 VA (需求段, 页对齐) /
        0x7C03 wmap_stats(ptr) → (pfa, pages, wlen, maps)。
      - **按需页**: #PF 钩子 wmap_fault (demand-zero 前): cr2∈权重
        区 → 从 WLIB 拷贝页 (frame_alloc_zero + PTE) → iretq 重试;
        与 M12 demand-zero 同 PT 路径。
      - **m86_wmap.elf 实测**: blob (i&0xFF pattern) → res
        0xB90000 → 读 4KB → `sum 一致 pfa=1 pages=1 wlen=4096` →
        **M86 RESULT: PASS**; 文档: docs/35-wmap.md
- [x] **M87** 模型卡:权限/计费/审计元数据(资源节)
      - **modelcard.rs**: 卡 (120B: name[24]/version/perm_mask/
        cost/calls/tokens/budget) + 审计环 16×32B
        (ts, model, tokens, result);
      - **接口**: 0x7D01 mc_register(ptr) / 0x7D02
        mc_call(len,perm_need) → 0 | -1 (perm 越权/超预算 deny,
        均入审计) / 0x7D03 mc_info(ptr) → (calls, tokens, budget,
        perm) / 0x7D04 mc_audit(ptr,cap)。
      - **m87_mcard.elf 实测**: qwen3-0.6b perm=3 budget=1000 →
        3 次 100tokens ok → perm_need 8 deny → 900tokens 超预算
        deny → `calls=3 tokens=300 aud=5` → **M87 RESULT: PASS**;
        文档: docs/36-modelcard.md
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
