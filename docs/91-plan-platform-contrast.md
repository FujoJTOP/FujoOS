# 91 · 修正计划: AI 波平台对照 (W29–W32, 四波)

> 背景/修正: 用户指出六波 AI 实验 (W22–W28, m141–m147) 全部 QEMU TCG, 无对照;
> "脱 QEMU 工程" (W20) 实际交付 = 平台差异审计表 14 项 + 语义协议 + 真机就绪
> 码路径 (GRUB2 模板/AHCI 双背板), **物理验证停在"就绪"** (ISO 未生成, checklist
> 未做, docs/76 的"真机 SATA"是 QEMU q35 路径)。
> 事实: 本机 QEMU 加速 = tcg + **whpx** (WHPX 可用); WSL2 Ubuntu 存在 (可装
> grub-mkrescue, 嵌套 KVM 可试); 物理机接入 = 无证据 (需用户提供)。
> 本计划 = 逐波交付: 每波自验 + docs + commit(波次与回归数) + push。

| 波 | 主题 | 交付 | 验收 |
|---|---|---|---|
| **W29** | 第二执行模式对照 (TCG vs WHPX) | `qemu-kvm.ps1` 双模式 (accel 参数化) + `fujoregress.py --accel` + **m141–m147 全 AI 波 × {TCG, WHPX} 行为一致性表** (含在线波: WHPX 下 monitor 仍在, shm 模型通道可用) | 对照表: 每 demo 平台 PASS 状态 + 差异记录; 回归 37/37 双模式 |
| **W30** | 真机就绪包 (W20 二期旧账) | WSL2 装 grub-mkrescue → **引导 ISO** (多 boot demo 选择) + 内核 **autostart** (mbi cmdline 解析 `fujo.run=<demo>` → 直启路径, 真机无 sendkey) + COM1 捕获模板 + 真机 checklist 文档 | ISO 在 QEMU 下验证启动 (无 -kernel 无 -initrd, 纯 ISO 引导) + autostart demo PASS; 回归不破坏 (cmdline 缺省=旧路径) |
| **W31** | 第二列: 物理机或嵌套 KVM | WSL2 内 QEMU `-enable-kvm` 跑五件套 + m141 离线 (或用户提供物理机: ISO 写入 + COM1 捕获跑同五件套); **m141/m144 在线波 = 继续 QEMU 系 (WHPX/KVM monitor 存在)** | KVM/WSL2 或真机: m142/m143/m145/m146/m147 + m144 离线 PASS; 表更新为三列 (TCG/WHPX/硬件) |
| **W32** | 平台一致性证据 → 论文 | docs/81 §7/§8.1 增"执行模式与平台对照"小节 + Threats to validity 正文化 (三列数据) | 论文证据节更新 + 全绿收尾 |

**边界 (诚实声明)**:
- 物理机波依赖用户硬件; WSL2 嵌套 KVM 需测试 (Hyper-V 宿主下 /dev/kvm 可能不可用) —— W31 时先探测, 不可用则 W31 降级为"真机就绪包验收 (ISO 引导)" + 物理机波标注"待设备接入";
- 本计划不改变"参考机 = QEMU TCG 可复现"的主线; 对照列的意义 = 证明结论不依赖单个执行模式。

**纪律**: 每波 fujoregress 全绿 (WHPX 波额外确认 TCG 37/37)、BSS 检查、commit 含波次与回归数、docs 同步。
