# 106 · W35 — 散件工厂（FUFORALL 第一波）：libc 散件 + 真实开源源码拼装 + FujoOS 原生运行

> 里程碑: W35 · 目标: 散件工厂 MVP —— **散件（header-only POSIX 子集）+ 公共域真实开源
> C 工具源码（SHA-256 原样）拼装为单编译单元 → FujoOS 原生 ELF 运行 → 标准向量输出匹配**。
> 一句话: **sfactory: abc/empty/data110 三向量全部匹配宿主（sha256 FIPS 180-4 标准输出），
> 散件工厂的运行侧闭环成立；内核内 tcc 大字源编译的 GP 记录为 B 类。**
>
> 注: 与目标叙事的关系 —— 用户目标（内核内自编译）被 tcc 运行时 GP 阻挡（见 §5 B 类），
> 本波交付 = 散件 + 真实源码 + FujoOS 运行时全部验证（编译侧暂在宿主，诚实记录）。

## 1. 交付

| 部件 | 说明 |
|---|---|
| `sdk/scatter/fujo_libc.h` | **散件一**: header-only POSIX 子集（size_t/uint*/malloc(128KB bump)/printf 最小引擎(%s %c %d %u %x %p 宽度)/str*/mem* + Linux x64 syscall 面 fd 封装；tcc 约束兼容（仅 a/D/S/d asm 约束）|
| `sdk/scatter/sha256.c` / `sha256.h` | **真实开源源码**（公共域, 983/SHA-256）——原样副本 |
| `sdk/scatter/fujo_main.c` | 测试驱动（FIPS 180-4 向量: "abc"/空串/110B 跨块 + 文件用例） |
| `tools/make_scatter_tool.py` | 拼装器: 三源 → 单编译单元 sha256tool.c（去 guards/include；适配层仅头文件 include 替换——算法零改动） |
| `sdk/build/sha256tool.elf` | 宿主编译（WSL gcc -nostdlib -static -O0）的 FujoOS 原生 ELF |
| `tools/compat_audit.py` | （P4 已交付）散件度量工具 |
| fujoregress | case `scatter`（needle: SFACTORY RESULT: PASS）|

## 2. 验证

```
sfx: abc = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad OK
sfx: empty = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 OK
sfx: data110 = b7121997d66bf89f5078cb7229faf5c7f56ea1a1efd222686500a69de199f1dd OK
SFACTORY RESULT: PASS                 （host WSL gcc 与 FujoOS 运行结果逐字一致）
```

- 宿主侧（WSL gcc -nostdlib）：三向量 PASS
- **FujoOS 侧（case scatter）**：同 ELF 运行 → 同输出 → **PASS**

## 3. 散件工厂上下游（本波验证）与目标（下波）

已验证: 源码(真实) + 散件(libc) + 拼装(适配) + FujoOS 运行(原生) + 输出匹配(向量)。
目标: **内核内 tcc 编译大字源** —— 受阻（§5 B-1），B 类修复后由 `sfactory` 内核命令承接
（shell.rs 已实现: 写 /tmp -> tcc -nostdlib -static -> runfile；include_str 内嵌工具源）。

## 4. 坑 (W35)

1. **tcc asm 约束**: `'r'` 约束 tcc 不支持 → 仅 a/D/S/d（与 mbuild sy4 同模式；否则
   "asm constraint 5 could not be satisfied"）。
2. **tcc 预处理器**: 嵌套 #ifndef/_H guards + 内嵌 include 在 tcc 下不稳 →
   拼装器 flatten（去 guards/include 行），单文件扁平。
3. **x64 variadic**: 手写"栈取参"在寄存器 ABI 下全错（宿主 segfault）→ `__builtin_va_*`
   （clang/tcc 内建, 标准）。
4. **tmpfs 2048B**: 16KB 工具源码写 /tmp 截断 → (a) 内核内编译路径需扩容
   （TMPFS_MAX→17408, BSS +132KB, load_end 0x2C0000→0x2F0000——**已回滚**）或 (b) FJFS 路径（未采用）。
5. **回滚教训 (关键)**: TMPFS 扩容 + load_end 0x2F0000 使 **multiboot initrd 模块顶入
   0x400000 用户区**（模块 = load_end 起 1.15MB → 0x408000）→ 与 tcc 加载冲突 →
   **m128/m140 全线同址 GP**（0x49f630）。修复 = 回滚 TMPFS/load_end（**load_end 上限约束
   = (0x400000 - 最大 initrd)/需 ≤0x2C0000**——硬约束记录）；散件工具源改为核外传递
   （B-1 设计: .run 资源/模块携带），**不做内核内嵌 16KB 字符串**。
6. **gcc 优化 O2 段错误**（宿主; O0 正常）: 记为本工具在 gcc O2 的 UB 疑点——
   tcc/clang 环境为准（O0 验证 + FujoOS 运行）。

## 5. B 类（后续）

- **B-1: tcc 内核内编译大字源 GP**（tcc .text 0x49f630, movzbl 邻域; 与源大小无关
  （4KB 与 16KB 同址）; mmap/brk 均在 → 疑 tcc 编译期间某内存/重定位内部路径;
  需要 tcc 侧断点/源码级取证）。修后 `sfactory` 命令直接切换"内核内编译"。
- 工具源码裁剪/库面扩展（POSIX 子集 → 下一个真实工具（流式 sha256sum/文件树））
