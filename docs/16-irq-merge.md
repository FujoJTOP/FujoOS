# 16 — 中断合并/减轻 (M67, v0)

状态: ✅ 完成。验收: QEMU 串口 `M67 RESULT: PASS`, demo `sdk/linux/m67_irq.c`。

## 1. 接口

| 编号  | 签名 | 说明 |
|-------|------|------|
| 0x6D01 | irq_set_window(w) | 合并窗口 1..64; 基点重置 (批数归零) |
| 0x6D02 | irq_cost_stats(ptr) | u64×4: (irqs, batches, total_cyc, worst_cyc) |

## 2. 语义

- **逐 tick 时钟/调度不变**; 合并层只做双账:
  `batches = (IRQS - W_START) / WINDOW` — 每 W 个 tick 计一次"组批"。
- **成本预算**: 相邻 PIT IRQ 的 rdtsc 间隔累计 total 与 worst →
  量化"高频中断"对执行面的侵入 (TCG 虚拟时钟下同样有效)。
- 窗口切换 (irq_set_window) 把基点置为当前 IRQS → 批数从 0 重新计
  (v0 曾用累计取模, 跨越窗口边界后相位错乱 — 修正为公式计算)。

## 3. 实测 (m67_irq.elf)

```
irq  : merge window=1 irqs=1637 batches=1637   (set 后基线)
irq  : merge window=1 irqs=1645 batches=8       (逐 tick: 8/8)
irq  : merge window=8 irqs=1653 batches=1       (组批: Δ8 → 1)
m67: w1 d_irqs=00000008 b=00000008 w8 d_irqs=00000008 b=00000001
m67: M67 RESULT: PASS
```

## 4. 串口/网卡中断合并面 (v0 记录)

- 串口 (16550 COM1): 硬件合并 = **FCR 触发阈值** (FIFO 深度
  1/4/8/14) — 当前内核用轮询 (无 IRQ4), 上 FIFO 阈值中断后
  合并面即接 `irq_set_window` 策略。
- 网卡: 无硬件网卡; 82574L 类设备的 MSI-X 多向量 + 中断合并表
  (ITR/IVAR) 为预留; 触发面接同一记账层。
- 后续内核: 中断节流 (tick 合并 + 软中断延迟) 以本记账为依据。
