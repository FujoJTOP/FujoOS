# 88 · W27 — 哨兵接管真实事件流 (0x8312 ev_digest, m147)

> 里程碑: W27 (AI 垂直 VI) · 上游: docs/82 (哨兵只处理 demo 文本) · 计划: docs/83
> 一句话: **内核 0x8312 ev_digest 从真实事件环生成摘要 (最近 100 ticks 事件速率 +
> 最近事件 pid/kind) —— 哨兵第一次感知系统自身: 事件风暴 (rate=99) → 自动识别 →
> 自动隔离 → 速率归零 → 判定恢复, 全链路由真实事件驱动。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `ctx.rs::ev_stats_recent(win)` | 事件环统计: 窗口内 (now-ts<=win) 事件数 (cap 99) + 最近事件 (pid, kind); 环 128 槽扫描 |
| `ai.rs::fujo_ev_digest` (0x8312) | 摘要文本 "ev pid=<p> rate=<r> wr=<kind>" (syscall/file/win/exit/anom/none), 用户缓冲返回 |
| `syscall.rs` 0x8312 注册 | 事件流感知接口 |
| `sdk/linux/m147_storm.c` | 真实闭环: T0 LAUNCH 事件注入风暴任务 (0x8004 EV_SYSCALL 风暴) → T1 digest rate=99 → T2 哨兵 (0x8304) anom=1 conf>=50 → 自动隔离 → T3 digest rate=0 → 哨兵 anom=0 (恢复) |
| build-samples / fujoregress | m147_storm 注册 (回归 36→37) |

## 2. 实测

```
m147: T0 storm task tid=1
digest 'ev pid=1 rate=99 wr=syscall'     ← 事件环真实统计 (风暴)
m147: T1 storm rate=99 (expect >=90)
m147: T2 sentinel anom=1 conf=80         ← 哨兵识别 + 自动隔离 (conf>=50, cfg2=1)
digest 'ev pid=1 rate=0 wr=anom'         ← 隔离后: 风暴停止, 速率归零
m147: T3 after-isolate rate=0 (expect < storm rate)
m147: T3b sentinel anom=0                ← 恢复判定
M147 RESULT: PASS
```

**意义**: 哨兵从"演示喂文本"升级为"系统器官感知自身" —— 事件环是眼睛, digest 是
神经接口, 处置后系统状态改善可测量 (99→0)。这完成 FUAI 骨架里
"eyes → sentinel → hands" 的实链路 (此前 eyes 只服务模型上下文)。

## 3. 踩坑

1. **事件源选择**: 初始风暴用 `write(1, &x, 8)` 循环 → 二进制垃圾淹没串口日志
   + EV_SYSCALL 仅在 sys_note 每 1000 次采样时 push → rate 永远 7。
   修正: 风暴用 0x8004 (`ctx_inject`) 直推事件环 + 静默 (无输出污染)。
   **教训: 真实事件风暴的最小干净源 = 0x8004 注入, 不是 syscall 负荷。**
2. **测试假跑**: 首次运行 "PASS" 来自 fujoregress needle? 不 —— 首版 FAIL (rate 不够)。
   教训: 风暴信号必须 ≥ 阈值且可复现 (99 cap 稳)。

## 4. 状态

- **W27 PASS** (确定性), fujoregress 36→**37 用例**;
- W28 收尾 (六波证据汇总 + 全量回归 + 论文评估节重写)。
