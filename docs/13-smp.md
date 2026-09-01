# 13 — 多核并行 v0 (M64: 探测 / 亲和 / 负载均衡统计)

状态: ✅ 完成。验收: QEMU `-smp 2` 串口 `M64 RESULT: PASS`,
demo `sdk/linux/m64_smp.c`。

## 1. 接口

| 编号  | 签名 | 说明 |
|-------|------|------|
| 0x6A01 | aff_set(tid, mask) | 任务亲和位图 (bit0=核0, bit1=核1; 0xFF 默认任意) |
| 0x6A02 | aff_get(tid) | 读回亲和 |
| 0x6A04 | smp_stats(ptr) | 写 u32×4: (ncpu, core0_count, core1_count, switches) |

## 2. 探测 (CPUID)

- global_asm 桥 `fujo_cpuid_leaf1` (rbx 为 LLVM 保留, 不能在 Rust
  侧内联 cpuid; 桥内 push/pop 保护)。
- 逻辑核数 = EBX[23..16] + 1; v0 截断桶到 2 核。
- 启动日志: `smp  : cpuid logical CPUs = 2 (affinity v0 armed)`。

## 3. 负载均衡 v0 (策略 + 统计)

调度侧 (sched::fujo_tick_sched 切换点) 调用 `smp::note_switch(next)`:

```
core = 亲和位图最低置位 bit (0xFF → task_id % ncpu 轮换)
CORE0 / CORE1 计数 += 1        (SWITCHES += 1, c0+c1==switches 不变量)
```

单 PIT 时钟源下该统计表达"**调度策略把任务归到哪核**"; 真并行
(每核时钟/APIC/多 TSS) 由 M65 承接, 亲和位图与记账面保持不变。

## 4. 实测

`-smp 2` 下 m64_smp.elf: fork → 父 aff=核0, 子 aff=核1 (bit1),
双方忙等 20M 轮给 PIT 轮转 → 统计:

```
sched: fork parent=0 child=1
sched: ctx-switch #1 -> task 1
...（轮转 #1..#8 -> task1/task0 交替）
m64: ncpu=00000002 c0=00000008 c1=00000008 sw=00000010
m64: M64 RESULT: PASS
```

- ncpu=2: CPUID 正确 (QEMU SMP);
- task1 (子, 亲和 bit1) 的 8 次切片全部计入 core1;
- task0 (父, 亲和 bit0) 的 8 次切片全部计入 core0;
- c0+c1 == sw 不变量成立。

## 5. 验证

```
qemu-system-x86_64 -m 256M -smp 2 -kernel kernel/fujo-kernel.bin \
    -initrd sdk/linux/m64_smp.elf -serial file:log \
    -monitor telnet:127.0.0.1:4568,server,nowait -display none -no-reboot
# monitor: sendkey o s spc r u n spc h e r m e s ret
```

## 6. 下一步

- M65: 每核 TSS / 中断注入优化 (SMP 启动: LAPIC + 每核 RSP0)。
- M70 性能验收: 以本里程碑的核负载统计给出并行评估面。
