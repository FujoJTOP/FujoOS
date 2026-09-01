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
