# 103 · B22 — 判别式确定性归纳器（induct v2，A4 间隙解决）

> 里程碑: B22 (docs/98 A4: induct_rules v1 判别间隙) · 上游: W33 A4 (4 candidates → 3 rules, 但 'ev '→1 过泛)
> 一句话: **induct_rules v2 改为"正负样本判别归纳"——needle 候选必须不命中任何负样本、
> 且命中正样本时值一致；输出词级 needle（wr=memleak / wr=zombie / launch / what is），
> 负样本误判 0/80、正样本 fidelity 100%，'ev ' 过泛规则消失。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `tools/gen_neg_samples.py` | 确定性负样本集（anom duty=2, value=0）：A known 负 20 条 (rate<10 wr=ok\|1) · B novel-neg 50 条 (rate 40–89 wr=ok, 封死 'rate=7x/8x' 过泛) · C 判别边界 10 条 (rate=9x wr=ok) = **80 条** |
| `tools/induct_rules.py` v2 | 判别归纳：词边界连续词序列候选 (span 1–3, minlen≥6, 短到长) → 满足 (1) 不命中负样本 (2) 命中正样本值一致 → 最快安全候选；输出与 distill_rules.BAKED 同构 |
| `sdk/rulebook/neg_samples_w34.json` | 80 条负样本（复现：gen_neg_samples.py） |
| `sdk/rulebook/inducted.json` | v2 归纳结果（复现：induct_rules.py --in train_cases_w23.json --neg neg_samples_w34.json） |

## 2. v1 → v2 对比

| | v1 (W33) | v2 (B22) |
|---|---|---|
| 候选 | 跨样本公共子串（含跨词/半数子串） | 词边界连续词序列（词级可读） |
| 选择 | 频率最高 | 最短安全（满足负样本/值一致约束） |
| 负样本 | 无 → "ev "→1 过泛（一切 ev 前缀判 1） | 80 条负样本 → 判别约束 |
| 结果 | 'ev '→1 / 'lau'→1 / 'wha'→2 (4 条) | 'wr=memleak'→1 / 'wr=zombie'→1 / 'launch'→1 / 'what is'→2 (4 条) |
| 验证 | fidelity 100% (候选集内) | fidelity 100% + **neg 0/80** (判别性显式断言) |

## 3. 数据（v2 运行输出）

```
[induct-v2] rules=4 coverage=100% fidelity=100% neg-mispredictions=0/80
[induct-v2]   duty=2 needle='wr=memleak' -> 1
[induct-v2]   duty=2 needle='wr=zombie' -> 1
[induct-v2]   duty=1 needle='launch' -> 1
[induct-v2]   duty=1 needle='what is' -> 2
```

## 4. 坑 (B22)

1. **负样本覆盖盲区**: v2 首版负样本仅 A+B(40-58) → 'rate=77' 逃逸（'rate=77'→1 会误判 "rate=77 wr=ok" 正常事件）——B 段扩到 40–89 后封闭。
2. **minlen=4 的过短 needle** ('e=77') 可读性差且弱判别 → 词边界候选 + minlen 6。
3. **最长优先 → 全文 needle**（泛化差）→ 最短安全优先（词序列 span 1 起）。
4. **无关消费链**: inducted.json 只进 distill_feed 流（runner 候选），不进 fujoregress/fjru.bin — 全量回归 40/40 确认无扰动。
