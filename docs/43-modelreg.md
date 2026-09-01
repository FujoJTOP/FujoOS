# 43 — AI 服务 (M94: 模型注册表 + fupm)

状态: ✅ 完成。验收: QEMU 串口 `M94 RESULT: PASS`, demo `sdk/linux/m94_fupm.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x8401 | fupm_install(ptr, size, name) | 安装模型 (≤8KiB/槽) |
| 0x8402 | reg_list(ptr) | 表转储 (size/active/calls/data_ptr ×4) |
| 0x8403 | reg_active(idx) | 单槽激活 |
| 0x8404 | fupm_remove(idx) | 移除 |

## 2. 实现

- 注册表 4 槽 (name[16], size, active, calls);
- 数据区 MODEL_DATA=0xF38000 (4×8KiB, 恒等映射, 内核侧 U=0)。

## 3. 实测 (m94_fupm.elf)

```
fupm : installed #0 size=4096   (qwen3-0.6b)
fupm : installed #1 size=2048   (tiny-lm)
fupm : active #1
fupm : removed #1
m94: s0=00001000 s1=00000800 active=00000001
m94: M94 RESULT: PASS
```

- 双模型安装/激活/移除; 条目字段正确。
- 坑: 用户态解引用 reg_list 返回的**内核区指针** → #PF (U=0);
  校验只做区段范围断言 (数据内容正确性由 M86 权重按需页路径覆盖)。

## 4. 集成

- M86 权重按需页 / M87 模型卡 / M93 执行器锚定注册表槽
  (active 模型 = 执行器输入面); M95 验收全链。
