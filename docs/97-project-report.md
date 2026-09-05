# FujoOS 项目详细报告（2026-09 · W31 收官时点）

> 性质: 项目管理/交接/对外展示用全景报告（非论文稿; 论文见 docs/81）
> 数据来源: docs/01–96 全部里程碑文档 + fujoregress 日志 + 本报告内复现命令
> 状态: 分支 fujoos-ai-dev · fujoregress 38/38 · 论文稿终稿化完成 · 硬件波冻结

---

## 1. 执行摘要

FujoOS 是一个 **零第三方依赖的 no_std x86_64 Rust 内核**（约 40 个自研子系统），
以"**把 LLM 做成有证明边界的系统器官**"为研究目标（论文 Q1/Q2），当前已达
**机制层全部闭合**：AI 器官五职责闭环 + 三引擎质量量化 + 蒸馏自进化 + 对抗验证 +
真实事件流感知，且全部结论在 **TCG/WHPX/KVM 三种执行模式下行为一致**。
fujoregress 回归 38/38 全绿; 平台差异审计表 17 项（12 项已闭环、3 项文档化平台限制、
2 项工程 followup）; 自托管闭环（网络取源码→系统内编译→运行）已演示。
**剩余开放全部为规模/硬件/外部依赖性质，无机制层面的悬挂问题。**

## 2. 系统定位（是什么 / 不是什么）

| 是 | 不是 |
|---|---|
| 内核级 AI 器官研究原型（机制可证明） | 产品级 AI 操作系统 |
| 参考机 QEMU 可复现的完整自研栈 | 移植性被全面验证的系统（真机波冻结） |
| 接口可证明的 FUAI 架构参考实现 | 形式化验证系统（A1-A4 是断言测试） |
| 论文一的证据来源 | 论文二（ingress 翻译协议）——预留 |

## 3. 能力全景

| 域 | 子系统 | 状态 |
|---|---|---|
| 内核芯 | 启动/内存(≤4TiB 映射, 按需零页)/GDT-TSS/IDT/调度(轮转+信号)/进程隔离(每任务页表+CR3) | ✅ |
| 兼容 | ELF/long / Mach-O / PE32+ 装载; linux-x64 40 syscalls / darwin shim / win32 shim | ✅ |
| 磁盘 | ATA PIO + AHCI (真机 SATA) 双背板 + FJFS (写→刷→回读) + virtio-blk | ✅ |
| 网络 | virtio-net + IPv4/UDP + 最小 TCP 服务端 (slirp; 客户端数据面限制已取证) | ⚠️ 已知限制 |
| 图形/UI | VBE LFB + 位图字体 + 窗口系统 + 输入 (PS/2 键/鼠) + 2D/游戏层 | ✅ |
| 工具链 | 系统内 tcc-static (mbuild)+ runfile; 自托管闭环 (网络→编译→运行) | ✅ |
| AI 器官 | 5 职责 + shm/COM2 模型通道 + 域 + 审计 + 蒸馏 FJRU + 事件环 + 哨兵 | ✅ |
| SMP | LAPIC INIT/SIPI (TCG ✓; WHPX/嵌套KVM 平台限制; 真机待验) | ⚠️ |
| 平台协议 | 检测 (VBE+Bochs/ICR 双语义/hypervisor 品牌) + 17 项审计表 | ✅ |

## 4. 里程碑与波次（W17b → W31）

