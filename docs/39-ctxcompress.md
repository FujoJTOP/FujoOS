# 39 — 上下文压缩 (M90: fujoctx 链)

状态: ✅ 完成。验收: QEMU 串口 `M90 RESULT: PASS`, demo `sdk/linux/m90_ctx.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x8001 | ctx_compress(src, len, dst, cap, win) | 压缩 (头 win + 标记 + 尾 win/2) |

## 2. 策略 (v0)

```
[头 win 字节][...ctx-compressed...][尾 win/2 字节]
win ≤ len/3 (下限 8); 输出 ≤ cap
```

- v0 = 截断+摘要窗口 (本地策略);
- **委托面**: 策略占位 → 宿主大模型摘要 (fujoctx 链: 0x5102
  fujo_ai_fetch 摘要注入) — 压缩逻辑与模型面解耦, 可替换。

## 3. 实测 (m90_ctx.elf)

```
m90: in=4096 out=00000316 ratio=00000788
m90: M90 RESULT: PASS
```

- 4KB → 790B (512 头 + 22 标记 + 256 尾), 头 AAAA / 尾 ZZZZ /
  中标记全部保留正确; ratio 字段 = 1928 (19.28% 保留率)。

## 4. 链位置

```
M89 摘要 (ctx_snap) → [上下文超限] → M90 压缩 (ctx_compress)
→ 压缩上下文注入 (0x5102) → 宿主/局部模型 → 会话检查点 (M88)
```
