# 51 — 项目现状描述 (FujoOS, 2026-09)

> 本文档是项目整体快照: 定位、架构、完成度、能力面、验收数据、
> 已知限制与下一步。用于对外演示/交接/继续开发时的一页式总览。

## 1. 项目定位

**FujoOS 是一个零第三方依赖的 x86_64 原生操作系统内核**, 全栈自研:
内核 → 驱动 → 桌面 → 游戏层 → 开发工具链 → AI OS 独有层 → 交付机制。
三平台二进制**加载器子集**(ELF64 / Mach-O / PE32+ 的最小静态样例可原地运行),
统一打包为自包含 **`.run`** (FUJR v1) 容器。

- 语言: Rust (no_std, edition 2021), 目标 `x86_64-unknown-none`, rust-lld 链接。
- 架构: 长模式内核 + 用户态程序(多任务 PIT 轮转, 小地址空间共享模型)。
- 运行平台: **QEMU 参考机** (Bochs-VBE / PS2 / ATA / AC97), 全部验证经
  无头 QEMU + 串口日志断言。

## 2. 完成度

| 段 | 状态 |
|----|------|
| M1–M100 里程碑 (docs/08-roadmap-100.md) | ✅ 全部完成 (90 项 `[x]`, 每项实现→构建→QEMU验证→提交推送) |
| M101–M106 桌面可操作面 | ✅ 完成 (Win1.0 级交互闭环: 桌面→点击→开窗/拖动/菜单/文本框/文件对话框/集成回归) |
| M107 桌面会话 v0 (内核态) | ✅ 完成 (无模块 boot 直进图形桌面; 合成/真鼠标双击链; 保留为无模块回退路径) |
| M108 用户态桌面代理 + 高地址窗口程序 | ✅ 完成 (代理=任务0 + 窗口程序=任务1 双用户态 PIT 轮转; 0x5B10 启动/替换; TTY 行读回) — **PASS** |
| 代理/程序镜像 | m108_desk.elf / hermes-high.elf / m107_tty-high.elf; user-high.ld; 内核对高区映射(0x5B10/0x5B11) + 帧保留 |

**验证现状(最近)**: 兼容矩阵 9/9, CI 38 用例, onebuild 3/3, FJFS 两阶段持久化,
M101-106 各 PASS, **M107 PASS + M108 PASS** (用户态双任务轮转 → 窗口程序
TTY 行 rows>0 → 完整 PASS)。

## 3. 架构分层

```
长模式内核 (Rust, no_std, 零依赖)
 ├─ 内核芯: GDT/IDT/PIT100Hz/系统调用门; 抢占多任务 (亲和/均衡统计/信号/fork)
 ├─ 内存: 虚拟内存 v0 (按需零页/帧分配器/U 位硬化) + M108 高地址 2MiB 用户映射
 ├─ 驱动: VBE 1024x768x32 + LFB · PS2 键鼠 (IRQ1/12) · ATA PIO + FJFS · AC97
 ├─ ABIs: ELF64/Mach-O/PE32+ 加载器 + Linux 39 syscalls + darwin/win32 shim
 ├─ 桌面: desk(0x5Bxx) · wm 窗口(0x55xx) · icon(0x59xx) · font(0x56xx) ·
 │        term(0x5Axx) · vbe(0x5Cxx) · fujokit (纯用户态头文件库)
 ├─ 游戏: 光栅(0x62xx)/blit(0x68xx)/着色器VM(0x69xx)/混音(0x5Fxx)/模式(0x66xx)
 ├─ 工具链: asm(0x70xx)/ld(0x71xx)/cc(0x75xx)/编辑器(0x74xx)/调试器(0x76xx)/
 │        trace(0x77xx)/性能窗口(0x78xx)/单测(0x79xx)/泄漏(0x7Axx)/转储(0x7Bxx)
 ├─ AI OS: 权重mmap(0x7Cxx)/模型卡(0x7Dxx)/会话(0x7Exx)/fujoctx(0x7Fxx,0x8001)/
 │        权限审计(0x81xx)/路由(0x82xx)/执行器(0x83xx)/注册表+fupm(0x84xx)
 └─ 交付: ACPI/PCI(0x85xx)/hw 面(0x86xx)/安装器(0x87xx)/签名更新(0x88xx)
```

