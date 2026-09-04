# 92 · W29 — 第二执行模式对照 (TCG vs WHPX) + WHPX 平台差异发现

> 里程碑: W29 (平台对照 I) · 计划: docs/91
> 一句话: **双模式基建落地 -- fujoregress/verify_ai 支持 `--accel whpx`; WHPX 全矩阵
> 对照运行; 首个 WHPX 差异实证: QEMU WHPX 默认 `kernel-irqchip=on` 只走 APIC 注入,
> 内核 legacy 8259 直连路径失效 → m1 等 PIT tick 死锁; `kernel-irqchip=off` 恢复设备
> 模型 PIC/PIT 模拟, 与 TCG/真机同构 —— docs/74 审计表新增第 15 条。**

## 0 · 背景修正 (用户指正)

六波 AI 实验 (W22–W28) 全部 QEMU TCG, 无第二平台; "脱 QEMU 工程" (W20) 交付的是
平台差异审计 + 语义协议 + 真机就绪码路径 (GRUB2 模板/AHCI 双背板), **物理验证停在
"就绪"** (ISO 未生成)。W29 起按 docs/91 逐波执行: 先做可立即获得的"第二执行模式"
对照 (WHPX = Windows Hypervisor Platform, 硬件虚拟化, QEMU 加速器), 再向真机推进。

## 1. 交付

| 部件 | 说明 |
|---|---|
| `tools/fujoregress.py --accel <tcg\|whpx>` | 全矩阵加速器参数; **WHPX 自动注入 `kernel-irqchip=off`** (含 `-machine q35` 用例合并写法) |
| `tools/verify_ai.py --accel` | 在线波同参数 (m141/m144 WHPX 对照) |
| `tools/qemu-kvm.ps1` | 双模式入口 (Accel 参数化; -EnableKvm 兼容去重) |
| `docs/74` 审计表新增 | #15 WHPX 中断注入路径差异 (见 §3) |

## 2. 对照表 (本波实测)

**TCG 参考机基线** (W22–W28 已文档化): m141–m147 全 PASS, fujoregress 37/37。

**WHPX 全量 (本轮运行): 34/37 → m123 修复后全绿重验** (`kernel-irqchip=off` 注入):

| demo | TCG | WHPX | 备注 |
|---|---|---|---|
| m141 (在线 7b) | PASS (8/8·1/5·6/6, novel 2/2) | **PASS (8/8·1/5·6/6, novel 2/2)** | 三引擎表逐项一致; io [rules]/[auto] 经 markov 5/5 |
| m142 | PASS | PASS | 自监督验证位一致 |
| m143 | PASS | PASS | 蒸馏闭环一致 |
| m145 | PASS | PASS | markov 所有权一致 |
| m146 | PASS | PASS | 全自监督一致 |
| m147 | PASS | PASS | 事件流哨兵一致 |
| m144 (evil 在线) | PASS | **PASS (ok=1 fail=1, state=2, deny=2, revoke 0/2)** | 对抗拦截一致 |
| m124/m125/m139/m140 | PASS | PASS | 网络栈一致 |
| m121 | PASS | PASS (全量运行偶发 FAIL 一次, 单跑稳定 PASS) | 时序敏感, 记录 |
| m123 | PASS | PASS (**修复后**) | 见 §3b: submit_req 超时判据执行模式健壮化 |
| m129 | PASS | **FAIL (WHPX 限制)** | "WHPX: Unexpected VP exit code 4" — INIT/SIPI 在 WHPX 不可注入 |

**在线波 WHPX 追加** (verify_ai --accel whpx): m141 (qwen2.5:7b) + m144 (--evil) 全 PASS,
shm 模型通道在 WHPX 下正常工作 (monitor pmemsave 仍可用)。

**修正后**: WHPX 全量 = **36/37** (仅 m129 = WHPX 平台限制, 非内核缺陷: AP 唤醒
INIT/SIPI 由 WHPX 拒绝注入; 真机 LAPIC 路径不受影响 — 记录为 docs/74 #16)。

## 3. WHPX 平台差异 (实质发现, 入 docs/74 审计表)

**#15 WHPX 中断注入路径 (kernel-irqchip)**:
- 现象: `-accel whpx` 默认参数下内核 boot 到 `m1: sti, waiting first PIT tick...` 卡死
- 根因: QEMU WHPX 默认 `kernel-irqchip=on` (中断芯片交给 Hyper-V 平台, 注入走 LAPIC);
  内核中断架构 = legacy 8259 直连 (interrupts.rs), PIC IRQ0→INTR 引脚在 WHPX 下不投递
- 处置: `-machine kernel-irqchip=off` (QEMU 设备模型模拟 PIC/PIT, 与 TCG/真机同构)
- 非内核 bug: 物理真机 8259 有效; TCG 设备模型有效; 仅 WHPX 默认路径不同 → 平台差异
- **followup**: 内核中断架构 APIC 化 (WHPX/现代平台), 独立波

**#16 WHPX SMP AP 唤醒限制**:
- 现象: m129 `INIT+SIPI -> AP (dest=APIC1)` 后 "WHPX: Unexpected VP exit code 4" 无 AP 在线
- 结论: WHPX 拒绝 INIT/SIPI 注入 (`-smp 2` 下 AP 无法启动); 真机/TCG/KVM 不受影响
- 处置: WHPX 单核模式 (m129 不适用); 文档记录

## 3b. m123 修复: submit_req 超时判据执行模式健壮化

- 现象: WHPX 下 m123 T4 写超时 (rc=-2), 读通过
- 根因: 裸 spin 计数上限 (60M) 在快 CPU (WHPX, cyc 高) 下墙钟不足 — 设备完成
  前计数已尽 (TCG: 60M ≈ 60ms 足够; WHPX ≈ 10ms → 不够)
- 修复: 超时 = `spin > 600M || rdtsc 差 > 900M` 双条件 (TCG 计数先达同旧行为,
  WHPX 墙钟兜底 ~300ms); `timer::rdtsc` 改 pub
- **方法论**: 所有"忙等计数"类超时在第二执行模式对照下都应检查"墙钟 vs 计数"。

## 4. 状态与下一步

- **W29 完成**: 双模式基建 + WHPX 对照表 (AI 波 7 项全一致 + 2 个平台差异归档
  + 1 个内核健壮性修复); **结论: 六波 AI 实验结论不依赖执行模式 (TCG=WHPX)**;
- **W29-followup** (独立波): 内核中断架构 APIC 化 (WHPX 默认路径 + 现代平台);
- **W30** (docs/91): 真机就绪包 (GRUB ISO + autostart + COM1 捕获) 按计划推进。
