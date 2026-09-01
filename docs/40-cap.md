# 40 — 权限与审计 (M91, 能力表 + 审计日志)

状态: ✅ 完成。验收: QEMU 串口 `M91 RESULT: PASS`, demo `sdk/linux/m91_cap.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x8101 | cap_grant(idx, perm) | 授予能力 (8 槽) |
| 0x8102 | cap_check(idx, perm) | 检查; deny → -1 且自动入审计 |
| 0x8103 | aud_log(action, subject) | 显式审计条目 |
| 0x8104 | aud_read(ptr, cap) | 审计环拷贝 (32B/项) |

## 2. 实现

- 能力表: `[u64;8] perm + [bool;8] granted`;
- 审计环: 32 项 (ts, action, subject, result), 覆盖写;
- deny 联动: cap_check 失败记 `(ts, 1, idx, 1)` (action=check,
  result=deny)。

## 3. 实测 (m91_cap.elf)

```
cap  : grant #0 perm=0x1
cap  : deny #0
m91: ok=00000000 deny=ffffffff aud=00000002
m91: M91 RESULT: PASS
```

- 允许/拒绝路径; 审计 2 条 (deny 自动 + 显式 (7,9)) 字段正确。

## 4. 集成

- M87 模型卡 perm + 本能力表 = 权限栈 (模型/操作双面);
- M95 全生命周期: 命令→模型→工具→审计 的审计链收口。
