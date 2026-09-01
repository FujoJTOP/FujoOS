# 17 — 帧时间表/性能计数器工具 (M68, v0)

状态: ✅ 完成。验收: QEMU 串口 `M68 RESULT: PASS`, demo `sdk/linux/m68_perf.c`。

## 1. 接口

| 编号  | 签名 | 说明 |
|-------|------|------|
| 0x6E01 | perf_frame_mark() | 帧边界标记; 与上次间隔入环形 (64 项) |
| 0x6E02 | perf_frame_stats(ptr) | u64×4: (frames, avg_us, max_us, sum_us) |
| 0x6E03 | perf_counter_enable(id, on) | 计数器开关 (0..7) |
| 0x6E04 | perf_counter_read(ptr) | u64×8 全部计数 |

## 2. 实现

- 时间基准: timer 两阶段校准 (cyc/us), `perf::init()` 首调 arm;
  首个 mark 触发校准 (日志 `timer: calibrated cyc/us≈2495`)。
- 帧汇总: 环形 F_TAB[64] + F_SUM/F_MAX 增量维护。
- 计数器挂钩:
  - **0** PIT IRQ — `irq::note()` 每 tick;
  - **1** syscall — `fujo_syscall_dispatch` 顶;
  - **2** ctx-switch — `sched` 切换点;
  - 默认启用 0/1; 2..7 由用户程序 open/close。
- 修过 bug: 帧计数原为 `F_N.min(63)` (恒 0) → 改 `F_N+=1` 上限 64。

## 3. 实测 (m68_perf.elf)

```
m68: frame timeline + perf counters v0
timer: calibrated cyc/us=2495
m68: frames=00000004 avg=00014494 max=00014d6f
m68: d_irq=00000008 d_sys=00000001 d_ctx=00000000
m68: M68 RESULT: PASS
```

- 5 次 mark → 4 条间隔 (avg≈83ms, max≈85ms — 20M 忙循环);
- 计数器差分: 8 次 PIT IRQ (80ms)、≥1 次 syscall 读取;
- 单任务 → d_ctx=0 (正确)。

## 4. 后续用途

- M69 2D 游戏#2: 帧时间表直接对标 (target 16.6ms, benchmark 面板);
- M70 验收报告: 用计数器 (IRQ/syscall/ctx) + 帧表给出平均/最坏。
