# 01 · FujoOS 系统架构

> 版本: 0.1 (M0)
> 决策原则: 应用程序为中心；兼容性是内核的一等能力而非补丁；

## 1. 总体结构（四层）

```
+--------------------------------------------------------------+
| 应用层           .run 容器 (FUJR)                            |
|   原生 FujoOS 程序 │ Windows 二进制 │ Linux 二进制 │ macOS 二进制 |
+--------------------------------------------------------------+
| 兼容层 (用户态)   winsubsys / linuxsubsys / darwinsubsys      |
|   PE/ELF/Mach-O 加载器 · API 垫片 (kernel32/ntdll, libc,      |
|   Foundation) · Mach trap 垫片 · IR 解释器/预翻译             |
+--------------------------------------------------------------+
| 系统服务 (Ring3)  srv_fs · srv_net · srv_disp(fujocom) ·     |
|                   srv_aud · srv_input · srv_pkg · srv_font    |
+--------------------------------------------------------------+
| 内核 (Ring0)      sched · vmm · ipc · syscall gate ·         |
|                   drv framework (Ring3, IOMMU 直通)           |
+--------------------------------------------------------------+
| 硬件             x86_64 (M0) → arm64 (M7)                  |
+--------------------------------------------------------------+
```

## 2. 内核设计（混合内核）

**选型**: 混合内核 —— Ring0 只含 CPU 相关与空间管理；设备驱动、VFS、网络栈全部用户态服务化（服务进程 + 共享内存 + IRQ fd 硬中断注入）。这是 NT / WSL2 式工程折中：绝大多数"与硬件无关"的代码处于可崩溃、可重启、可热升级的 Ring3 环境。

| 内核组件 | M0 状态 | M1 目标 |
|---|---|---|
| 启动引导 | Multiboot v1 → Long Mode（恒等 1 GiB） | Limine/UEFI + 高半区 + 5 级页表 |
| 线程调度 | 单 CPU idle | EEVDF 公平调度 + 多核 SMP + NUMA 感知 |
| 分页 | 恒等映射（大页） | 4 级页 + 独立地址空间 + 写时复制 |
| 系统调用 | 表数据 + 分发占位 | MSR LSTAR syscall gate + 内核栈切换 |
| IPC | — | 消息端口 + 零拷贝共享内存 + 能力句柄 |
| 驱动 | — | Ring3 驱动框架（中断经 IOMMU/IRQ fd 注入） |

**调度器**: 参考 Linux EEVDF + NT 优先级混合：虚拟时间公平性保证交互延迟，配合"游戏模式"动态提频调度。
**内存**: 4 KiB 页 + 2 MiB 大页；物理内存以 2 MiB 伙伴系统管理；用户空间共享内存经 **fujo-map** 零拷贝成对映射（GPU 栅栏同步）。

## 3. Syscall Gate（本项目创新点之核心）

三套 ABI 共享**同一内核服务面**（POSIX 风格服务 + FujoOS 扩展），差别只在编号映射：

```
程序 ──syscall──> 内核 [abi 选择器: linux / darwin / fujo-native]
                         │
                         ├─ Linux x86_64: 原始 syscall 号 → 服务表（第一公民, 零垫片）
                         ├─ Darwin x86_64: BSD 空间(0x2000000|nr) + Mach trap 空间
                         └─ FujoOS 原生: fujo syscall 号（扩展指令集/栅栏/能力）
```

- Linux 二进制**不需要**用户态垫片——glibc 直接 syscall，原生速度。
- Windows 走 `ntdll` 垫片（其 syscall 编号不透明），kernel32/ws2_32 等 shim DLL 再调 FujoOS 服务。
- macOS 走 BSD 表 + `mach_msg` 垫片（我们的 IPC 端口本身就是幂等 mach port 语义）。

## 4. 进程模型与隔离

- 每个进程是「地址空间 + 线程组 + 能力句柄表」，统一底座，三种 compat 进程只是 ABI 标签不同。
- `.run` 默认运行在 **fujo-sandbox** 内：以 manifest 声明的 API/能力为准（`api.subsystems`、文件系统字名空间、网络白名单）。此即"打包即权限"。
- 信号语义按 ABI 归一：Linux 信号 → 内核统一信号模型；Win32 异常（SEH）→ 由垫片注册表映射统一模型。

## 5. 存储与文件

- 内核只负责块设备访问（经 Ring3 驱动）；文件语义全部由 `srv_fs` 服务实现。
- 服务端驱动表: `fujo-fs`（原生）、`ext4`（Linux 兼容读取）、`ntfs`（只读 M0 计划）、`exfat/fat`（共享分区）、`hfs+/apfs-ro`（M6 只读）。
- 所有服务可注入检查点（M5 后），为「翻译缓存」与「进程迁移」打底。

## 6. 安全模型

1. 内核无引用计数不信任任何线程指针：所有用户地址经 `copy_from_user/strnlen_user` 边界校验。
2. 能力句柄：文件、端口、GPU 资源全部为句柄 + 权限掩码（类似 NT handle，但默认为最小权限）。
3. `fork/exec` 中间态不泄漏挂起句柄（PIE 安全）。
4. 签名：`.run` Ed25519（M8），可配置"仅运行已签名"。
5. 三平台垫片全部 clean-room，不包含任何微软/苹果代码。

## 7. 性能预算（设计指标）

| 指标 | 目标 | 验证方式 |
|---|---|---|
| 冷启动到桌面 | < 2 s | QEMU/KVM 计时 |
| syscall 往返 | < 800 ns | bench (M1) |
| 组合器提交延迟 | < 10 ms | fujocom bench |
| 输入到屏幕 | < 1 ms 附加 | 游戏模式 |
| dynarec 翻译开销 | 2–4× 原生 | spec 基准 (M7) |
| .run 冷启动 | < 350 ms | fujorun bench |
