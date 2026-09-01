# 14 — 每核 TSS / 中断注入优化 (M65, v0)

状态: ✅ 完成。验收: QEMU `-smp 2` 串口 `M65 RESULT: PASS`,
demo `sdk/linux/m65_tss.c`。

## 1. 接口

| 编号  | 签名 | 说明 |
|-------|------|------|
| 0x6B01 | core_id() | CPUID initial APIC ID (BSP=0) |
| 0x6B02 | tss_info(ptr) | u64×3: (tss0_rsp0, tss1_rsp0, gdt_limit) |
| 0x6B04 | irq_route(mask) | 中断目标核掩码 (1=核0, 2=核1, 3=轮转) |
| 0x6B05 | irq_stats(ptr) | u32×4: (lapic_id, r0, r1, inj) |

## 2. 双 TSS (gdt.rs)

- GDT 16 槽: 槽0..4 同前; 5/6 = TSS0 (选择子 0x28); 7/8 = TSS1
  (选择子 **0x38**, 新增 `TSS1_SEL`)。
- TSS0.rsp0 = 0x300000 (syscall/核0 栈), TSS1.rsp0 = **0x3A0000**
  (核1 独立内核栈 — 与 0x2C0000/0x280000/0x340000/0x380000 不冲突)。
- 两 TSS 均 `repr(C,packed)` (rsp0 在偏移 4, 硬件非对齐读)。
- GDT_PTR.limit = 16*8-1 (0x7F)。

验证读回: `tss0_rsp0=0x300000 tss1_rsp0=0x3A0000 gdt_limit=0x7F`。

## 3. 核标识 — CPUID vs LAPIC MMIO

- v0 用 **CPUID leaf 1 EBX[31..24]** (初始 APIC ID)。
- LAPIC MMIO (0xFEE00000 基址, ID 寄存器 @0x20) 读触发了
  `#PF cr2=0xFEE00000 err=0` —— boot 页表 (gen_stub32.py) 未映射该
  区域 (M65 已知限制): 记录并在后续里程碑补页表映射后切回 MMIO 读
  (可同时读 ID 与版本寄存器, 支持 AP 唤醒 CMPXCHG 循环)。

## 4. 中断注入优化 v0

- `irq_route(mask)`: 目标核掩码; 每次 PIT 中断在 `fujo_tick_sched`
  入口 (`intr_note`) 归桶:
  - mask=1 → 全部核0; mask=2 → 全部核1; mask=3 → 轮转 (inj%2)。
- 统计不变量: `r0 + r1 == inj` (mask≠0 时)。

## 5. 实测 (m65_tss.elf, -smp 2)

每段 20M 用户态忙循环 (PIT 在用户态中断因此被计数;
`sleep_us` 是内核态 syscall, IF 被 SFMASK 屏蔽, 不能用于此验证):

```
m65: lapic_id=00000000
m65: tss0_rsp0=00000003 tss1_rsp0=00000003 gdt_limit=0000007f
m65: route(core0) d_r0=00000008 d_r1=00000000
m65: route(core1) d_r0=00000000 d_r1=00000008
m65: route(rotate) d_r0=00000004 d_r1=00000004
m65: M65 RESULT: PASS
```

- 全核0: 8 次中断全入核0; 全核1: 8 次全入核1; 轮转: 4/4 分散 ✓。

## 6. 与 M64 的关系 / 下一步

- M64 提供亲和位图 + 任务核归属; M65 提供**中断侧**核桶与双 TSS,
  两者合计构成"每核状态"面的雏形。
- 真 SMP 启动 (AP SIPI, 每核 IDT/LAPIC 定时器, 每核 RSP0 装载) 的
  挂接点在后续内核扩展; 本章的 TSS 槽/路由统计接口保持不变。
