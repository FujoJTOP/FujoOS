# 33 — 崩溃转储 (M84, minidump 雏形)

状态: ✅ 完成。验收: QEMU 串口 `M84 RESULT: PASS`, demo `sdk/linux/m84_dump.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7B01 | dump_arm(on) | 捕获开关 |
| 0x7B02 | dump_read(ptr, cap) | 拷贝 minidump (120B) |
| 0x7B03 | dump_info(ptr) | u64×4: (count, vec, rip, cr2) |

## 2. Minidump 布局 (120B)

```
[0..8)    "FUJDUMP\0"
[8]  vec  [16] rip  [24] cr2  [32] rsp  [40] cs
[48..8+64) regs8 (r11 r10 r9 r8 rdi rsi rdx rcx)
[112]     count
```

## 3. 挂接

- fujo_exc2 用户异常分支 (M14 隔离转场前) 调 `dump::note_exc`
  (vec, regs, err 偏移) — 单任务停机/多任务隔离均覆盖;
- cr2 仅 #PF (vec 14) 读取。

## 4. 实测 (m84_dump.elf)

```
EXC user vec=6 cs=0x23 rip=0x40041a
dump : captured minidump #1 vec=6 rip=0x40041a
proc: task 1 terminated (crash isolated)
m84: count=00000001 vec=00000006 rip=00000040 n=00000078
m84: M84 RESULT: PASS
```

- fork 子任务 ud2 → #UD 捕获 → 隔离转场; 父读回 minidump
  (count=1 vec=6 n=120) ✓。

## 5. 后续

- M85 验收面; 真 minidump 解析器 (外部) 可基于该布局做
  栈回溯 (rsp→用户栈帧) 与符号化 (M71 汇编器符号表 + M72 链接表)。
