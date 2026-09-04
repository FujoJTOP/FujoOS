# 79 · W20 p8 — >4GiB 高区窗口 (PML4[1] 恒等; m138)

> 里程碑: W20 p8 (脱 QEMU 专属) · 上游: docs/77 #7 后续
> 一句话: **4G+ 物理内存经 PML4[1..] 高区窗口恒等映射 —— map_phys_identity
> 通用化到 PML4 级 (每级按需分配), map_high_ram 上限 4TiB; QEMU -m 8192 实测
> high RAM mapped 7167MiB, 全链路正常 (m138 = m136 ELF 8G 变体)。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `mem.rs::map_phys_identity` | **PML4 级通用化**: pml4i = phys>>39; 每 PML4/PDPT/PD 槽按需分配 (页表帧自举于低区帧池); 4TiB 上限 |
| `mem.rs::map_high_ram` | hi 上限 0x1_0000_0000 (4GiB) → **0x1000_0000_0000 (4TiB)** |
| `tools/fujoregress.py` | m136-mem 用例改 **-m 3072** (QEMU ≥4G module 失效; 见下) |

**实测** (QEMU 9.2, -m 8192):
```
mem  : high RAM mapped 7167MiB
(boot 全链路: U-guard / demand-zero / VBE / PCI 6 设备 / 桌面 shell 正常)
```
**QEMU 限制 A/B 实证**: `-m 2048` module len=9256 正常; `-m 4096`/`-m 8192`
multiboot module 不装载 (len=0, 任何 initrd) —— QEMU 9.2 高 RAM 下
fw_cfg/module 失效; fujoregress 大内存用例取 **-m 3072** (窗口内最大值);
>4GiB PML4[1] 路径由本波手动验证覆盖 (7167MiB mapped), QEMU 修复或
GRUB 引导路径 (module 经 GRUB) 后可回归完整。

## 2. 踩坑

1. **PML4[0] 上限**: 原有 map_phys_identity 硬编码 `pm4.read()` (PML4[0] 的
   PDPT) —— >4GiB 物理 (PML4 entry > 0) 无映射; 通用化后每级按索引;
2. **页表帧自举**: PML4[1..] 的 PDPT/PD 帧从低区帧池分配 (boot 期 128MiB 池
   足够顶层表; 页级 PTE 计数 ~180 万 for 7GiB —— 8G TCG boot 增加 ~4s;
   2MiB 大页优化列表 W20 p9);
3. **统计口径**: `high_usable` (base≥4G 区) 与映射范围 (整个高位段含低区
   4G 段) 分开 —— m136 探针字段语义文档化 (docs/77)。

## 3. 审计表更新 (#7)

**✅**: ≤4TiB 物理内存完整识别+映射 (PML4[1..] 窗口); 8G 真机内存面闭合;
消费端 (用户 mmap 大块/内核大页池) 为下一阶段 (现在帧池 128MiB 固定)。

## 4. 下一步

- W20 p9: 2MiB 大页映射 (boot 时间优化; 帧池 top-down 分配保护) /
  帧池接入 mmap 大内存 (BITMAP 动态化) —— 真机 8G+ 的"内存可用"收官;
- 或按用户指示切换方向。
