# 96 · 物理机 / 云 KVM 波 — 冻结专档（未执行，待硬件/待决策）

> 状态: **2026-09 冻结**（用户决策: 暂不触碰物理机, 全部文档化）
> 上游: docs/91 (计划 W31) / docs/94 (KVM 列) / docs/93 (真机就绪包)
> 一句话: 硬件波的全部**前置条件已就绪**（ISO 构建链 + autostart + COM1 模板 +
> 平台差异 17 项审计表 + icr 判据修正 + APIC 规范使能），冻结为"只读手册";
> 解除条件 = 用户提供物理机 或 提供带 /dev/kvm 的 Linux 主机。

## 1. 为什么冻结

1. **本地物理机影响确认**: U 盘整机引导对 Windows 数据零写入（FujoOS 纯内存运行、
   demo 均无磁盘路径），但需临时关闭 SecureBoot + 切换引导顺序 —— 用户判断暂不执行;
2. **云 Linux (非嵌套 KVM)** 是零本地影响替代方案: 未提供主机资源;
3. 项目评估: 剩余价值 = m129 SMP 第二平台 + 真机 AI 对照 + 论文 Threats 第 4 列
   数据 —— **非阻塞**（TCG/WHPX/KVM 三列已交付, 论文可先投）。

## 2. 冻结时已就绪的前置（全部完成, 无需再开发）

| 前置 | 状态 | 证据 |
|---|---|---|
| 引导 ISO 构建链 | ✅ `scripts/make-boot-iso.ps1` (WSL2 grub-mkrescue) | docs/93; sdk/build/fujo-boot.iso |
| 无键盘 autostart | ✅ mbi cmdline `fujo.run=<demo>` → 直启; cmdline=权威 (GRUB module name 字段为空已适配) | docs/93; m148 用例 |
| 引导期段一致性 | ✅ #17 修复 (SS=0x10 显式装载) → GRUB/ISO 全链路 PASS | docs/95 (3ca14c2) |
| LAPIC 规范使能 | ✅ wrmsr 0x1B (IA32_APIC_BASE.EN) | docs/74 #16 关联 (639e7d8) |
| ICR 语义判据 | ✅ VBE=QEMU && hv=TCG → 低写; else Intel 高写 (KVM/真机正确) | docs/74 (639e7d8) |
| 串口捕获模板 | ✅ COM1 (DB9/USB-转串) 115200/8N1 checklist | docs/93 §4 |
| 平台审计表 | ✅ 17 项 (含 #15 WHPX 中断注入 / #16 INIT-SIPI / #17 SS) | docs/74 |

## 3. 解除后的一次性流程（已写死, 照抄即可）

```
A. U 盘物理机 (真机数据零写入, 需临时 SecureBoot Off + 引导 U 盘)
   1. pwsh scripts/make-boot-iso.ps1 -Demo m142_feedback -Out sdk/build/fujo-boot.iso
   2. Rufus dd 模式写 U 盘; BIOS 临时关闭 SecureBoot; F12 选 U 盘
   3. COM1 捕获: 115200/8N1 → tee qemu-realhw.log
   4. 验收 needle: "boot: autostart" + "M142 RESULT: PASS"
   5. m129-smp: 重建 ISO (-Demo m129_smp) 复测 AP 唤醒 (真 LAPIC + Intel 高写语义)
   6. 完成: 恢复 SecureBoot + 引导顺序; 数据表入 docs/97 (报告) + docs/74 审计表
B. 云 Linux (非嵌套 KVM, 零本地影响)
   1. 需主机: qemu-system-x86 + /dev/kvm (裸金属或 KVM-enabled VPS)
   2. git clone https://github.com/FujoJTOP/FujoOS.git
   3. 参考 tools/kvm-run.sh (已 WSL2 验证) → 同参数非嵌套 KVM 直跑
   4. m129 预期 PASS (真实 LAPIC 语义); AI 六件套预期 PASS
```

## 4. 预期结果（写入验收标准, 便于日后对账）

| 项 | TCG | WHPX | WSL2-KVM(嵌套) | 物理机/云KVM(非嵌套) | 
|---|---|---|---|---|
| m129 (SMP) | ✅ | ❌ 架构限制 | ❌ LAPIC 虚拟化不可用 | **预期 ✅ (待测)** |
| AI 波六件套 | ✅ | ✅ | ✅ | 预期 ✅ |
| m141/m144 在线 | ✅ 7b | ✅ 7b | 未架 | 需 COM2 实串口 |

## 5. 更新处

- 本档 = 冻结权威文档; 解除执行时更新: docs/74 (审计表后缀) / docs/97 (项目报告平台矩阵)。
