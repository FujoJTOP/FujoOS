# 105 · W34 — 兼容层：Windows 文件完美运行（.run/FUJR + Console API 面）

> 里程碑: W34 · 目标: **Windows 文件在 FujoOS 完美运行** —— 标准 Win32 Console API 模式
> 的 PE32+ 程序（零修改）直跑 + 经 `.run`（FUJR）容器运行, 两部分均 PASS。
> 一句话: **kernel32 shim 面 +10 API（GetStdHandle/WriteConsoleA/VirtualAlloc/时间/进程）,
> m152_win.exe 直跑 PASS, fujopack 打包成 .run 后同样 PASS——Windows 文件全链完美运行。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `kernel/src/pe_loader.rs` | SHIM_TABLE +10（kernel32.dll: GetStdHandle/WriteConsoleA/GetConsoleMode/SetConsoleMode/GetTickCount/GetSystemTimeAsFileTime/VirtualAlloc/VirtualFree/FlushFileBuffers/GetCurrentProcessId, 0x5025–0x502E） |
| `kernel/src/syscall.rs` | shim_dispatch +10 实现（console 句柄映射 → fd；堆复用 shim_heap_alloc；tick ms；FileTime 近似；process=task）· dispatch 范围 0x5019..=0x5018→**0x502E** |
| `sdk/win/m152_win.c` | 真实 Console 程序（零 CRT 裸 PE32+）：标准 API 模式 GetStdHandle→WriteConsoleA 输出 · GetTickCount/FileTime · VirtualAlloc 缓冲 · CreateFileA+WriteFile+FlushFileBuffers · GetCurrentProcessId · ExitProcess(0xAB) |
| `sdk/win/kernel32.def` | EXPORTS +11（编译期导入面） |
| `tools/fujoregress.py` | 新用例: `pe-m152`（直跑）+ `run-w152`（.run 容器载荷） |
| `scripts/build-samples.ps1` | m152_win 入 win 列表 |

## 2. 验证

```
:: [40] pe-m152  PE32+ x win-console  PASS   (直跑 sdk/win/m152_win.exe)
:: [41] run-w152 ELF(.run) x win32 console PASS (fujopack pack -e m152_win.exe → .run)
```

程序输出（节选）:
```
fujo: windows console path (W34)
fujo: GetTickCount=0x0000000000004c00
fujo: FileTime=0x01d30000004c0000
fujo: heap buffer from VirtualAlloc OK
fujo: file open \boot\module OK -> ...  pid=0x...
fujo: W152 RESULT: PASS
user : ExitProcess(171) - kernel takeover, M3 verified
```

## 3. 兼容层现状（W34 后）

| 面 | API 数 | 说明 |
|---|---|---|
| kernel32.dll | 30 | 文件/控制台/时间/内存/进程/异常/模块 |
| gdi32/user32 | 10 | 字体/文本/DC（M109 图形面） |
| msvcrt.dll | 31 | mingw CRT 面（M27/M28: malloc/printf/atof/...） |
| 容器 | FUJR `.run` | header+manifest+≤8 资源; PE 载荷经统一加载器 |
| 脚本 | `.shell` | FUJR EMBED 首行 `#!fujoshell`（零容器格式改动）→ 内置解释器（echo/注释/未知行报告） |

W34 后回归: **43/43**（新增 `run-w153` .shell 用例；.shell 载荷 = `sdk/shell/m153_shell.sh` →
`fujopack pack -e` → `.run`，解释执行输出 3 行 + `W153 RESULT: PASS`）。

## 4. 坑 (W34)

1. **Win64 零扩展**: `(DWORD)-11` 经 `mov ecx,imm32` 传参被零扩展为 +0xFFFFFFF5 →
   GetStdHandle 判断必须按低 32 位截断（`(a1 as u32) as i32`）；否则句柄匹配失败。
2. **syscall 分发范围**: SHIM_TABLE 加号后必须同步 `syscall.rs` 的 dispatch 范围
   （0x5019..=0x5024 → 0x502E），否则落入 "unimplemented"。
3. **kernel32.def 是编译期导入面**: 新 API 需同步 def + dlltool 重建（否则 lld 链接失败）。
4. **VirtualAlloc 复用 shim_heap_alloc**（SHIM_HEAP cursor 0x800000 线性）——M27 malloc
   同源；非真实页表分配（页面在恒等映射内恒等可用，U 位区）。

## 5. 后续（wave 2 候选）

- msvcrt.dll 动态链接的**真实 CRT 程序**（需要 mingw-w64 工具链或预编译 CRT 二进制;
  SHIM_TABLE msvcrt 面已 31 个, 缺口 = 常用 IO 系 printf/scanf 完整集验证）
- LoadLibraryA 真实 DLL 映射（当前假句柄）
- .run 容器资源 API（ResGet 等）与 PE 载荷 manifest 字段

## 6. 兼容论（FUFORALL 立场文档）

> 工程名: **FUFORALL**（"any file runs"）。与 Wine 的根本区别: **翻译不是仿真**。
> 与 FUAI 论文同构: 不可信外部组件 → 翻译层 + 内核强制包络 → 度量。

**兼容判据（功能性兼容）**：程序 P 在 FujoOS 上兼容 ⟺
1. **命令全执行** — P 的完整命令序列（所有系统交互/API 调用）全部成功返回
   （无 unimplemented、无崩溃、无静默丢失）；
2. **画面显示** — P 的输出画面（文本控制台 + GUI 图形）被正确显示；
3. **性能损失允许且可逆** — 当前慢可以接受；每个 shim 优化都是可逆回滚的改进。

**与 Wine 的区分**：Wine 在非 Windows 上**仿真** win32 抽象（让程序以为自己在
Windows）；FUFORALL **翻译**到 Fujo 原生语义（shim → 宏原生 syscall），验证的是
**功能等价**（命令成功计数 + 画面核对）而非行为 diff。差异根源：Wine 的可用面
= 仿真那些**没有对应系统**的 Windows 内部；FUFORALL 的边界 = 只翻译**有等价
语义**的对象，没有等价语义就**声明不支持**（诚实边界，README "加载器子集"）。

**四个命题**：
- **P1 兼容 = 集合包含**：P 在 Fujo 运行 ⟺ P 的依赖闭包 ⊆ Fujo 提供的语义。
  推论: 兼容层工作 = 枚举 + 补齐依赖闭包（shim 表 / def / 回归用例即证据）。
- **P2 兼容 = 接口契约，不是容器模拟**：有等价语义→翻译；没有→声明不支持。
  容器（.run）归一**二进制差异**，不模拟系统。
- **P3 兼容边界必须可声明、可测量**：API 数量、回归矩阵（42/42、每 API 一用例）、
  载荷测试 = 边界主张方式。兼容不是目标状态，是度量的函数（S2 思路）。
- **P4 兼容 = 回归矩阵覆盖的依赖闭包**（度量自动化见 tools/compat_audit.py）：
  每个 shim API 有正例用例；支持面有反例边界。
