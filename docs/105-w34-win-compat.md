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
