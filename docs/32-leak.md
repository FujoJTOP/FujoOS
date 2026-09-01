# 32 — 内存泄漏检测 (M83, 分配器统计)

状态: ✅ 完成。验收: QEMU 串口 `M83 RESULT: PASS`, demo `sdk/linux/m83_leak.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7A01 | leak_begin() | 快照 (kobj 计数 4 类) |
| 0x7A02 | leak_end(ptr) | 差分 (delta, allocs, frees, baseline) |
| 0x7A03 | leak_stats(ptr) | 当前计数 u64×4 |

## 2. 锚定面

- 内核对象表 (M19 kobj): 4 类 [file, pipe, shm, sig], 有快照与
  逐槽计数 (kobj::counts);
- 差分语义: delta>0 = 未释放候选 (泄漏可检); delta==0 = 平衡;
  delta<0 = 低于基线的净释放 (快照窗口重建)。

## 3. 实测 (m83_leak.elf)

```
leak : delta +4 (unreleased slots)     ← kobj_create ×4 未释放
m83: after-alloc delta=00000004
kobj : free slot=0..3
leak : delta -4 (freed below baseline) ← 全释放 (回到阶段前? 基线快照
                                         在 free 前=4 → 释放后 0)
m83: after-free delta=00000000
m83: M83 RESULT: PASS
```

## 4. 扩展

- 后续 M86+ (模型 mmap) 与 M91 (审计) 复用快照面: 资源记账 +
  泄漏窗口 (begin/end 包围长生命周期操作)。
