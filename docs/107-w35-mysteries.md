# 107 · W35 未解之谜记录（散件工厂波 · 含 B-1 修正）

> 上游: docs/106 (W35 散件工厂) · 目的: 把本波留下的未解项与"自主执行中可能误判的结论"
> 记录在案, 每条带复验路径。私有镜像: D:\Dev\FujoOS-private\docs\107-mysteries.md (B20 段落)。

## M-1 (主谜, 修正版) — tcc 内核内编译 GP 的根因判定被污染

**现象**: tcc-static 编译工具源码时内核报 `EXC user vec=13 rip=0x49f630`（tcc .text 内）。

**过程与修正**:
1. W35 初期: 16KB 源 GP → 判定"与源大小无关"（4KB 同址）→ 归为"tcc 内部问题"
   → 交付降级为"宿主编译 + FujoOS 运行"。
2. 回滚 load_end (0x2F0000→0x2C0000) 后: **m128 (300B hello) 立刻 PASS**——
   → 300B 的 GP 其实是 **multiboot initrd 模块顶入 0x400000 用户区**（load_end 冲突）—— **与 tcc 无关**。
3. **缺陷**: 4KB/16KB 的"同址 GP"测试全部发生在冲突态 load_end=0x2F0000 时——
   **没有在回滚态对大字源做过对照实验** → "tcc 内部问题"的定性**证据不完整**。

**修正后的判定**: B-1 大概率 = 同一个 load_end 冲突；若把工具源改为**核外传递**
（.run 资源 / FUJOMULT 模块携带, 不 include_str 内嵌内核）且 load_end 合规,
**16KB 工具大概率能在内核内编译成功**。

**复验实验（下一波第一个, ~30min）**:
```
1) 恢复 sfactory 命令（源从核外模块/资源读, 不内嵌）
2) load_end = 0x2C0000（合规态）
3) tcc 编译 16KB sha256tool.c -> 期望: 无 GP, 编译产物 runfile -> 向量 PASS
```
若通过 → "内核内自编译" 闭环直接落地（docs/106 的 B-1 从"受阻"改"已解"）。

## M-2 — include_str 内嵌 17KB 字符串, __kernel_end +0x5000 (20KB)

- 现象: `include_str!("sdk/scatter/sha256tool.c")`（字符串应属 rodata）使 BSS 尾
  0x2BEC30 → 0x2C3C30 (+0x5000), 且移除后回落。
- 未解: rodata 常量为何涨 BSS（Rust include_str 常量 placement/对齐细节?）。
- 规避: 不用内嵌（核外传递/M-1 路径）。复验需要 rustc 对象级检查（lld --gc-sections 剖面）。

## M-3 — 宿主 WSL gcc O2 段错误 (O0 正常)

- 现象: 同一 sha256tool.c, `gcc -nostdlib -static -O0` 三向量 PASS,
  `-O2` 段错误（首次 data110 后 segfault）。
- 未解: 源码 UB（未初始化/严格别名）或 gcc 优化交互。影响: 宿主验证用 O0;
  真环境（tcc/clang）无此现象。复验: -fsanitize=undefined O2 跑。

## 老谜汇总（仍挂 / 已解标注）

| 谜 | 状态 | 待办 |
|---|---|---|
| B-1 tcc 内编译（M-1 修正） | ⏳ 待重验 | 核外源 + 合规 load_end 对照实验 |
| qwen3:4b 慢 100× + 盲区 0/10 | 机理空白 | B27（ollama 后端取证） |
| m134 干净盘首读 0 | AHCI 首调 DMA 竞态特征 | B26（QEMU 状态机剖析） |
| io=0/30 | ✅ 已解（B20: 真实能力缺失, 非解析伪影） | — |
| spin 120M TCG 耗时 | 环境特性（记录） | — |

## 教训（本波）

1. **机�制陷阱会伪装成"组件 bug"**: load_end 冲突的 GP 在 tcc 内出现,
   且与源大小无关 → 极易误判为 tcc 问题。**回滚对照实验必须做全**（回滚后重测同场景）。
2. **"调度"降级要带对照**: 自主执行中为交付而降级（宿主编译）时,
   应记录"降级是否可逆"与"回滚态重验项"——本波的 B-1 复验实验即来自此。
3. **BSS 硬约束**: load_end ≤ 0x2C0000（= 0x400000 - 最小 initrd 余量）——
   已写入 docs/106 坑 #5; 任何 BSS 扩容前先算模块冲突。
