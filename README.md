# FujoOS

> 一个零第三方依赖的 x86_64 原生操作系统内核 —— 内核、驱动、窗口系统、游戏层、
> 开发工具链、AI OS 独有层全栈自研。
> 三平台二进制**加载器子集**: Linux ELF / macOS Mach-O / Windows PE32+ 的最小静态样例可原地运行,
> 统一打包为自包含 **`.run`** (FUJR v1)。
> 注: "兼容"指加载器/垫片面, 不是完整用户态兼容; **能力边界见文末"已知限制"**
> (内部编译器为 C 子集、ACPI 高地址表未映射、参考机为 QEMU)。

**状态: v1.0 发布 → W20 真机化 → W29 双执行模式对照 (M1–M147 全部 PASS)** — 100 项里程碑
(docs/08-roadmap-100.md) 之后新增 W13–W21 机制线: virtio-blk/net + IPv4/TCP echo · VFS + busybox
目录命令 · ABI v1 冻结 · 系统内 tcc 自托管编译链 · SMP AP 上线 · 统一审计 (0x8C01) · 网络闭环
(UDP 克隆 → 系统内编译 → 运行) · **W20 脱 QEMU 专属**: 平台检测 (Bochs VBE 证据链 → is_qemu) +
LAPIC ICR 双语义运行时切换 + GRUB2 真机引导路径 + AHCI (SATA) 真盘 + FJFS 真机持久化回读 +
>1GiB 高内存恒等映射 + PCI 多函数枚举 (m137 PASS) · **W22–W28 AI 垂直六波** (m141–m147):
三引擎质量对照 (auto/model/rules) · 蒸馏闭环 (AI_CALLS 38→≤1) · 对抗验证 (blast-radius 可复现) ·
IO 所有权重判 (确定性基线) · 五职责全自监督 · 事件流哨兵 · **W29 第二执行模式**: TCG vs WHPX
(fujoregress `--accel whpx`; AI 波 7 项全一致; WHPX 36/37, m129 = 平台限制, docs/92)。
参考机回归 37/37, CI 38 用例, onebuild 3/3。

---

## 特性一览

| 层 | 能力 |
|----|------|
| 内核 | x86_64 长模式 · IDT/GDT/TSS · PIT 100Hz · 抢占多任务 (亲和/均衡统计) · 用户异常隔离 · 双 TSS + IRQ 路由 |
| 内存 | 虚拟内存 v0 · 按需零页 · 帧分配器 · U 位硬化 · 权重 mmap 按需页 (0x7Cxx) |
| ABI | ELF64 / Mach-O / PE32+ 加载器 · Linux x86-64 39 syscalls · darwin/win32 shim 家族 · .run (FUJR) 容器 |
| 存储 | ATA PIO + FJFS 4MiB 卷 (格式化/持久化, 两阶段跨重启 PASS) · **AHCI/SATA 真盘 (ICH9 q35, 0x8E01-03, W20)** · 页缓存/预读 (0x6Cxx) · 存档沙箱 (0x67xx) |
| 网络 | virtio-net legacy (W14a) · IPv4/UDP 往返 (ARP 应答) · 最小 TCP 服务器 SYN/ACK/PSH echo (W14b) · UDP 克隆闭环 (W21) |
| 平台 | W20 真机化 (平台检测/GRUB2/AHCI/PCI 多功能) · W29 双执行模式对照 (TCG / WHPX, --accel whpx) |
| 图形 | VBE 1024x768x32 + LFB · 5x7 字体 · 软件光栅 rect/tri/line (0x62xx) · blit/scale (0x68xx) · 着色器字节码 VM (0x69xx) |
| 输入 | PS/2 键盘 IRQ1 · 鼠标 IRQ12 (8042 序列/命中测试) · XInput · IME |
| 音频 | AC97 (0x5F01-04) · 4ch 混音器 + LPF/增益链 (0x5F05-09) |
| 桌面 | wmsg 窗口表/64 消息队/刷新 (0x55xx) · fujokit button/textbox/list |
| 游戏 | 游戏模式 (0x66xx) · Pong / Breakout 自研引擎 · 输入延迟基准 avg 94µs (0x6Fxx) · 性能验收 (docs/19) |
| AI OS | 模型权重 mmap (M86) · 模型卡权限/计费/审计 (0x7Dxx) · 会话检查点 (0x7Exx) · fujoctx 摘要/上下文压缩 (0x7Fxx/0x8001) · 能力表+审计 (0x81xx) · 意图路由 (0x82xx) · 推理执行器 (0x83xx) · 注册表+fupm (0x84xx) · AI For Next (M112+): shm-link 模型通道 · 事件环+结构态 (0x8002-05) · cap_exec 动作 (0x8105) · 异常哨兵 (0x8304) · **W22–W28 六波**: 引擎强制门 0x830F · 蒸馏闭环 (FJRU v2) · 对抗验证 (m144) · IO markov 基线 · act_verify 自监督 · 事件哨兵 0x8312 (m141–m147) |
| 工具链 | 系统内汇编器 (0x70xx) · 链接器 (0x71xx) · fujocc 编译壳 C→ELF64 全链 (0x75xx) · 编辑器 (0x74xx) · 调试器 TF 单步/int3 断点 (0x76xx) · trace (0x77xx) · 性能窗口 (0x78xx) · 单测 (0x79xx) · 泄漏 (0x7Axx) · minidump (0x7Bxx) |
| 交付 | fujopack / fujorun · fujoci 38 用例 · onebuild 3/3 · live 镜像+安装器 (0x87xx) · 签名/更新 (0x88xx) |

