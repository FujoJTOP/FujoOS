# 59 · AI OS 接口规格初稿 (W8 · R1 公理化 + R3 时延一致性)

> 定位: W8 产出物 —— 把 AI 层四条隐藏不变式写成**公理**并给出可断言语义,
> 把"陈旧建议"从风险变成**协议**(快照@t0 + 增量区间 + 过期标记, 每步可测)。
> 该文档基于 docs/52 蓝图与 M112–M115 实测, 是论文《AI OS 的接口公理》的规格草稿。

## 1. 公理 (A1–A4) 与断言

模型输出永远只是 **hint**: 内核是最终裁判。四条公理 + 断言语义:

| # | 公理 | 断言语义 (内核可执行) |
|---|---|---|
| A1 | 模型永远不能执行未授权动作 | `cap_exec(act∉grant)` == -1, 无副作用 (cfg 不变), deny 计数 +1 |
| A2 | 每个动作都有审计记录 | 每次 `cap_exec` 落笔 `aud {ts, action=2, subject=act, result}`; 条目总数单调 +1 |
| A3 | 模型缺席时系统继续运行 | `rules_anom/plan/nlc` 对同一输入输出确定; 链路超时/丢弃 → engine=2 正常返回 |
| A4 | 每个"失败"被计数并降级 | deny → `DENIES` 计数; 引擎降级 qwen→rules 由 engine 字段可观察 |

**内核侧自检**: `0x830A fujo_inv_run(out) -> [mask, denies, aud_num]` — 纯内核执行,
模型离线也跑 (无链路等待)。mask bit0..3 = A1..A4 PASS。
前置条件: exec 槽未授权时调用 (引导默认); 调用后槽 6 授予 0x3F。
用户态封装: `sdk/linux/m119_inv.c` → needle `M119 RESULT: PASS` (已入 fujoregress, 离线跑)。

## 2. R3 时延一致性协议 (shm 帧 v2)

### 2.1 帧头 (0xA00000 起, ver=2, 48B)

| 偏移 | 类型 | 字段 | 语义 |
|---|---|---|---|
| 0x000 | u32 | magic | 0x48534A46 ("FJSH" LE) |
| 0x004 | u32 | ver | 2 (M118 起要求) |
| 0x008 | u64 | seq | 请求序号 (回包以该帧 seq 编号) |
| 0x010 | u32 | kind | 1=classify 2=anomaly 3=PLAN 4=IO 5=NLC 6=ENV |
| 0x014 | u32 | len | payload 长度 (≤0x400) |
| 0x018 | u64 | **t0** | **快照时刻** (内核 PIT ticks @100Hz, 请求写入时) |
| 0x020 | u64 | **evw** | **事件环写位置快照** (请求写入时 EV_W) |
| 0x028 | u32 | **crit** | **关键事件掩码** (bit=kind-1; 内核固定 0x1C = EV_ANOMALY/EXIT/WINDOW) |
| 0x030 | — | payload | ≤1KB |
| 0x800 | — | ctx | fujoctx v2 结构态文本 (≤0x600) |

### 2.2 回包 (模型声明有效期)

shm 回包统一追加 `TTL=<ticks>`: 模型 (宿主服务端) 声明建议有效时长
(以推断耗时 ×2 + 4s 余量估算, PIT tick @100Hz)。COM2 行协议降级路径无快照, 不带 TTL。

```
FJAI:RSP <seq> INTENT=1 TAG=qwen2.5:7b TTL=1320
FJAI:RSP <seq> ANOM=0 CONF=10 TAG=qwen2.5:7b TTL=1320
```

### 2.3 内核判定 (wait_rsp, 每步可测)

请求写入时记录 `SNAP{ t0, evw, crit, valid }`; 收到匹配 seq 的回包后:

1. `crit_n = ev_delta_critical(evw, crit)` — t0 以来新到关键事件数;
   `crit_n > 0` → **丢弃** (reason=1, 返回 None → 规则兜底)。
2. `el = ticks_now - t0`; `el > TTL` → **丢弃** (reason=2, 规则兜底)。
3. 通过 → 接受建议 (reason=0)。快照一次消耗。

规则兜底是最终裁判; 丢弃后**不再二次询问模型** (classify 直接走 rules, 不做 COM2 重试)。

### 2.4 确定性探针 (0x8309)

`fujo_r3_probe(mode, out)` — 消除"是否真的会丢弃"的时序疑虑:

| mode | 行为 | 断言 |
|---|---|---|
| 0 | 正常请求 | engine=1 reason=0 |
| 1 | 快照后注入 EV_ANOMALY | engine=2 reason=1 crit≥1 |
| 2 | t0 回拨 1e6 ticks (强制过期) | engine=2 reason=2 |

`out = [engine(1=接受 2=丢弃→规则), reason(0/1/2), crit_n, elapsed]`。
用户态封装: `sdk/linux/m118_r3.c` (T1–T5, 含主职责哨兵回归), 期望 `M118 RESULT: PASS`。

### 2.5 为什么正确 (边界讨论)

- TCG 单核下 syscall 内无法并发注入事件, 检查在**真并发 (SMP/多任务) 到来前**
  恒真; 其价值是把语义**公理化** —— 任何使检查变真的时序 (SMP、长推断期间
  的内核事件源) 都被同一代码路径拦截, 无需改动调用方。
- TTL 以宿主侧相对耗时估算, 内核侧按自身 tick 纪元比较, 两纪元仅要求同频
  (PIT@100Hz), 不要求同原点。
- 6s 链路超时仍是最后防线 (LINK_TIMEOUT_TICKS)。

## 3. 新增接口一览

| syscall | 签名 | 用途 |
|---|---|---|
| 0x8309 | `fujo_r3_probe(mode, out, cap, _)` | R3 探针 (确定性测试) |
| 0x830A | `fujo_inv_run(out, cap)` | R1 公理化自检 (离线) |

## 4. 验收 (W8)

- `python tools/verify_ai.py --demo m118_r3 --needle "M118 RESULT: PASS" --model qwen2.5:7b` (在线, 7B)
- `python tools/fujoregress.py` → 10/10 (新增 r1-invariants 用例 = 离线跑 A1–A4)
- BSS 尾 < 0x2C0000 (镜像尾 0x2A1D50; load_end/pad 已随 M116 提至 0x2C0000/0x1C0000)

## 5. 已知简化 (ponytail)

- 关键事件掩码固定 0x1C (异常/退出/窗口): 未按职责区分, 需要时提为帧字段。
- TTL 由服务端启发式给出: 无模型自声明; 蒸馏后 (W10) 可由模型推理声明置信度取代。
- A4 的"降级"当前仅计数 + 引擎字段观察: 未做"N 次失败后更保守"的策略, 属 W9/W10。
