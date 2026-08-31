# FujoOS — 通用兼容操作系统

> 一个操作系统，运行所有系统的东西：Windows `.exe` / `.dll`、Linux `ELF`、macOS `Mach-O`，
> 最终统一编译打包为 FujoOS 原生可执行文件 **`.run`**（容器格式 magic：`FUJR`）。
> UI、性能、游戏、开发者体验四大支柱同等优先。

---

## 一、愿景与核心命题

FujoOS 的目标不是"又一个 Linux 发行版"，而是一个**以应用程序为中心的全栈操作系统**：

1. **一次打包，到处运行**：把 PE / ELF / Mach-O 二进制"转译"成 `.run` —— 一个自包含的
   FujoOS 可执行容器，内含清单、原始二进制或预翻译代码、依赖资源和签名。
2. **三平台二进制兼容**：内核提供**原生 syscall 表**（Linux ABI 第一优先级）、`ntdll` 级
   Win32 垫片层和 Mach trap / BSD 兼容层，配合 dynarec 交叉架构翻译（类 Rosetta / Box64）。
3. **同等重要的一级公民**：
   - **UI**：GPU 合成器 `fujocom` + 控件库 `fujokit` + 窗口管理 `fujowm`；
   - **性能**：Rust 内核、零拷贝 IPC、EEVDF 调度、1 GiB 大页、目标 boot < 2s；
   - **游戏**：DXVK / vkd3d-proton 式 DirectX→Vulkan 管线、低延迟输入、XAudio2/XInput 垫片；
   - **开发**：`fujocc`（LLVM 交叉编译器）、`fujo-rs`（Rust 标准库移植）、`fupm` 包管理器、
     `fujopack` 打包器、`fujorun` 运行器。

## 二、技术选型（一句话决策）

| 层 | 选择 | 理由 |
|---|---|---|
| 内核语言 | **Rust (no_std)** | 内存安全 + 零成本抽象 + 单二进制 |
| 内核架构 | 混合内核：宏内核 + 用户态驱动/服务 | 性能与隔离的工程折中（参考 NT/WinDbg 模式） |
| 内核 ABI | Linux syscall 表为第一公民；BSD/Mach、NT 垫片 | Linux 应用生态最大，syscall 级兼容零开销 |
| 二进制格式 | `.run`（`FUJR`）自研容器，v0.1 本仓库实现 | 自包含、可签名、可预翻译（AOT）、跨架构 |
| 二进制解析 | `object` 式自研轻量解析（fujo-compat，零依赖） | PE/ELF/Mach-O 三格式统一 API |
| 交叉架构翻译 | 自研 dynarec（`fujo-tcg`）+ AOT 预翻译 + 翻译缓存 | 离线翻译 + 运行时 JIT 双路径 |
| 图形 | Vulkan 前向合成器 + DXVK/vkd3d 集成 | 游戏 & 桌面统一管线 |
| 引导 | Multiboot v1 → Long Mode（开发环）；生产换 Limine(UEFI) | Windows 开发机 QEMU 直接可跑 |

## 三、仓库结构

```
FujoOS/
├── docs/                    # 设计与规范（架构 / .run 格式 / 兼容层 / UI / SDK / 路线图 / AI OS 愿景）
├── fujo-compat/             # 二进制识别库：PE / ELF / Mach-O / .run 容器读写（零依赖）
├── fujopack/                # 打包器 CLI：任意格式 -> .run
├── fujorun/                 # 运行器 CLI：解析 / 校验 / 导出 .run
├── kernel/                  # fujo-kernel：x86_64 内核（multiboot -> long mode）
├── sdk/                     # 示例源码与测试样本生成器（hello.c / fixtures）
├── tools/                   # 构建辅助（ELF 扁平化等）
├── scripts/                 # setup / build / qemu 一键脚本
```

> **AI OS 愿景**：我们的立场见 `docs/07-ai-os-vision.md` 与 [Issue #1](https://github.com/FujoJTOP/FujoOS/issues/1)（四件套：模型调用原语 / Agent 一等进程 / 上下文即服务 / 权限与审计）。

## 四、快速开始（Windows 开发机）

```powershell
# 1) 工具链
rustup target add x86_64-unknown-none      # 内核裸机目标
winget install LLVM.LLVM                   # clang/lld（可选：三格式样本编译）

# 2) 构建打包器 / 运行器（零依赖，纯 std）
cargo build --release
python sdk/fixtures/make_fixtures.py       # 生成 ELF/PE/Mach-O 测试样本
target\release\fujopack.exe sdk\fixtures\out\sample_x64.elf -o sdk\fixtures\out\sample.run
target\release\fujorun.exe sdk\fixtures\out\sample.run --dump

# 3) 内核启动验证（QEMU）
powershell scripts\build-kernel.ps1        # 构建 + 扁平化 + QEMU 启动，输出启动日志
```

## 五、路线图摘要（详见 docs/06-roadmap.md）

| 里程碑 | 内容 | 周期 |
|---|---|---|
| M0 | 本仓库：格式规范 + 工具链原型（已完成） | — |
| M1 | 内核 MVP：SMP、分页、IDT/PIT、syscall gate、IPC | 6w |
| M2 | `linuxsubsys` v0：musl/glibc 静态 ELF 直接运行 + `.run` 落地 | 6w |
| M3 | `winsubsys` v0：PE 加载器、ntdll/kernel32 垫片、控制台应用 | 8w |
| M4 | `fujocom` + `fujokit`：组合器、输入、字体、GPU 合成 | 8w |
| M5 | 游戏层：DXVK/vkd3d、XAudio2/XInput、游戏模式 | 10w |
| M6 | `darwinsubsys` v0：Mach-O 加载器、libSystem/objc、Cocoa 最小集 | 10w |
| M7 | `fujo-tcg`：dynarec + AOT 转译器 + 翻译缓存 | 12w |
| M8 | fupm + fujocc + fujo-rs + 镜像工具链 -> v0.1 发布 | 8w |

## 六、合规声明（重要）

兼容层采用 **clean-room** 方式实现：不包含、不链接、不分发 Microsoft / Apple / GNU 的任何
专有二进制或代码；API 行为依据公开文档与逆向工程规范描述（如 NT 系统调用、Mach trap 列表、
GNU ABI 文档）。用户自行承担提供第三方二进制的许可责任。内核与工具链 100% 自有代码。
