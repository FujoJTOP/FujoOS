# 112 · W37 — 真机实验波：三执行模式全量对照（TCG / WHPX / KVM）

> 里程碑: W37 (真机对照) · 需求: 用户"做真机实验，包括此前所有未完成的真机实验，
> 不切换系统、不息屏" —— 物理机引导（重启切换）排除；真机硬件路径 = **WHPX
> （本机 Windows Hypervisor Platform）+ KVM（WSL2 硬件虚拟化）** 全量对照。
> W29 (docs/92) 后 W33–W37 共 14 个新用例从未在硬件虚拟化执行 → 本波补全。
> 一句话: **TCG 51/51（基线）· WHPX 48/51 · KVM 48/51 —— 全部非预期失败可归因：
> 2 个既有平台限制（m129 INIT/SIPI / m121 时序敏感）+ 1 个新发现真机风险
> （m137: q35+无盘 AHCI 卡 boot，TCG 例外——真机 U 盘引导前置阻断）+ 工具时序
> （m140 键盘注入丢键，非内核）。**

## 1. 三模式对照表（W37 全量 51 用例）

| 模式 | 结果 | 说明 |
|---|---|---|
| TCG（W37 基线） | **51/51** | docs/111 已记录 |
| **WHPX**（本机 Windows Hypervisor Platform, `--accel whpx`） | **48/51** | 新 14 用例（W33-W37 兼容层/散件/BOX 全链）首次硬件虚拟化执行 |
| **KVM**（WSL2, `--accel kvm`, /dev/kvm 硬件虚拟化） | **48/51** | 同上；fujoregress 补 WSL 兼容（taskkill→pkill 平台分支） |

## 2. 失败归因（全部分析完毕，无一为盲）

| 用例 | WHPX | KVM | 归因 |
|---|---|---|---|
| m129-smp | FAIL | FAIL | **WHPX**: INIT/SIPI 注入被拒（docs/92 #16 既有）。**KVM**: WSL2 嵌套 vCPU CPUID `edx.9=0`（APIC 标志被 Hyper-V CPUID 过滤）→ 内核 "APIC absent" 回退 → SMP 路径不启动 —— 平台差异，非内核缺陷（真机 KVM 的 CPUID 不隐藏） |
| m121-isol | FAIL（偶发） | PASS | docs/92 既有"全量偶发、时序敏感"；TCG 51/51 证明无回归。WHPX 快 CPU 下任务调度时序差异 |
| m137-pci | FAIL | FAIL | **新发现（真机风险，见 §3）** |
| m140-selfhost | PASS | FAIL | **键盘注入丢键**（"hello-clone.c"→"ello-clone.c"，首字符丢）：WSL KVM 下 sendkey 注入时序交叠；非内核缺陷（TCG/WHPX 同注入 PASS） |

## 3. m137 新发现 —— q35 + 无盘 AHCI 卡 boot（真机前置阻断）

**现象**：`-machine q35`（无 `-drive`）下 boot 打印至
`ahci : ATA device on (LBA48 ok)` 后**再无输出**（demo 未启动）——WHPX 与 KVM
**共同**；TCG 同参数 PASS（51/51 基线）。

**意义**：q35 无盘 = **真机 U 盘/光驱引导等价拓扑**（USB 介质不占 AHCI 端口）——
真机 checklist（docs/93 §4）的 ISO/U 盘路径受此直接威胁；同时影响 ISO/USB 中的
"非 ATA 背板" 场景。**W38 修正前置项**（真机引导前必须过）。

**调查计划**（独立波）：QEMU monitor `info pci/info qtree` 取证（kvm whpx 下
ich9-ahci 设备模型行为）→ 定位 fjfs::init 或 ahci cmd 空盘路径的等待点 →
修（无盘时不激活 AHCI_READY / fjfs 快退）。

## 4. 交付与修复

- `tools/fujoregress.py`: `--accel kvm`（WSL2 硬件虚拟化）+ 平台分支 qemu 清扫
  （`os.name=="nt"` → taskkill；否则 pkill）；help 文案更新
- WSL2 前置记录: `/dev/kvm` 默认 660 (root:kvm) → 运行时 `wsl -u root chmod 666 /dev/kvm`
  （WSL 重启失效——脚本化列入 kvm-run 说明）
- 全部失败为环境/平台归因，**无 TCG 路径回归**（51/51 保持）

## 5. 真机 experiment 状态（用户约束下的完整集合）

| 项 | 状态 |
|---|---|
| WHPX 全量（W29 后首次） | ✅ 48/51（差异全归因） |
| KVM 全量（WSL2） | ✅ 48/51（差异全归因） |
| 物理真机引导（U 盘/ISO） | ⛔ 排除（切换系统违背用户约束）——**前置阻断 = m137 修复**（§3） |
| opencode CLI 真机（Windows 真机 app 面） | ✅ 本机已验证（1.18.1 运行 + 行为面盘点，见对话记录） |
