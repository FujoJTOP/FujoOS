# 36 — 模型卡 (M87: 权限/计费/审计元数据)

状态: ✅ 完成。验收: QEMU 串口 `M87 RESULT: PASS`, demo `sdk/linux/m87_mcard.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7D01 | mc_register(ptr) | 装载模型卡 (资源节元数据面) |
| 0x7D02 | mc_call(len, perm_need) | 调用: perm 检查 + 计费 + 审计 |
| 0x7D03 | mc_info(ptr) | (calls, tokens, budget, perm) |
| 0x7D04 | mc_audit(ptr, cap) | 审计环拷贝 (32B×n) |

## 2. 模型卡布局 (120B)

```
[0..24)  name  [24] version
[32]     perm_mask u64   [40] cost u32   [48] calls u32
[56]     budget u64      [64] 预留
```

## 3. 语义

- mc_call: `(perm & perm_need) == perm_need` 且 tokens 累计不超
  budget (budget≠0 且 ≠MAX) → 计费 (calls++/tokens+=) → 0;
  否则返回 -1 (deny), **两类都写审计环**;
- 审计条目: (ts, model_idx, tokens, result)。

## 4. 实测 (m87_mcard.elf)

```
mcard: registered 'qwen3-0.6b' perm=3
mcard: call tokens=100 result=0 ×3
mcard: call tokens=100 result=0xFFFFFFFFFFFFFFFF   (perm_need=8 deny)
mcard: call tokens=900 result=0xFFFFFFFFFFFFFFFF   (超预算 deny)
m87: calls=00000003 tokens=0000012c aud=00000005
m87: M87 RESULT: PASS
```

- 3 次正常调用 (300 token); 越权 + 超预算各 1 次 deny;
  审计 5 条全记录。

## 5. 集成

- 资源节 (M31 DATA) 中携带模型卡 → 装载路径 mc_register;
- M91 能力表/审计日志复用审计环格式; M95 全生命周期验收挂卡。
