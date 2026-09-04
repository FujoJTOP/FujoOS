# 82 · W22 — AI 垂直开发 I: 三引擎质量对照 + 自监督反馈闭环

> 里程碑: W22 (AI OS 的 AI 第一条垂直线) · 上游: docs/81 自我评价缺口
> (模型在环的质量对比为零证据 + 无反馈回路)
> 一句话: **同一份金标准样本集在 规则/模型/自动 三引擎下量化对比 ——
> 模型在规则边界外的边际价值第一次有实测数字 (anom novel +2/2, cls +2/6),
> 且 8/8 全绿 (fujoregress 31→33) + 模型在线 verify_ai PASS。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `ai.rs` **EVAL_FORCE 引擎强制门** (0x830F) | 6 个入口 (0x5101 classify / 0x8304-0x8308 五职责) 在 `rules_match` / shm 之前查询: `0=auto` (现状: 蒸馏→模型→规则) / `1=force-model` (跳过规则面直走模型) / `2=force-rules` (跳过全部直接确定性规则) —— 三引擎对照的实现基础 |
| `ai.rs` **anom 自监督验证位** | `fujo_anom_run` 自动隔离执行后 (iso_rc==0) 内核查 `task_state(pid)==2` (隔离态) → `fb_verified=1`; 审计环 (0x830D) 尾条 `result` 字段 = **建议被行动证据证实/证伪** 的 self-label (b 槽保留 iso_rc) |
| `ai.rs` **审计环边界修复** | `ai_aud_note` 中 `e[48+tlen]` 在 tlen=40 (env digest 满长) 时越界 -> PANIC; 改为 `if tlen < 40` 时才写 0 (存量 bug, W22 env 首撞) |
| `sdk/linux/m141_eval.c` | **三引擎质量评测**: 19 样本金标准集 (anom 8 = 4 known + 4 novel, io 5 novel 周期, cls 6 = 4 known + 2 novel) × 3 引擎; T0 链路探测 (novel 命令 "launch program": rules=0 / 模型=1, 带 COM2 降级); T1-T2 规则确定性断言; T3 对照表; T4 离线降级语义 |
| `sdk/linux/m142_feedback.c` | **自监督反馈闭环**: LAUNCH worker → cfg 开自动隔离 → 注入异常 (pid=tid) → 0x8304 自动隔离 → 审计尾条 result=1 (verified) + 0x8005 `t<id>:2` 双证据 → 正常事件 result=0 → resume 系统继续 |
| `tools/fujoregress.py` | m141-eval / m142-feedback 用例 (无模型确定性路径, <=30s/用例) |
| `scripts/build-samples.ps1` | m141_eval / m142_feedback 注册 |

## 2. 实测 (qwen2.5:7b 在线, verify_ai.py --demo m141_eval)

**T3 三引擎对照表** (19 样本金标准):

| 引擎 | anom (8) | io (5) | cls (6) |
|---|---|---|---|
| `[rules]` 强制规则 | 6/8 | 0/5 | 4/6 |
| `[model]` 强制模型 | **8/8** | 1/5 | **6/6** |
| `[auto ]` 蒸馏→模型→规则 | 8/8 | 1/5 | 6/6 |

**规则边界外的增量** (novel 子集, 规则确定性错/未知):
```
T1 rules anom-known=4/4 novel-pos=0/2 io=0/5   (规则: 已知满分, 边界外全错)
T3 model novel-pos anom 2/2 (rules baseline 0/2)  ← 模型增量: +2 异常检出
cls novel: "launch program"→1(RUN)✓ "what is the time"→2(QUERY)✓ (rules 0/2)
```

**结论 (诚实)**:
- 模型在**规则覆盖内**的增量 = 0 (auto==rules 语义, R5 蒸馏优先正确);
- 模型在**规则边界外**有真实增量: anom novel-pos 0/2 → **2/2**, cls 0/2 → **2/2**;
- 模型**不通吃**: io 周期预测 7b 仅 1/5 (NEXT=6 越界/答 3/4 错) —— 该职责不宜交给模型
  (规则 last-block 0/5 也差, 双引擎均低于基准 → io 预测器是真实的开放问题, 不是模型能轻易解决);
- **0.5b 对照**: 全答 RUN (cls 全 1) / NEXT=6 → 0.5b 在该任务集不可用, 质量曲线随模型规模陡升
  (该次运行 M141 FAIL 因走 auto 断言, 但对照数字本身成立 —— 见 §3)。

## 3. 踩坑

1. **0x8309 probe (shm-only) 对链路竞态不宽容**: pmemsave 慢/帧漂移时 engine=2 误判
   offline → 跳过 [model] 全量; 换 0x5101 novel 命令探测 (rsp bad 时 COM2 重发降级),
   判定更稳。文档留: 链路探测别用无降级的探针。
2. **T2 断言必须锁定引擎**: 原 design 里 plan/nlc/env 断言在 auto 下执行, 模型在线时
   模型输出不确定 → FAIL; 修正 = T1/T2 全程 force=2 (断言只针对确定性语义),
   模型/自动对照只在 T3 记录。**教训: 评测 demo 的断言区域与记录区域必须分离。**
3. **存量 audit 环 88B 越界**: `e[48+tlen]=0` tlen=40 时 index 88 (len 88) → PANIC;
   env digest 首次达到 40B 触发 (m115 env 从未入回归矩阵, 从未撞上)。
4. **0.5b vs 7b 对照**: 同 prompt 同链路, 0.5b classify 全 1 / io 全 6 —— 模型质量
   标尺必须固定 ≥7b, 小模型结论 (如 m112 时代) 应重新标定。

## 4. 状态与后续

- **W22a 全绿**: m141 + m142 PASS (offline 确定性) + verify_ai 7b 在线 PASS;
- **fujoregress 31 → 33 用例** (m141-eval, m142-feedback);
- **后续垂直开发候选** (W22 同线延伸):
  - io 预测器: 双引擎均 <21% → 重新设计特征 (非 last-num), 或降级为"记录型"职责;
  - 自监督标签接入蒸馏候选: m142 已验证 verified 位; 下一步把 novel-pos 命中样本
    自动导出 -> train_cases 补充 -> FJRU 重编 -> **调用率下降曲线** (R5 闭环完整化);
  - 模型质量曲线: 同集 × {0.5b, 7b, 更大} 的准确率-规模表 (论文 §Evaluation 证据);
  - 对抗测试: 恶意回复注入 (model server 可编程) → blast radius 域拦截实测。
