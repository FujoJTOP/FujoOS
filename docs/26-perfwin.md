# 26 — 性能计数器窗口 (M77)

状态: ✅ 完成。验收: QEMU 串口 `M77 RESULT: PASS`, demo `sdk/linux/m77_win.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7801 | win_begin(id) | 快照 (timer_us, IRQ 计数, syscall 计数) |
| 0x7802 | win_end(id) | 差分 → 窗口记录 |
| 0x7803 | win_read(ptr) | u64×4: (us_delta, irq_delta, sys_delta, calls) |

## 2. 实现

- 时间基: timer::fujo_timer_us (两阶段校准, cyc/us);
- 计数基: M68 perf 计数器 CTR[0]=PIT IRQ / CTR[1]=syscall;
- 窗口: begin 快照 3 元组 + calls++; end 差分; 单槽 v0。

## 3. 实测 (m77_win.elf)

```
m77: perf counter windows v0
timer: calibrated cyc/us=2496
m77: us=00014497 irq=00000008 sys=00000001 calls=00000001
m77: M77 RESULT: PASS
```

20M 忙循环窗口: 82,967µs (~83ms), 窗口内 8 次 PIT 中断, 1 次
syscall (win_end 读回), 1 次窗口调用。

## 4. 用途

- 热点循环/帧节拍/中断侵入的量化窗口 (可与 M76 trace、M68 汇总、
  M67 成本预算交叉对照); M85 验收的数值面。