| 波 | 内容 | 交付 |
|---|---|---|
| W17b | SMP AP 唤醒 (QEMU LAPIC 低写语义取证) | m129 |
| W18 | VFS 目录语义 + busybox musl ls | m132 |
| W20 p1-p8 | 平台差异协议 14 项 + AHCI/FJFS/高内存/PCI 多功能 | m133-m137 |
| W21 | 网络完整性 + 自托管闭环 (UDP clone → tcc → run) | m139/m140 |
| W22 | AI 垂直 I: 三引擎对照 + anom 自监督验证 | m141/m142 |
| W23 | AI 垂直 II: 蒸馏闭环 (候选→FJRU v2→零调用) | m143 |
| W24 | AI 垂直 III: 对抗测试 (EVIL 注入×域拦截) | m144 |
| W25 | AI 垂直 IV: IO 所有权重判 (Markov 基线 5/5) | m145 |
| W26 | AI 垂直 V: 五职责全自监督 (act_verify) | m146 |
| W27 | AI 垂直 VI: 哨兵接真实事件流 (digest 99→0) | m147 |
| W28 | 六波证据收尾 (所有权矩阵/系统结论) — 论文 §8 | docs/89 |
| W29 | 第二执行模式对照 (TCG vs WHPX; #15/#16 发现 + submit_req 修复) | docs/92 |
| W30 | 真机就绪包 (autostart + GRUB ISO + #17 发现) | docs/93 |
| W31 | KVM 硬件列 + **#17 解密 (SS=0x18) + icr 判据修正** | docs/94/95 |
| (冻结) | 物理机/云 KVM 波 → 文档化待硬件 | docs/96 |

## 5. 关键证据（论文支撑）

1. **模型边际价值 = 规则边界外**：anom novel 2/2 vs 0/2、cls 2/2 vs 0/2 (qwen2.5:7b, 19 样本);
2. **模型规模阈值**：0.5b 全答 RUN 不可用 → ≥7b 才有增量;
3. **blast-radius 可复现实验**：恶意 PLAN (kill 越权) 域门拦截 + 审计 + revoke 即效;
4. **自进化闭环**：在线命中→候选(4)→bake→FJRU v2→调用率 ~38→≤1;
5. **职责所有权实测判定**：io 由确定性 Markov 基线拥有 (5/5 零调用 vs 模型 1/5);
6. **器官感知自身**：事件风暴 rate=99→哨兵→隔离→0→恢复;
7. **平台独立性**：AI 波六件套 × {TCG, WHPX, KVM} 行为全一致。

## 6. 平台/执行模式矩阵

| 通道 | 说明 | AI 波 | SMP |
|---|---|---|---|
| TCG | QEMU 参考机 (主线, 可复现) | ✅ | ✅ |
| WHPX | Windows Hypervisor (需 kernel-irqchip=off) | ✅ | ❌ 架构限制 (INIT/SIPI) |
| WSL2 嵌套 KVM | Linux 硬件虚拟化 (WSL2 /dev/kvm) | ✅ | ❌ LAPIC 虚拟化不可用 |
| 物理机/非嵌套 KVM | 待硬件 (冻结, docs/96) | 预期 ✅ | 预期 ✅ |

## 7. 平台差异审计表（docs/74 · 17 项 → 摘要）

12 ✅ 闭环（ICR 双语义/磁盘双背板/时钟校准/GRUB 引导/多核拓扑/PCI 多功能/
framebuffer 任意地址/mbi cmdline/超时判据/审计导出/段一致性 #17/…）·
3 ⚠️ 文档化平台限制（#15 WHPX APIC-only → kernel-irqchip=off + APIC 化 followup；
#16 INIT/SIPI WHPX 拒绝 + 嵌套 KVM LAPIC 不可用；#12 ACPI 大表）·
2 ⬜ 工程 followup（中断架构 APIC 化; 安装器外设面）。

## 8. 未解与开放项（分层）

> 完整分档（A 无头绪 6 项 / B 有路径未排期 16 项 / C 外部依赖 4 项）见 **docs/98**。

**A. 外部依赖（唯一阻塞性质）**
- 物理机/非嵌套 KVM 验证（冻结, 待硬件 — docs/96）
- 文献核对 + 选刊（zcode — 论文提交前唯一外部关键路径）
- 外部评审（投稿前社区公开 + 导师通读）

**B. 工程 followup（已定方向, 未排期)**
- 中断架构 APIC 化（#15 根治; 面向现代平台约定）
- TCP 客户端数据面（QEMU slirp 限制; KVM 对照可低成本验证）
- >4GiB 消费端 / FJFS 多簇写 / ACPI 大表 / 安装器外设面（历史欠账, 见 docs/74/76/77）

**C. AI 规模扩展（ask2 §2 七个缺口, 均"规模"而非"机制"）**
- 样本集 19→40; 多模型曲线; 蒸馏全自动 (系统内归纳); env 自监督
- io 流持久化; 在线波 KVM 链路; 对抗样本多样性

## 9. 关键决策记录（含否决）

| 决策 | 理由 | 文档 |
|---|---|---|
| clone 传输用 UDP | QEMU 9.2 slirp 丢 guest→host TCP 数据 (证据链) | docs/80 |
| io 职责交给 Markow 基线 (非模型) | 实测 5/5 vs 1/5; 所有权=结果 | docs/86 |
| WHPX 用 kernel-irqchip=off | 否则 legacy 8259 死锁 (#15); split 不支持 | docs/92 |
| m129 在 WHPX 标记平台限制 | INIT/SIPI 架构性不可投递 (三模式实验) | docs/94 |
| 物理机波冻结 | 不触碰用户 Windows (U 盘引导方案已就绪, 待决策) | docs/96 |
| A1-A4 定位"断言测试"非形式证明 | 论文诚实边界; 形式化列为 future | docs/81 §8.2 |

## 10. 验证体系（五条线 + 纪律）

1. `fujoregress` 38 用例 (TCG 主线, needle 早退)
2. `fujoregress --accel whpx` (第二模式, 36/37)
3. `verify_ai` (模型在线: m141/m144, 7b + --evil)
4. `kvm-run.sh` (KVM 硬件列)
5. `make-boot-iso + -cdrom` (GRUB/ISO + autostart)
纪律: 每波 Compiling 确认 / BSS<0x2C0000 / 回归全绿 / docs+commit(波次+回归数)+push。
铁律: fujoregress 与 verify_ai 不可并行 (monitor 4568)。

## 11. 风险与边界

- **规模边界**: n=19 样本/8 任务表/16 审计槽 —— 声明为接口验证而非规模验证;
- **BSS 预算**: 尾 0x2BCC30, 余量 ~13KB, 每波 +1KB 趋势 (大功能需第二系统);
- **平台边界**: 所有结论 TCG/WHPX/KVM 一致; 物理机未验 (已文档化);
- **自托管半自治**: bake 归纳含外部步骤 (7B 宿主), 系统内归纳为 future;
- **外部环节**: 论文 related-work/选刊未完成 (zcode); 单作者无评审。

## 12. 未来路线（三线）

- **论文线**: 文献+选刊 (zcode) → 投稿; 论文二 (ingress-time 二进制翻译, 预留)
- **硬件线**: 解除冻结 → ISO+autostart+COM1 三件套 (全就绪, 10 分钟流程, docs/96)
- **技术线**: 中断 APIC 化 (最大结构性 followup) → 大内存消费端 → 多模型规模化

## 13. 复现入口

见 docs/81 Appendix A（五条命令线）; 论文证据索引 docs/81 Appendix B/C。
仓库自查: `git log --oneline -30` 可见全部波次 commit (含波次与回归数)。
