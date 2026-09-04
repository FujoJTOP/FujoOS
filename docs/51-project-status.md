# 51 — 项目现状描述 (FujoOS, 2026-09)

> 本文档是项目整体快照: 定位、架构、完成度、能力面、验收数据、
> 已知限制与下一步。用于对外演示/交接/继续开发时的一页式总览。
> 最新更新: **W29 (第二执行模式对照, TCG vs WHPX)**。

## 1. 项目定位

**FujoOS 是一个零第三方依赖的 x86_64 原生操作系统内核**, 全栈自研:
内核 → 驱动 → 桌面 → 游戏层 → 开发工具链 → AI OS 独有层 → 交付机制。
三平台二进制**加载器子集**(ELF64 / Mach-O / PE32+ 的最小静态样例可原地运行),
统一打包为自包含 **`.run`** (FUJR v1) 容器。

- 语言: Rust (no_std, edition 2021), 目标 `x86_64-unknown-none`, rust-lld 链接。
- 架构: 长模式内核 + 用户态程序(多任务抢占, 每任务页表链 + CR3 切换)。
- 运行平台: **QEMU 参考机 (TCG)** 为基线; **W29 起第二执行模式 WHPX 对照**
  (`fujoregress.py --accel whpx`); 真机路径 (GRUB2/AHCI/平台检测) W20 已开启。

## 2. 完成度

| 段 | 状态 |
|----|------|
| M1–M100 里程碑 (docs/08-roadmap-100.md) | ✅ 全部完成 (100 项 `[x]`, 每项实现→构建→QEMU验证→提交推送) |
| M101–M108 桌面可操作面 + 用户态代理 | ✅ 完成 (桌面交互闭环; 用户态代理+高地址窗口程序双任务轮转) |
| M109–M120 AI For Next 阶段一 | ✅ 完成 (时延协议/公理化/权限域/策略蒸馏/审计自改进) |
| M121–M137 W11–W20 | ✅ 完成 (每任务地址空间 · VFS+tmpfs · PCI/virtio · TCP/IP · ABI v1 · SMP · 统一审计 · W20 真机化) |
| W21 | ✅ 完成 (网络完整性 + 自托管闭环: UDP 克隆→系统内 tcc 编译→运行, m139/m140 PASS) |
| W22–W28 AI 垂直六波 (m141–m147) | ✅ 完成 (三引擎对照 · 蒸馏闭环 · 对抗验证 · IO 所有权重判 · 全自监督 · 事件流哨兵) |
| **W29 第二执行模式对照** | ✅ 完成 (TCG vs WHPX 全矩阵; AI 波 7 项行为一致; WHPX 36/37, m129=平台限制) |

**验证现状(最近)**: 参考机回归 **37/37** (TCG; W28 起), WHPX 对照 36/37 (仅 m129 =
WHPX 拒绝 INIT/SIPI 注入, 非内核缺陷), CI 38 用例, onebuild 3/3, FJFS 两阶段持久化 PASS。
AI 在线波 (qwen2.5:7b 经 shm-link): m141 三引擎对照 8/8·1/5·6/6 (novel 2/2), m144 对抗
拦截——越权 kill 全拒+审计 (blast-radius 定理可复现)。

## 3. 架构分层

```
长模式内核 (Rust, no_std, 零依赖)
  ├─ 内核芯: GDT/IDT/PIT100Hz/系统调用门; 抢占多任务 (亲和/均衡统计/信号/fork, 每任务页表链)
  ├─ 内存: 虚拟内存 v0 (按需零页/帧分配器/U 位硬化) + 高地址用户映射 + >1GiB 恒等映射
  ├─ 驱动: VBE + LFB · PS2 键鼠 · ATA PIO / AHCI (SATA) + FJFS · AC97 · virtio-blk/net · PCI 多功能
  ├─ 网络: 自研最小 IPv4/UDP + TCP (ARP 应答; QEMU slirp), virtio-net legacy
  ├─ ABIs: ELF64/Mach-O/PE32+ 加载器 + Linux 39 syscalls + darwin/win32 shim + ABI v1 冻结
  ├─ 桌面: desk(0x5Bxx) · wm 窗口(0x55xx) · icon(0x59xx) · font(0x56xx) ·
  │        term(0x5Axx) · vbe(0x5Cxx) · fujokit (纯用户态头文件库)
  ├─ 游戏: 光栅(0x62xx)/blit(0x68xx)/着色器VM(0x69xx)/混音(0x5Fxx)/模式(0x66xx)
  ├─ 工具链: asm(0x70xx)/ld(0x71xx)/cc(0x75xx)/编辑器(0x74xx)/调试器(0x76xx)/
  │        trace(0x77xx)/性能窗口(0x78xx)/单测(0x79xx)/泄漏(0x7Axx)/转储(0x7Bxx)
  ├─ 系统: VFS+tmpfs (模型即设备 /dev/model0) · SMP AP 上线 · 统一审计 (0x8C01) · 应用管理 (0x8B01)
  ├─ AI OS: 权重mmap(0x7Cxx)/模型卡(0x7Dxx)/会话(0x7Exx)/fujoctx(0x7Fxx,0x8001)/
  │        权限审计(0x81xx)/路由(0x82xx)/执行器(0x83xx)/注册表+fupm(0x84xx)
  │        + AI For Next: 事件环(0x8002-05)/cap_exec(0x8105)/五职责(0x8304-08)/
  │         蒸馏引擎(0x830B,C)/引擎门(0x830F)/事件哨兵(0x8312)
  └─ 交付: ACPI/PCI(0x85xx)/hw 面(0x86xx)/安装器(0x87xx)/签名更新(0x88xx)
```

## 4. 能力面摘要

