# 37 — agent 会话 (M88: 会话/检查点/恢复)

状态: ✅ 完成。验收: QEMU 串口 `M88 RESULT: PASS`, demo `sdk/linux/m88_sess.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7E01 | sess_create(id) | 打开会话 (忙 -EBUSY) |
| 0x7E02 | sess_save(id, ptr, len) | 检查点 (≤128B) |
| 0x7E03 | sess_load(id, ptr) | 恢复 → len; gen++ |
| 0x7E04 | sess_info(ptr) | (active, max_ck, max_gen, max_tok) |
| 0x7E05 | sess_tick(id, tokens) | 会话 token 记账 |

## 2. 模型

```
会话槽: ACTIVE / GEN (恢复代数) / TOKENS (累计) / CK_DATA[128]
生命周期: create → tick* → save (检查点) → [中断/崩溃] → load (恢复,
          gen++) → 继续上次上下文 → save 新检查点 ...
```

## 3. 实测 (m88_sess.elf)

```
sess : create #0
sess : load #0 gen=1   (恢复 A: 0xAA...)
sess : load #0 gen=2   (恢复 B: 0xBB...)
m88: active=00000001 ck=00000080 gen=00000002 tok=00000096
m88: M88 RESULT: PASS
```

- 检查点 A/B 往返一致; gen 递增; tokens=150 (100+50) 记账。

## 4. 集成

- 检查点载荷未来可挂: 会话上下文摘要 (M89/90 fujoctx 链)、
  模型卡计费 (M87)、m86 权重映射区 (资源绑定)。
