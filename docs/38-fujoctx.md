# 38 — fujoctx 升级 (M89: 窗口焦点/文件变更/syscall 摘要注入)

状态: ✅ 完成。验收: QEMU 串口 `M89 RESULT: PASS`, demo `sdk/linux/m89_ctx.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7F01 | ctx_snap(ptr, cap) | 生成上下文摘要行 → 长度 |

## 2. 摘要格式

```
fujoctx v1 win_focus=N files=N syscalls=N ticks=N\n
```

| 字段 | 来源 |
|------|------|
| win_focus | 焦点窗口 id (v0 占位 0; 后续 wmsg 焦点面) |
| files | VFS 文件写计数 (vfs::fs_writes, M89 新增) |
| syscalls | M68 perf CTR[1] (常开计数) |
| ticks | PIT 100Hz 计数 |

注入路径: 0x5102 fujo_ai_fetch 上下文旅 (上下文摘要 → 模型调用)。

## 3. 实测 (m89_ctx.elf)

```
m89: ctx1=fujoctx v1 win_focus=0 files=0 syscalls=2 ticks=1748
m89: M89 RESULT: PASS
```

- 前缀/字段存在性 + syscalls/ticks 非零 + 长度检查通过。

## 4. 后续

- M90 上下文压缩 (委托宿主大模型) 以本摘要为输入面;
- M91 审计复用 files/syscalls 面做行为摘要。
