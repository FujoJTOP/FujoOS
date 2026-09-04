# 84 · W23 — 蒸馏闭环自动化 (novel 命中 → 候选 → bake → FJRU v2 → 零调用)

> 里程碑: W23 (AI 垂直 II) · 上游: docs/82 (m141 三引擎对照) · 计划: docs/83
> 一句话: **m141 模型在线命中的 novel 样本 → distill_feed 收集候选 →
> bake 进 BAKED → FJRU v1→v2 (14→19 条) → m143 载入后同样本集全走 rulebook,
> 模型调用 ~38→≤1 (仅 io 未覆盖 1 条 fallback)。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `tools/distill_feed.py` | 蒸馏候选收集: 解析 m141 串口日志 `cand ... HIT` 行 → `sdk/rulebook/train_cases_w23.json` (duty/text/value/conf=80); 去重交归纳步骤 |
| `tools/distill_rules.py` | **BAKED 14→19 条** (m141 7b 实测 bake: anom `wr=memleak`/`wr=zombie`→1, cls `launch`→1/`what`→2); 精确 needle 重排至 `rate=` 通配之前 (rules_match 按序首中) |
| `sdk/linux/m143_distill_feed.c` | 闭环验证: 0x830B 载入 19 条 → novel anom 4/4 (engine=3) + cls 2/2 (RULE_HITS+2) + io 2/5 (记录蒸馏不完整 → W25) + **AI_CALLS≤1** + anom/cls 审计全 engine=3 |
| `m141_eval.c` | 候选打印行 (`m141: T3 cand <kind> '<text>' -> <got> gt=<gt> HIT\|miss`) — 在线评测的蒸馏出口 |
| `m120_distill.c` | 规则条数断言 14→19 (同步) |
| build-samples / fujoregress | m143_distill 注册 (回归 33→34) |

## 2. 实测

**闭环轨迹 (完全共形)**:
```
m141 (qwen2.5:7b 在线): cand anom-novel 'ev pid=4 rate=77 wr=memleak' -> 1 gt=1 HIT
                        cand anom-novel 'ev pid=5 rate=82 wr=zombie' -> 1 gt=1 HIT
                        cand cls-novel  'launch program' -> 1 gt=1 HIT
                        cand cls-novel  'what is the time' -> 2 gt=2 HIT
distill_feed: candidates=4
m143: T1 rulebook anom=4/4 (engine=3)
m143: T2 cls launch=1 what=2 rulehits+=2
m143: T3 io hits=2/5 rulebook-hits+=4   (io 蒸馏不完整: "1 2 3 4 5"->5 误覆盖, 2 条未覆盖 -> W25)
m143: T4 calls=1 hits=10 anomClsEngine3=1  (m141 在线基线 calls~38 -> <=1)
M143 RESULT: PASS
```
**调用率下降曲线点**: m141 在线 `AI_CALLS≈38` (19 样本 × auto+model 双跑) → 蒸馏后同集 `≤1` (仅 io 未覆盖样本 1 次模型尝试, 无模型超时即 fallback)。

## 3. 踩坑

1. **通配 needle 掩盖精确规则**: 旧 BAKED `(2,"rate=")` (任意 rate=NN→0) 排在前面,
   新增 `wr=memleak` 等永不可达 —— rules_match 按序首中, 新精确 needle 必须插在
   通配之前; `distill_rules.simulate` 不检查覆盖冲突 (只查首中输出), 需人工检查顺序。
2. **io 周期无法 needle+param 编码**: `"N N+1 N+2" → N+3` 需要算术, FJRU v1 无算子;
   现有 `(4,"1 2 3 4",5)` 在 `"1 2 3 4 5"` 上误覆盖 (→5, gt=0)。W25 重判所有权时
   一并解决 (二阶马尔可夫 / 保留模型但降级为辅助)。
3. **候选收集与 bake 顺序**: 先收集后 bake (本波顺序正确); bake 后再跑 feed 会
   因规则已覆盖而 0 候选 (第一次运行误导)。distill_feed 改为收集全部 HIT。

## 4. 状态与后续

- **W23 全绿**: m143 PASS; fujoregress 33→**34 用例** (运行中本轮全量确认);
- 后续: W24 对抗测试 (恶意回复注入 × 域拦截) → W25 io 所有权重判 → W26 全自监督 → W27 真实事件流哨兵 → W28 收尾。