## 4. 能力面摘要

| 面 | 关键原语/组件 | 验证锚点 |
|----|---------------|----------|
| 进程 | fork(57)/信号(0x512x)/用户异常隔离/强杀 | m20/m22/m84 |
| 持久存储 | FJFS 4MiB 卷(格式化/写/读/双阶段跨重启) + 页缓存/预读 | m97/m99 |
| 图形 | 光栅/字体/图标/窗口/桌面/控件 | m46/m101-106 |
| 音频 | AC97 探测 + 4ch 混音/LPF/增益链 | m63 |
| AI | 模型卡计费-审计/意图路由/执行器双模式/权重按需页 | m86-m95 |
| 工具 | 系统内 C→asm→ELF64 全链 + 调试器 + CI | m71-m85 |

## 5. 验收数字 (诚实面)

- 每个里程碑: QEMU 串口 `MXX RESULT: PASS` 日志断言 (100+ 条可回放)。
- 兼容矩阵 9/9; CI 38 用例 (fujoci, ~10 分钟); onebuild 3/3。
- 游戏层: pong 60 帧轨迹 / breakout 10 帧 avg 94µs 输入→渲染延迟。
- 桌面: 开窗/拖动/菜单/文本框/文件保存-读回 5 段集成回归通过。
- 会话检查点/更新签名(FNV)/安装器 bootcount 跨重启递增全过。

## 6. 已知限制 (对外口径 — 与 README/docs 一致)

1. **二进制兼容为加载器子集**, 非完整用户态兼容 (PE 仅 kernel32 垫片家族;
   Mach-O 仅 darwin 8 syscall 最小集; 样例均为静态无 libc)。
2. **推理非端侧**: AI OS 层是编排/元数据/审计面; 推理由宿主链路
   (COM2 → 宿主模型服务) 承载; 无本地 LLM。
3. **参考机 = QEMU (TCG)**: 真机/KVM 预期 10-100x, M57 对照面已架。
4. FJFS **多簇写往返**记录为已知 (M99 修复单簇读回 + ATA PIO 写等待)。
5. ACPI 表体 >64MiB 未映射 (M96 guard)。
6. 系统内编译器 = C 子集 (单函数)。
7. M107 内核桌面主循环版"程序执行"仍受限于内核态 hlt (PIT 仅在用户态切换)
   → 该路径仅无模块回退; **M108 用户态代理+高地址窗口程序已解除此限制**。

## 7. 使用

```powershell
# 构建/扁平化
cd kernel; cargo build --release; cd ..
python tools/flatten_elf.py kernel/target/x86_64-unknown-none/release/fujo-kernel kernel/fujo-kernel.bin --pad 0x1A0000

# 启动 (示例: M106 桌面操作回归; 带盘持久)
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin -initrd sdk/linux/m106_operate.elf `
  -drive file=disk.img,format=raw,if=ide -serial file:qemu.log -display none -no-reboot
# (demo 自动执行, 无需 monitor 注入; 看 qemu.log 中 RESULT)

# M108 桌面会话 (用户态代理 + 高地址窗口程序; 无注入)
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin -initrd sdk/linux/m108_desk.elf `
  -serial file:qemu.log -display none -no-reboot
# (看 qemu.log 中 "m108: M108 RESULT: PASS")

# 回归
python tools/fujoregress.py; python tools/ci.py; pwsh scripts/onebuild.ps1
```

## 8. 下一步 (按优先级)

1. ✅ **M108 收尾完成**: 用户态桌面代理 + 高地址窗口程序双任务轮转 →
   TTY 行读回 → `m108: M108 RESULT: PASS` (QEMU 串口断言)。
2. M109 真鼠标注入回归 (PS/2 包) / GUI 显示 (`-display gtk`) 人工可操作。
3. FJFS 多簇写修复; ACPI 高内存表映射; 系统内编译器扩展。
4. KVM 对照基准重跑 (m58/m69/m101 同帧)。

> 里程碑打卡与逐项文档: docs/08-roadmap-100.md (M1-100) +
> docs/50-desktop.md (M101-108) + 本文件 (M107 起)。
