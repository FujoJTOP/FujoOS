# 94 · W31 — 第二硬件列: WSL2 KVM 对照 (m141-m147 三执行模式一致)

> 里程碑: W31 (docs/91) · 上游: W29 (WHPX) + W30 (autostart/ISO)
> 一句话: **WSL2 嵌套虚拟化 `/dev/kvm` 可用; KVM (硬件虚拟化执行, 无 TCG 解释)
> 跑 AI 波六件套 (m141 离线 + m142/m143/m145/m146/m147) 全 PASS + autostart 直启;
> TCG / WHPX / KVM 三执行模式行为一致 —— AI 波结论不依赖单个执行模式。
> #17 (GRUB/ISO 交付中断帧 #GP) 仍未解, 列为真机引导前置卡点 (W32 后独立波)。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| WSL2 环境 | `/dev/kvm` 存在 (嵌套虚拟化开启) + `qemu-system-x86` 安装 |
| `tools/kvm-run.sh` | 参数化 KVM 验收 (`-enable-kvm` + `-append fujo.run=<demo>` autostart) |
| 对照结论 | 三执行模式矩阵 (见 §2) |

## 2. 三执行模式对照表 (最重的一列)

| demo (autostart) | TCG (参考机) | WHPX | **KVM (硬件虚拟化)** |
|---|---|---|---|
| m141 (离线) | PASS | PASS | **PASS** |
| m142 | PASS | PASS | **PASS** |
| m143 | PASS | PASS | **PASS** |
| m145 | PASS | PASS | **PASS** |
| m146 | PASS | PASS | **PASS** |
| m147 | PASS | PASS | **PASS** |
| m141/m144 (在线 7b) | PASS | PASS | (WSL 模型链路未架; WHPX 已覆盖在线) |

**意义**: KVM = 硬件虚拟化 (`/dev/kvm`), 指令不经 TCG 解释 —— 与真机指令执行
同族; AI 波在"解释执行"与"硬件执行"两种极端下结果一致: **蒸馏/自监督/所有权/
事件哨兵/域拦截 全部不依赖执行模式**。这对论文的价值 = 把"QEMU TCG 上测得的结果"
升级为"跨执行模式稳定的结论"。

## 3. 实测摘录 (KVM)

```
boot: autostart (cmdline fujo.run) -> direct launch
m142: T3 audit verified=1 iso_rc=0 ... M142 RESULT: PASS
m143: T4 calls<=1 ... M143 RESULT: PASS
m145: T2 [auto] io=5/5 model-calls+=0 ... M145 RESULT: PASS
m146: T1 plan verified=2 ... M146 RESULT: PASS
m147: T1 storm rate=99 ... rate=0 ... M147 RESULT: PASS
m141: [rules] anom=6/8 io=5/5 cls=4/6 (offline) ... M141 RESULT: PASS
```

## 4. 状态与后续

- **W31**: 第二硬件列 ✅ (KVM); #17 (GRUB/ISO 中断帧) 保持未解 = 真机引导前置;
- **W32**: 平台/执行模式对照证据 → docs/81 §7/§8.1 (三列矩阵 + #15/#16/#17 摘要);
- 后续独立波候选: #17 诊断 (GRUB 变体 A/B; 中断帧 dump); 在线波 KVM (WSL 模型链路);
  物理机波 (待设备)。
