# 24 — 调试器 v0 (M75: 单步/断点)

状态: ✅ 完成。验收: QEMU 串口 `M75 RESULT: PASS`, demo `sdk/linux/m75_dbg.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7601 | dbg_step(on) | 单步状态 (用户态 TF 生效) |
| 0x7602 | dbg_bp0(addr) | 软件断点 (int3 替换) |
| 0x7603 | dbg_info(ptr) | u64×4: (count, last_rip, steps, bps) |
| 0x7604 | dbg_clear() | 清断点/状态 |

## 2. 机制

### 单步 (TF)
```
用户态: pushfq / orq $0x100, (%rsp) / popfq   (置 TF)
→ 下条指令完成 → #DB (vec 1, fujo_dbg_stub)
   → fujo_dbg_exc: count++ / last_rip 记录 / steps++
     帧 RFLAGS &= ~0x100  (清 TF, 不级联) → iretq 续跑
```

### 断点 (int3 软件断点, #BP vec 3)
```
dbg_bp0(addr): BP_ORIG = *(u8*)addr; *(u8*)addr = 0xCC
→ 执行到 addr → #BP (fujo_bp_stub)
   → fujo_dbg_bp_exc: 恢复原字节; RIP-1 (重执原指令); bps++
     iretq → 原指令执行 → 函数体正常返回
```

## 3. 关键坑 (实测)

- **用户态 `int3` 是 INT 指令**: 经中断门需 **DPL ≥ CPL**;
  attr=0x8E (DPL=0) 时用户态 int3 → **#GP** (m75 第一次实测
  `vec=13 cs=0x23 rip=int3地址`); 修正 attr = **0xEE** (DPL=3)。
- DR 执行断点 (DR0+DR7 RW=00): QEMU TCG 下设置读回正常
  (DR7 回读含 GD=0x403) 但执行不触发; 软件断点 (int3) 是 TCG
  通用面 (gdb stub 同路)。
- 断点目标函数须有**副作用或 noinline+volatile 路径** (clang
  -O2 常量折叠消除 call, 断点"不命中"其实是函数无调用)。

## 4. 实测 (m75_dbg.elf)

```
dbg  : #BP hit @0x40002c        (裸 int3)
dbg  : int3 bp @0x4005f0        (dummy 入口)
dbg  : #BP hit @0x4005f0        (命中, 恢复重执)
dbg  : #DB (step) rip=0x400170 / 0x40017a / 0x400188
m75: bp_count=00000002 total=00000005 steps=00000003 bps=00000002
m75: M75 RESULT: PASS
```
