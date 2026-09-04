# 86 · W25 — I/O 预测职责所有权重判 (二阶马尔可夫基线, m145)

> 里程碑: W25 (AI 垂直 IV) · 上游: docs/82 (W22 io 双引擎均差) + docs/84 (FJRU
> 无算术编码) · 计划: docs/83
> 一句话: **内核新增二阶马尔可夫 I/O 基线 (io_markov, engine=4, 自训练访问流):
> 周期流 3/5→5/5 零模型调用, 而 7b 模型仅 1/5 —— io 职责所有权 = 确定性基线,
> 模型在基线 miss 时仅作辅助。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `ai.rs::io_markov` | **二阶马尔可夫预测器** (engine=4): 每次预测调用把序列数字追加进自训练访问流 (最近 64 项窗口, 96B static), 反向扫描 (a,b)→c 转换表取最近后继; 无后继 = None。零模型依赖、确定性、96B 开销 |
| `ai.rs::fujo_io_predict` | 引擎顺序重构: rulebook → **markov (auto 中优先于模型)** → 模型 → last-num; force=2 = markov+last (确定性); force=1 = 纯模型 (不污染流) |
| `sdk/linux/m145_io_own.c` | 所有权重判: T1 [rules] 5 周期样本流学习; T2 [auto] 基线优先 + 模型调用计数; T2b 审计 engine=4 标识; T3 [model] 在线对照 |
| `m141_eval.c` | T1 io 断言放宽 (W22 断言 last-num 0/5; 现基线升级为 markov, 由 m145 专测) |
| build-samples / fujoregress | m145_io_own 注册 (回归 34→35) |

## 2. 实测

**离线 (fujoregress, 无模型)**:
```
m145: T1 [rules] io=3/5 (W22 last-num baseline 0/5)
m145: T2 [auto] io=5/5 model-calls+=0   ← 基线全命中, 零模型调用!
m145: T2b io audit markov=8 nonMarkov=2
M145 RESULT: PASS
```

**在线对照 (qwen2.5:7b)**:
```
m145: T1 [rules] io=3/5
m145: T2 [auto] io=5/5 model-calls+=0
m145: T3 [model] io=1/5   (纯模型, 与 W22 一致)
```

**ownership 矩阵结论** (职责 × 引擎):
| 职责 | 蒸馏规则 | 确定性基线 | 模型 | 所有权 |
|---|---|---|---|---|
| anom | 已知 4/4 | rules 6/8 | **novel 2/2 增量** | 蒸馏 + 模型 (边界外) |
| cls  | 已知 6/6 | rules 4/6 | **novel 2/2 增量** | 蒸馏 + 模型 |
| io   | 2/5 (编码受限) | **5/5** | 1/5 | **确定性基线** (模型仅辅助) |
| plan/nlc/env | 编译正确 | 确定 | 建议面 | 蒸馏优先 |

## 3. 踩坑

1. **流窗口越界**: io_markov 反向扫描起点必须 `N-3` (保证 j+2 < N), 否则
   最后二元组在序列内部时读到流外垃圾 (首版设计隐患, 实现已规避)。
2. **流污染**: force=1 (纯模型) 不应追加流 (markov 未被调用即不追加 ✓);
   m145 引擎顺序 [rules]→[auto]→[model] 保证模型对照不受前序流污染。
3. **m141 断言联动**: T1 的 io=0/5 是 last-num 语义断言, 基线升级后失效;
   拆分职责: m141 管综合三引擎表, m145 管 io 专项。

## 4. 状态

- **W25 PASS** (离线 + 在线), fujoregress 34→**35 用例** (全量确认随 W26 一起);
- 后续: W26 五职责全自监督 (plan/nlc 后果验证) → W27 真实事件流哨兵 → W28 收尾。
