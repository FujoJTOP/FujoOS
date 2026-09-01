# FujoOS

> 一个零第三方依赖的 x86_64 原生操作系统内核 —— 内核、驱动、窗口系统、游戏层、
> 开发工具链、AI OS 独有层全栈自研。
> 三平台二进制第一公民: Linux ELF / macOS Mach-O / Windows PE32+, 统一打包为自包含 **`.run`** (FUJR v1)。

**状态: v1.0.0 发布** — 路线图 M1–M100 全部完成 (docs/08-roadmap-100.md 100 项 `[x]`,
每项均经 QEMU 无头验证 + 提交推送)。兼容矩阵 9/9, CI 38 用例, onebuild 3/3。

---

## 特性一览

| 层 | 能力 |
|----|------|
| 内核 | x86_64 长模式 · IDT/GDT/TSS · PIT 100Hz · 抢占多任务 (亲和/均衡统计) · 用户异常隔离 · 双 TSS + IRQ 路由 |
| 内存 | 虚拟内存 v0 · 按需零页 · 帧分配器 · U 位硬化 · 权重 mmap 按需页 (0x7Cxx) |
| ABI | ELF64 / Mach-O / PE32+ 加载器 · Linux x86-64 39 syscalls · darwin/win32 shim 家族 · .run (FUJR) 容器 |
| 存储 | ATA PIO + FJFS 4MiB 卷 (格式化/持久化, 两阶段跨重启 PASS) · 页缓存/预读 (0x6Cxx) · 存档沙箱 (0x67xx) |
| 图形 | VBE 1024x768x32 + LFB · 5x7 字体 · 软件光栅 rect/tri/line (0x62xx) · blit/scale (0x68xx) · 着色器字节码 VM (0x69xx) |
| 输入 | PS/2 键盘 IRQ1 · 鼠标 IRQ12 (8042 序列/命中测试) · XInput · IME |
| 音频 | AC97 (0x5F01-04) · 4ch 混音器 + LPF/增益链 (0x5F05-09) |
| 桌面 | wmsg 窗口表/64 消息队/刷新 (0x55xx) · fujokit button/textbox/list |
| 游戏 | 游戏模式 (0x66xx) · Pong / Breakout 自研引擎 · 输入延迟基准 avg 94µs (0x6Fxx) · 性能验收 (docs/19) |
| AI OS | 模型权重 mmap (M86) · 模型卡权限/计费/审计 (0x7Dxx) · 会话检查点 (0x7Exx) · fujoctx 摘要/上下文压缩 (0x7Fxx/0x8001) · 能力表+审计 (0x81xx) · 意图路由 (0x82xx) · 推理执行器 (0x83xx) · 注册表+fupm (0x84xx) |
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

# 2) 扁平化 (pad 与 MB_HEADER 一致: 0x1A0000)
python tools/flatten_elf.py kernel/target/x86_64-unknown-none/release/fujo-kernel kernel/fujo-kernel.bin --pad 0x1A0000

# 3) 启动 (任意 demo 作为 initrd; monitor 注入 "os run hermes")
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin -initrd sdk/linux/m30_linux.elf `
  -monitor telnet:127.0.0.1:4568,server,nowait -display none -no-reboot
# monitor: sendkey o s spc r u n spc h e r m e s ret

# 4) 回归 / 一键
python tools/fujoregress.py     # 兼容矩阵 9 用例
python tools/ci.py              # CI 38 用例
pwsh scripts/onebuild.ps1       # hello/gui/game 模板构建+运行 3/3
```

## 仓库结构

```
kernel/        fujo-kernel (x86_64, no_std, 40+ 模块: syscall 分发面/驱动/AI OS)
sdk/           示例 (linux/win/mac/kit/hermes/user + templates)
tools/         flatten_elf / fujopack / fujorun / fujoregress / ci.py / qemu-kvm.ps1
scripts/       build-kernel.ps1 / cross-build.ps1 / onebuild.ps1
docs/          index.md (官网) · 08-roadmap-100.md (100 项) · 11..48 (里程碑文档)
```

## 文档

- [官网] docs/index.md · [发布公告 v1.0] docs/49-release-notes.md
- [100 里程碑路线图] docs/08-roadmap-100.md
- [SDK 教程] docs/29-sdk-close.md · [2D 引擎分析] docs/10-2d-engine.md · [DXVK 可行性] docs/09-dxvk-feasibility.md

## 已知限制

- TCG 解释执行 (真机/KVM 预期 10-100x; M57 对照面已架, 建议重跑同 demo);
- FJFS 多簇写往返 (M99 修复单簇读回 + ATA 写等待; 大文件列后续);
- ACPI 表体 >64MiB 未映射 (M96 guard);
- 系统内编译器为 C 子集 (单函数); 参考机为 QEMU (真机矩阵列 M96+ 扩展)。
