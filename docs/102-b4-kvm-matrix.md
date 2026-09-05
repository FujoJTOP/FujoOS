# 102 · B4 — KVM 全量矩阵收尾 (三列矩阵实表 + m133 断言语义升级)

> 里程碑: B4 (docs/98 测试收尾) · 上游: B2/B3 · W31 (KVM 列)
> 一句话: **KVM 矩阵补满非 AI 核心 13 用例 (全部 PASS), 三列平台矩阵实表落定;
> KVM 暴露第 2 个断言语义升级: m133 "QEMU→ICR 低写" 是 W20 原型假设, 实为
> is_qemu(设备) ≠ LAPIC 语义(hypervisor) —— 断言升级为三值模型 (VBE+TCG→低写,
> 其余→Intel 高写), KVM 下 PASS。**

## 1. KVM 矩阵结果 (WSL2 /dev/kvm, 每 demo 55s)

| 用例 | TCG | KVM | 备注 |
|---|---|---|---|
| m116/m119/m120/m121/m122/m123/m127/m130/m132 | ✅ | **✅ 全 PASS** | 域/公理/蒸馏/aspace/dev/vblk/exec/审计/dirs |
| m133 | ✅ | ✅ (断言升级后) | 见 §2 |
| m134/m135/m136/m137 | ✅ | **✅ 全 PASS** | AHCI/FJFS(SATA)/高内存(-m 3072)/PCI 多功能 |
| m141–m149, m150 | ✅ | ✅ (W31/B2 已跑) | AI 波 + TCP 探针 |
| m126 | ✅ | ⏭ 脚本局限 | multi.initrd + shell 注入 (KVM 无 sendkey 简化; 可经 monitor telnet 扩展) |
| m129 | ✅ | ❌ 嵌套 KVM LAPIC | 已知限制 (docs/94) |

**三列矩阵实表**: TCG 40/40 · WHPX 36/37 (m129 架构) · **KVM 37/38** (m126 脚本局限
+ m129 限制; 无真实回归失败)。

## 2. m133 断言语义升级 (KVM 矩阵第 2 个真实发现)

- **旧断言**: `aligned = (is_qemu==1 && icr_mode==0) || (is_qemu==0 && icr_mode==1)`
  —— W20 原型假设 "QEMU 设备 → ICR 低写语义";
- **KVM 实证**: is_qemu=1 (VBE 0xB0C5 仍在) 但 icr_mode=1 (LAPIC=Intel 语义) →
  FAIL; **platform 检测的"设备"与"LAPIC 语义"是两个轴**;
- **升级**: 断言三值模型并接入 0x6401 (hv brand): `(is_qemu==0 && icr==1) ||
  (is_qemu==1 && accel==0 && icr==0) || (is_qemu==1 && accel!=0 && icr==1)`;
- **实测**: KVM `is_qemu=1 vbe=0xb0c5 icr=1 accel=1 → M133 PASS`;
- docs/74 #1 更新 (ICR 判据版本: 双条件 VBE+TCG; m133 断言同步)。

## 3. 状态

- **B4 完成**; 脚本 `tools/kvm-matrix.sh` (无 host 依赖 14) + `kvm-matrix2.sh`;
- 后续 (可选): m126/m128/m140/m131 类 keys 用例经 KVM monitor telnet 注入扩展;
  网络 4 用例 host 服务在 WSL 侧补 (m124/125/139/140) —— 均降为 B4 延伸, 非阻塞。
