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

| 模式 | 修复前 | 修复后（m137 AHCI 签名/墙钟） | 说明 |
|---|---|---|---|
| TCG（W37 基线） | **51/51** | **51/51** ✅ | docs/111 已记录；m137 修复无 TCG 回归 |
| **WHPX**（本机 Windows Hypervisor Platform, `--accel whpx`） | **48/51** | **49/51** | +m137 修复；剩 m129（INIT/SIPI 平台限制）+ m121（既有时序敏感） |
| **KVM**（WSL2, `--accel kvm`, /dev/kvm 硬件虚拟化） | **48/51** | **49/51** | +m137 修复；剩 m129（WSL2 嵌套 CPUID 隐藏 APIC）+ m140（键盘注入丢键，工具时序） |

## 2. 失败归因（全部分析完毕，无一为盲）

| 用例 | WHPX | KVM | 归因 |
|---|---|---|---|
| m129-smp | FAIL | FAIL | **WHPX**: INIT/SIPI 注入被拒（docs/92 #16 既有）。**KVM**: WSL2 嵌套 vCPU CPUID `edx.9=0`（APIC 标志被 Hyper-V CPUID 过滤）→ 内核 "APIC absent" 回退 → SMP 路径不启动 —— 平台差异，非内核缺陷（真机 KVM 的 CPUID 不隐藏） |
| m121-isol | FAIL（偶发） | PASS | docs/92 既有"全量偶发、时序敏感"；TCG 51/51 证明无回归。WHPX 快 CPU 下任务调度时序差异 |
| m137-pci | FAIL | FAIL | **新发现（真机风险，见 §3）** |
| m140-selfhost | PASS | FAIL | **键盘注入丢键**（"hello-clone.c"→"ello-clone.c"，首字符丢）：WSL KVM 下 sendkey 注入时序交叠；非内核缺陷（TCG/WHPX 同注入 PASS） |

## 3. m137 新发现 —— q35 + 无盘 AHCI 卡 boot（真机前置阻断）✅ 已修

**现象**：`-machine q35`（无 `-drive`）下 boot 打印至
`ahci : ATA device on (LBA48 ok)` 后**再无输出**（demo 未启动）——WHPX 与 KVM
**共同**；TCG 同参数 PASS（51/51 基线）。

**根因（已定位，W37 修复）**：
1. **签名误判**：QEMU q35 无盘端口默认 `P_SIG=0xffff_0101`（高 16 位全 1 =
   设备不存在），原有判据 `sig & 0xFF == 0x01`（只看低字节）→ 误判 ATA →
   `AHCI_READY=true` → fjfs 读卷 → cmd() 对空盘 busy 重试；TCG 的 mmio 模拟快
   （几十 ms 失败），**WHPX/KVM 每次 mmio_rd 是 VP 陷出（µs 级 × 8M 次 = 30s+）**
   → boot 拖延过 TTL。真机（无盘端口 sig=0xffff_ffff）同误判风险。
2. **纯 spin 计数上限**（m123 同类，docs/92 §3b）：cmd busy 等待只有 spin 计数，
   快 CPU 上墙钟无界。

**修复（`kernel/src/ahci.rs`）**：
- 签名**精确匹配** `sig == 0x0000_0101`（ATA LBA48；无盘/无设备端口拒绝）
- cmd busy 等待双上限：`spins < 2M && rdtsc 差 < 250ms`（真盘 SATA 冷启动 <100ms）

**验证（三模式单跑 + 全量复验）**：m137 TCG ✅ / WHPX ✅ / KVM ✅；盘路径
m134/m135 TCG ✅；全量三模式复验见 §1（本波后）。

**意义**：q35 无盘 = 真机 U 盘/ISO 引导等价拓扑 —— 修复后**真机引导前置阻断解除**
（docs/93 §4 checklist 可执行）。

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
