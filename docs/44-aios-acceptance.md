# 44 — AI OS 验收 (M95: agent 全生命周期)

状态: ✅ 完成。验收: QEMU 串口 `M95 RESULT: PASS`, demo `sdk/linux/m95_life.c`。

## 1. 生命周期链

```
命令  "open the file"
  → 意图路由 (M92: route_classify → OPEN)
  → 模型 (M94 fupm 安装 tiny-lm 权重; M87 模型卡注册 perm=1/budget=500;
          M93 infer 本地执行 → 45B 响应; 计费 mc_call)
  → 工具 (M19 kobj 创建/释放 → 无泄漏; M88 会话检查点 128B 往返 gen=1)
  → 审计 (M91 aud_log ×3 → 3 条)
```

## 2. 实测 (m95_life.elf)

```
infer: run len=14 intent=3
mcard: call tokens=45 result=0
leak : balanced (no leak)
sess : load #0 gen=1
m95: intent=00000003 resp_n=0000002d leak=00000000 aud=00000003
m95: M95 RESULT: PASS
```

## 3. 验收表 (Wave 6 汇总)

| Milestone | 交付 | 实测 |
|-----------|------|------|
| M86 | 权重 mmap 按需页 | pfa=1 pages=1 |
| M87 | 模型卡 | 3 calls/300 tokens/2 deny/aud 5 |
| M88 | 会话检查点 | gen=2 ck=128 |
| M89 | fujoctx 摘要注入 | syscalls=2 ticks=1748 |
| M90 | 上下文压缩 | 4096→790 (19.3%) |
| M91 | 能力表+审计 | deny auto-audit=2 |
| M92 | 意图路由 | 跨引擎 OPEN=3 一致 |
| M93 | 推理执行器 | local 45B + metering |
| M94 | 模型注册表+fupm | install/active/remove |
| M95 | 全生命周期 | PASS |

**AI OS 独有层 (文档 07 四件套: mmap 权重/模型卡/fujoctx/审计) 闭环。**

---

# 44-Ext · Five-AI 回归 (M115 · Wave 7)

状态: ✅ 完成。验收: QEMU 串口 `M115 RESULT: PASS`, demo `sdk/linux/m115_five.c`,
模型 qwen2.5:7b (宿主 Ollama; 缺失时规则降级)。

## 五职能对照表 (基线=规则语义 / 模型=Qwen 2.5 7b)

| 职能 | 接口 | 基线 | 模型实测 | 判定 |
|------|------|------|----------|------|
| A 异常哨兵 | 0x8304 | 10/0 (规则) | hits=10 fp=0 (100 样本) | PASS |
| B 计划-执行 | 0x8305 | A2 1;A5 1 规则 | ok=2 fail=0 verify=1 | PASS |
| C I/O 预测 | 0x8306 | LRU=0/30 | model=10/30 | PASS (≥基线) |
| D 自然语言配置 | 0x8307 + 0x6601 | POL 规则 | applied=3, 执行面拒绝 -1 | PASS |
| E 环境侦察 | 0x8308 | desktop/2 规则 | SCENE=desktop PROFILE=2, cfg(6)=2 | PASS |
| 链路 (M95 面) | 0x5101 | — | intent=1 (RUN) | PASS |

**结论: 五条 AI 系统职能闭环; 模型输出皆为"提示", 规则为兜底 — 两者对照保持
"基线保底 ≥8/10、模型追优" 的双轨验收。**

详见 [53-m112.md](53-m112.md) / [54-m113.md](54-m113.md) / [55-m114.md](55-m114.md)。
