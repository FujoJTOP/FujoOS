# 22 — 迷你编辑器 (M73, vi 子集 v0)

状态: ✅ 完成。验收: QEMU 串口 `M73 RESULT: PASS`, demo `sdk/linux/m73_edit.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7401 | ed_init() | 清空缓冲/游标 |
| 0x7402 | ed_text(ptr, n) | 全文载入 (游标置末行尾) |
| 0x7403 | ed_key(c) | 单键处理 |
| 0x7404 | ed_dump(ptr, cap) | 全文拷贝 (返回长度) |
| 0x7405 | ed_info(ptr) | u64×4: (row, col, lines, len) |

## 2. 键模型

| 键 | 语义 |
|----|------|
| i | 插入模式 (后续字符插入, Esc 退出) |
| j / k | 游标下/上行 (行尾钳制) |
| x | 删除游标字符 (含 '\n' → 行合并, 游标留合并行尾) |
| ^ / $ | 行首 / 行尾 |

文本缓冲 2KiB BSS, '\n' 分隔, 游标 (row, col)。

## 3. 实测 (m73_edit.elf)

```
输入: "abcd\nefg" → 光标 (1,3)
k/j ↔ → (0,3)/(1,3); k + x → 删 'd' → "abc\nefg"
$ → col=3; ^ → col=0
i 'X' Esc → "Xabc\nefg"
dump: Xabc\nefg
m73: M73 RESULT: PASS
```

## 4. 集成面

- 编辑器内核态就绪 (selftest 打印 "ed: vi-subset ready");
- 后续: 编辑器承接 bash 式 shell (M90+ 工具链), 或作为
  fujokit 文本组件后端 (kt_textbox 已在 SDK 侧, 编辑器为内核版本)。
