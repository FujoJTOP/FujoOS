# 83 · AI 垂直开发计划 (W23–W28, 六波)

> 目标: 把 AI 器官从"可评测"（W22）推进到"可自进化、有所有权、受对抗验证"的
> 系统部件。每波: 内核面 + demo + 回归用例 + 模型在线验证 + docs + commit(含波次与回归数)。
> 基线: fujoregress 33/33 (W22)。

| 波 | 主题 | 交付 (demo + 接口) | 验收证据 |
|---|---|---|---|
| **W23** | 蒸馏闭环自动化 | m143 (novel-pos 命中 → 审计导出候选文本) + `tools/distill_feed.py` (候选→train_cases 增量→FJRU 重编→0x830B 载入) + 调用率对比 | 蒸馏后同 novel 样本规则全命中, 模型调用数下降 (0x830C), m143 PASS |
| **W24** | 对抗测试 (恶意模型) | `qwen_model_server.py` 恶意回复模式 (FUJO_EVIL=1) + m144 (显式域绑定 → 恶意 PLAN 注入 → 只执行授权动作, 其余 deny+审计) | 越权动作全部 deny, 授权动作执行, 审计含 deny 记录, m144 PASS |
| **W25** | IO 预测所有权重判 | 内核新基线 (二阶马尔可夫: 历史后继表; 缓存命中) + m145 (周期/混合序列 × {新基线, last-num, 模型}) | 新基线 ≥4/5 周期样本, 模型 ≤ 基线; 职责所有权矩阵结论 (哪个引擎配哪个职责) |
| **W26** | 五职责全自监督 | m146 (plan/nlc 动作后果验证 → 审计 result 标签; anom 已有 W22) | plan isolate→state2→verified, nlc enforce 拒绝生效→verified, m146 PASS |
| **W27** | 哨兵接管真实事件流 | 内核 `0x8312 ev_digest` (事件环→摘要文本) + m147 (真实异常任务 → digest → 哨兵自动分类 → 自动隔离) | 事件驱动哨兵检测异常任务并处置, 系统继续, m147 PASS |
| **W28** | 六波证据收尾 | docs/81 评估节重写 (质量表/蒸馏曲线/对抗矩阵/所有权矩阵) + docs/84-86 汇总 | 论文证据节完整, fujoregress 全绿 |

**波内纪律** (沿用): ① `cd kernel; cargo build --release` 见 "Compiling"; ② BSS 尾 < 0x2C0000;
③ fujoregress 全绿 + AI 波模型在线 verify_ai; ④ commit 含波次与回归数; ⑤ docs 同步。

**范围外** (此计划不做): TCP 客户端数据面 (QEMU slirp, zcode)、>4GiB 消费端、文献核对 (zcode)。