| 面 | 关键原语/组件 | 验证锚点 |
|----|---------------|----------|
| 进程 | fork(57)/信号/用户异常隔离/每任务 CR3/强杀 | m20/m22/m84/m121 |
| 持久存储 | FJFS 4MiB 卷(格式化/写/读/双阶段跨重启) + AHCI 真盘背板 + 页缓存/预读 | m97/m99/m134/m135 |
| 网络 | IPv4/UDP 往返 (ARP 应答) + 最小 TCP echo + UDP 克隆闭环 | m124/m125/m139/m140 |
| 图形 | 光栅/字体/图标/窗口/桌面/控件 | m46/m101-106 |
| 音频 | AC97 探测 + 4ch 混音/LPF/增益链 | m63 |
| AI | 模型卡计费-审计/意图路由/执行器双模式/权重按需页 + 五职责 + 蒸馏/对抗/自监督/哨兵 | m86-m95, m141-m147 |
| 工具 | 系统内 C→asm→ELF64 全链 (tcc 自托管) + 调试器 + CI | m71-m85, m128 |
| 平台 | 双执行模式对照 (TCG/WHPX) + 真机就绪 (GRUB2/AHCI/平台检测) | m137, W29 |

## 5. 验收数字 (诚实面)

- 每个里程碑: QEMU 串口 `MXX RESULT: PASS` 日志断言 (140+ 条可回放)。
- 参考机回归 **37/37** (fujoregress, ~25s/用例早退); WHPX 对照 36/37 (m129=平台限制)。
- CI 38 用例 (fujoci, ~10 分钟); onebuild 3/3。
- AI 波: m141 三引擎 19 样本金标准集 (rules 6/8·0/5·4/6; 7b 8/8·1/5·6/6; novel 2/2 vs 0/2);
  m143 蒸馏后 AI_CALLS ~38→≤1; m145 io [auto] 5/5 零模型调用; m146 五职责自监督 verified 2/2/1;
  m147 事件风暴 rate 99→sentinel 自动隔离→0。
- 游戏层: pong 60 帧轨迹 / breakout 10 帧 avg 94µs 输入→渲染延迟。
- 桌面: 开窗/拖动/菜单/文本框/文件保存-读回 5 段集成回归通过。
- 会话检查点/更新签名(FNV)/安装器 bootcount 跨重启递增全过。

## 6. 已知限制 (对外口径 — 与 README/docs 一致)

1. **二进制兼容为加载器子集**, 非完整用户态兼容 (PE 仅 kernel32 垫片家族;
   Mach-O 仅 darwin 8 syscall 最小集; 样例均为静态无 libc)。
2. **推理非端侧**: AI OS 层是编排/元数据/审计面; 推理由宿主链路
   (COM2 → 宿主模型服务) 承载; 无本地 LLM。
3. **基线 = QEMU TCG**; 第二执行模式 WHPX 对照已架 (W29): WHPX 拒绝 INIT/SIPI 注入
   (m129 不适用) + legacy 8259 需 `kernel-irqchip=off`。真机/KVM 预期 10-100x;
   真机视频面 (INT 10h VBE) 与 USB 驱动面未完成。
4. FJFS **多簇写往返**记录为已知 (M99 修复单簇读回 + ATA PIO 写等待)。
5. ACPI 表体 >64MiB 未映射 (M96 guard)。
6. 系统内编译器 = C 子集 (单函数)。
7. 内核中断架构仍为 legacy 8259 直连 (W29 发现 WHPX 默认路径差异) →
   **APIC 化列为 followup** (docs/74 #15/#16)。

## 7. 使用

```powershell
# 构建/扁平化
cd kernel; cargo build --release; cd ..
python tools/flatten_elf.py kernel/target/x86_64-unknown-none/release/fujo-kernel kernel/fujo-kernel.bin --pad 0x1C0000

# 启动 (示例: M106 桌面操作回归; 带盘持久)
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin -initrd sdk/linux/m106_operate.elf `
  -drive file=disk.img,format=raw,if=ide -serial file:qemu.log -display none -no-reboot
# (demo 自动执行, 无需 monitor 注入; 看 qemu.log 中 RESULT)

# 全量回归 (TCG) / 第二执行模式 (WHPX, W29)
python tools/fujoregress.py
python tools/fujoregress.py --accel whpx    # 36/37, m129=WHPX 平台限制

# AI 在线验证 (模型需在宿主 Ollama, qwen2.5:7b)
python tools/verify_ai.py --demo m141 --needle "M141 RESULT: PASS" --model qwen2.5:7b --timeout 420
python tools/verify_ai.py --demo m144 --needle "M144 RESULT: PASS" --model qwen2.5:7b --timeout 420 --evil

# 其他
python tools/ci.py; pwsh scripts/onebuild.ps1
```

## 8. 下一步 (按优先级)

1. **W29-followup**: 内核中断架构 APIC 化 (WHPX 默认路径 + 现代平台; docs/74 #15)。
2. **W30**: 真机就绪包 —— WSL2 grub-mkrescue → 引导 ISO + 内核 autostart
   (mbi cmdline `fujo.run=<demo>`) + COM1 捕获 + 真机 checklist (docs/91 计划)。
3. **W31**: 第二列 —— 物理机或 WSL2 嵌套 KVM 跑五件套; 不可用则降级为 ISO 引导验收。
4. **W32**: 平台一致性证据 → docs/81 论文证据节 (三列数据正文化)。
5. FJFS 多簇写修复; ACPI 高内存表映射; 系统内编译器扩展。

> 里程碑打卡与逐项文档: docs/08-roadmap-100.md (M1-100) + docs/11..92 (逐波)
> + docs/58-handoff.md (新对话起点) + 本文件 (现状快照)。
