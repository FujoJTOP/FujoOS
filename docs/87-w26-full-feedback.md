# 87 · W26 — 五职责全自监督 (plan/nlc 动作后果验证, m146)

> 里程碑: W26 (AI 垂直 V) · 上游: docs/82 (W22 anom 验证位) · 计划: docs/83
> 一句话: **内核 act_verify 为每个 plan 动作执行后立即查系统状态确证效果
> (KILL→dead / ISOLATE→isolated / RESUME→runnable / SET_CFG→读回 / ACK→pending 清),
> nlc 每条策略验证 cfg 读回 —— 五职责审计 result 字段全部成为 self-labeled
> verified 计数 (anom W22 + plan/nlc W26)。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `ai.rs::act_verify` | 动作后果验证: KILL→state==3 / ISOLATE→state==2 / RESUME→state==1 / SET_CFG→cfg 读回==a1 / ACK→anom_pending()==0; 未定义动作→0 |
| `ai.rs::fujo_plan_run` | 每个 rc==0 的动作后 `n_verified += act_verify`; 审计尾条 result=n_verified (b 槽保留 verify 标志) |
| `ai.rs::fujo_nlc_set` | 每条策略 cfg_set 成功后 `cfg_get(k)==v` → verified++; 审计 result=n_verified |
| `ai.rs::anom_pending` | 导出待确认异常计数 (act_verify ACK 用) |
| `sdk/linux/m146_full_fb.c` | T1 isolate+resume (ok=2, verified=2) → T2 nlc (applied=2, verified=2, cfg3=1) → T3 kill (verified=1) → T4 正常事件 (verified=0) |
| build-samples / fujoregress | m146_fullfb 注册 (回归 35→36) |

## 2. 实测

```
m146: T0 worker tid=1
m146: T1 plan ok=2 fail=0 audit duty=3 verified=2   ← isolate→state2 + resume→state1 均确证
m146: T2 nlc applied=2 audit duty=5 verified=2 cfg3=1  ← cfg3=1/cfg4=2 读回验证
m146: T3 plan-kill ok=1 verified=1                   ← state==3 确证
m146: T4 normal audit duty=2 verified=0              ← 无误报验证
M146 RESULT: PASS
```

**五职责自监督闭环矩阵** (审计 result 字段语义, 全部 self-labeled):
| 职责 | 验证方式 | 标签 |
|---|---|---|
| anom (W22) | 隔离执行后 task_state==2 | fb_verified |
| plan (W26) | 每个动作后状态/配置确证 | n_verified 计数 |
| nlc (W26) | cfg 读回==目标值 | n_verified 计数 |
| io (W10 R6) | 上次预测 vs 本次实际块 | PREV_IO hit |
| env | 无外化动作 (仅记录 profile) | — |

## 3. 状态

- **W26 PASS** (确定性); fujoregress 35→**36 用例**;
- 后续: W27 哨兵接管真实事件流 (ev_digest) → W28 六波证据收尾。
