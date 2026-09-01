# 48 — 签名/更新机制 (M99)

状态: ✅ 完成。验收: QEMU 串口 `M99 RESULT: PASS`, demo `sdk/linux/m99_upd.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x8801 | upd_check(ptr, cap) | 计算 FNV-1a 签名 (≤4KiB) |
| 0x8802 | upd_apply(ptr, len, expected) | 校验+写盘+读回验证 |
| 0x8803 | upd_status(ptr) | (hash, pending, upd_count) |

## 2. 机制

```
upd_apply:
  fnv1a(data) == expected?        (写前签名校验; 否则 -EINVAL)
  → FJFS write fujo-kernel.bin
  → rdtsc 忙等 ~10ms (QEMU IDE 写缓存握手; PIT 被 SFMASK 屏蔽)
  → read_file 读回 → fnv1a == expected?  (返回 0 + UPD_COUNT++)
                                      否则 -EINVAL + PENDING=1
```

签名: FNV-1a 64 (自写, 与 fujopack fnv1a 同族; 定数量化面)。

## 3. 实测 (m99_upd.elf)

```
upd  : applied update #1 (hash verified)
upd  : hash mismatch - refused
m99: ok=00000000 tamper=ffffffea cnt=00000001
m99: M99 RESULT: PASS
```

- 正常数据应用成功 (计数 1); 篡改 1 字节 → 拒绝。

## 4. FJFS/ATA 修复 (存储层)

- **write_sectors 写后等待**: 原 1000 循环仅查 BSY=0 — PIO 连续写
  时第二扇区命令在写完成前发出 → 数据零点 (磁盘检查: LBA5 独为 0
  而其它扇区正确); 修复: **50000 循环 + BSY=0 且 DRQ=0 双条件**。
- 写后立即读回 (同进程) 需握手段 (QEMU IDE 缓存), rdtsc 忙等
  (~10ms) 后读回与盘数据一致。

## 5. 后续

- M100 发布 (更新机制作为发布面: 新内核 → 签名 → upd_apply)。
