# 42 — 推理执行器插槽 (M93)

状态: ✅ 完成。验收: QEMU 串口 `M93 RESULT: PASS`, demo `sdk/linux/m93_infer.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x8301 | infer_run(ptr, len, out, cap) | 执行推理请求 → 响应长度 |
| 0x8302 | infer_slot(ptr) | (mode, calls, tokens, last_ms) |
| 0x8303 | infer_set(mode) | 0=host-link 1=local-kernel |

## 2. 执行器

```
mode=local: 确定性响应 "fujo-infer-local: recv=<len> tokens intent=<X>"
             (X = 规则意图 RUN/OPEN/QUERY/UNKNOWN; 微小计量 = 定数量化
              内核评估面的占位语义)
mode=host:  中继面 (COM2 模型服务); 同签名输出 (v0 占位响应)
执行记账: calls / tokens / last_ms (tick 级)
```

## 3. 实测 (m93_infer.elf)

```
infer: mode=local-kernel
infer: run len=15 intent=2
infer: mode=host-link
infer: run len=6 intent=2
m93: n1=0000002e calls=00000002 tokens=00000015
m93: M93 RESULT: PASS
```

- local 响应 46B ("recv=15 tokens intent=QUERY");
- 双模式可切换, 记账 (2 calls, 21 tokens) 正确。

## 4. 链位置

```
M92 路由 → M93 执行器 (host/local) → 工具调用 (kit/syscalls) →
M91 审计 → M88 会话检查点 → M95 验收
```