## 架构蓝图

```
长模式内核 (Rust, no_std, 零依赖)
 ├─ 内存/调度/异常 → 三 ABI 共存 (ELF/Mach-O/PE)
 ├─ 驱动 (VBE/PS2/ATA/AC97/PIT) → 桌面 (wmsg/fujokit/IME)
 ├─ 游戏层 (光栅/blit/着色器 VM/混音/模式) → 性能验收
 ├─ 工具链 (asm/ld/cc/调试/trace/CI) → 一键构建
 ├─ AI OS (权重页/模型卡/会话/审计/路由/执行器) → 验收
 └─ 交付 (live/安装/签名更新) → v1.0 发布
```

## 快速开始 (Windows 开发机 + QEMU)

```powershell
# 0) 依赖: rustup target add x86_64-unknown-none; winget install LLVM.LLVM qemu

# 1) 构建内核 (改 src 后确认出现 "Compiling" 行)
cd kernel; cargo build --release; cd ..

# 2) 扁平化 (pad 与 MB_HEADER 一致: 0x1C0000, load_end 0x2C0000)
python tools/flatten_elf.py kernel/target/x86_64-unknown-none/release/fujo-kernel kernel/fujo-kernel.bin --pad 0x1C0000

# 3) 启动 (任意 demo 作为 initrd; monitor 注入 "os run hermes")
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin -initrd sdk/linux/m30_linux.elf `
  -monitor telnet:127.0.0.1:4568,server,nowait -display none -no-reboot
# monitor: sendkey o s spc r u n spc h e r m e s ret

# 4) 回归 / 一键
python tools/fujoregress.py                 # 全量 37/37 (TCG); --accel whpx 走 WHPX 对照
python tools/ci.py                          # CI 38 用例
pwsh scripts/onebuild.ps1                   # hello/gui/game 模板构建+运行 3/3
```

## 可验证的证据 (Evidence)

- **回归闭环**: `python tools/fujoregress.py` 全量 **37/37** (TCG; AI 波 m141–m147 每波 PASS);
  `--accel whpx` 第二执行模式对照 36/37 (m129 = WHPX 平台限制, docs/92 #16);
- **AI 波在线证据**: `python tools/verify_ai.py --demo m141 ... --model qwen2.5:7b` (三引擎对照 8/8·1/5·6/6, novel 2/2)
  · `--evil` 对抗拦截 (m144: 越权 kill 全拒+审计, blast-radius 可复现);
- **官网站点**: [docs/index.html](docs/index.html) — 单文件官方站点 (GitHub Pages /docs);
- **运行时架构图**: [assets/archify/fujoos-runtime.html](assets/archify/fujoos-runtime.html) —
  仓库运行时架构可视化 (关键路径) + 视觉检查件 (1440x900 / 2048x1320, 明暗双版);
- **真机证据**: GRUB2 multiboot v1 真机引导路径 (docs/74 §4) · AHCI/SATA 真盘持久化回读
  (docs/75/76/79) · LAPIC 平台双语义 + CPUID/MSR 探测 (docs/74);
- **30 秒复现**: 上方快速开始命令 —— QEMU 无头启动 → monitor 注入 `os run hermes`。

## 仓库结构

```
kernel/        fujo-kernel (x86_64, no_std, 40+ 模块: syscall 分发面/驱动/AI OS)
sdk/           示例 (linux/win/mac/kit/hermes/user + templates)
tools/         flatten_elf / fujopack / fujorun / fujoregress / ci.py / qemu-kvm.ps1
scripts/       build-kernel.ps1 / cross-build.ps1 / onebuild.ps1
docs/          index.html (官网单文件站点) · index.md (索引) · 08 (100 项) · 51 (现状) · 57 (路线) · 58 (交接) · 11..92 (里程碑/波次)
```

## 文档

- [官网] docs/index.html (单文件站点) · [官网索引] docs/index.md
- [发布公告 v1.0] docs/49-release-notes.md · [项目现状] docs/51-project-status.md
- [100 里程碑路线图] docs/08-roadmap-100.md · [长期路线图] docs/57-long-roadmap.md
- [接手文档] docs/58-handoff.md (新对话从这里开始)
- [平台对照 W29] docs/92-w29-platform-contrast.md · [AI 波总结 W28] docs/89-w28-ai-vertical-summary.md
- [SDK 教程] docs/29-sdk-close.md · [2D 引擎分析] docs/10-2d-engine.md · [DXVK 可行性] docs/09-dxvk-feasibility.md

## 已知限制

- **AI 推理为非端侧**: 内核(AI OS 层)提供权重页/模型卡/审计/执行器编排面, 实际推理由宿主链路
  (COM2 → 宿主模型服务) 承载; 无本地 LLM。
- TCG 解释执行 (真机/KVM 预期 10-100x; M57 对照面已架, 建议重跑同 demo);
- FJFS 多簇写往返 (M99 修复单簇读回 + ATA 写等待; 大文件列后续);
- ACPI 表体 >64MiB 未映射 (M96 guard);
- WHPX 对照: INIT/SIPI 注入被拒绝 (m129 不适用, docs/92 #16); legacy 8259 路径需 `kernel-irqchip=off`
  (docs/92 #15) — 内核中断架构 APIC 化为 followup;
- 系统内编译器为 C 子集 (单函数); **真机路径 W20 已开启** (GRUB2 引导 / AHCI 真盘 / 平台检测原语均
  有实证), 剩余: 真机视频面 (INT 10h VBE) 与 USB 驱动面未完成。
