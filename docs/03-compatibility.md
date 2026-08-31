# 03 · 三平台兼容层设计（Peer→Fujo 翻译）

> 状态: 设计定稿；M0 已交付识别+封装（fujo-compat / fujopack / fujorun）
> 原则: **clean-room**（零微软/苹果/GNU 专有代码）+ 声明式映射（表驱动, 少写 C）

## 1. 兼容矩阵（诚实版）

| 目标 | 装载 | 执行速度 | 交付 | 备注 |
|---|---|---|---|---|
| Linux CLI/静态 ELF | ELF 加载器 + Linux syscall gate | **原生**（零垫片） | M2 | 第一公民: 内核直接实现 Linux ABI |
| Linux GUI (GTK/Qt) | 同上 + Wayland/XWayland 服务 | 原生+合成器开销 | M4 | X11 经 fujo-xwayland(a) |
| Windows CLI (.exe 无 GUI) | PE 加载器 + ntdll 垫片 | 原生+垫片 | M3 | shim DLL 是 FujoOS 自己的实现 |
| Windows GUI/游戏 | 同上 + GDI/DirectX→Vulkan | 原生+垫片+翻译 | M5 | D3D9/11 经 DXVK, D3D12 经 vkd3d-proton |
| macOS CLI (无 GUI) | Mach-O 加载器 + BSD/mach trap 垫片 | 原生+垫片 | M6 | libSystem 自实现 |
| macOS GUI/AppKit | 同上 + Cocoa 垫片（fujokit 后端） | 原生+垫片 | M6+ | 部分 AppKit 子集起步 |
| 跨架构（如 x86 程序跑 arm64 FujoOS） | dynarec (fujo-tcg) | 2–4× | M7 | AOT 预翻译进 CODE 节 |
| 内核驱动 .sys/.ko/.kext | — | — | 不支持 | 提供 hypervisor/虚拟机逃逸方案(桌面版) |

## 2. 加载器统一接口

```
trait Loader {
  auth()  -> 校验头/PE/ELF/Mach-O (fujo-compat)
  relocate(base) -> 重定位 (PE 基址重定位/ELF 重定位/Mach-O dyld fixups)
  resolve(imports) -> 绑定垫片库或真实库
  map_segments -> 建立地址空间
  entry_info -> 入口/栈/环境
}
```

## 3. Linux ABI（第一公民, 最快路径）

- 内核 syscall gate 接受 **原始 Linux x86_64 编号**（表见 fujo-compat::abi + kernel::syscall）。
- VDSO: 提供 vtimer 函数（M2）；glibc 版本符号（GLIBC_2.x）宽松匹配到我们的 ABI 即可。
- 架构不动、语义不变 → **Linux 二进制零插桩**，这是兼容最优解；`.run` 仅做封装。
- 游戏/GPU 通常走 DRM + EGL → 内核实现 DRM/KMS ioctl shim（M5）或直接接 fujocom 的
  Wayland 协议（GL 转 Vulkan: Zink）。

## 4. Windows ABI（winsubsys）

- **装载**: PE 加载器完成节映射、基址重定位（`.reloc`）、导入表绑定、TLS 初始化、
  SEH 表由 pad 的 ntdll 装入 TEB（垫片即"内核"）。
- **垫片图书馆**（FujoOS 自己写, 签名模块）:

| shim DLL | 提供语义 | 落地 |
|---|---|---|
| ntdll | 进程/内存/对象内核接口 | M3 |
| kernel32/kernelbase | 文件/线程/注册表(@srv_fs) | M3 |
| user32/gdi32 | 窗口/消息/绘制（接 fujokit） | M4 |
| ws2_32 | winsock（接 srv_net, 兼容 fd 复用） | M3 |
| dxgi/d3d9/d3d11/d3d12 | 接 Vulkan（DXVK/vkd3d 为底座, MIT/zlib） | M5 |
| xaudio2/winmm | 接 srv_aud | M5 |
| xinput | 接 srv_input (RawInput/XInput 兼容) | M5 |

- **关键差异**: 我们把 Windows 二进制当作"APICall 语言"程序来理解——比 Wine 更进一步的是:
  垫片 DLL 直接调 FujoOS 原生服务，而不是再套一层 POSIX。

## 5. Darwin ABI（darwinsubsys）

- Mach-O 装载: 段映射 + dyld fixups + LC_MAIN/LC_UNIXTHREAD 入口。
- **Mach trap 垫片**: mach_msg（把我们的 IPC 端口当作 mach 端口语义）、
  mach_vm_*（vmm）、task/thread 端口（调度器）。
- **BSD 空间**: 0x2000000|nr 映射到 FujoOS 服务表。
- **系统库自实现**: libSystem（自用 libc+私有符号区）、objc4 移植（我们的 fujocc 编译）、
  Foundation/CoreFoundation 子集。
- **Cocoa**: AppKit 薄层接到 fujokit——NSApplication/NSView/NSWindow/NSMenu 最小集；
  绘制经 CoreGraphics 语义直接走 GPU 矢量路径（fujocom 2D 层）。
- **诚实声明**: 完全 GUI 等价极难；对重度 AppKit 应用, M6 之后提供 hypervisor 兜底
  （x86 上跑 macOS guest 做"桥接窗口"模式）。

## 6. 交叉架构: fujo-tcg（M7）

- 设计参照 Box64/QEMU-TCG/Rosetta 的组合: **分块基本块翻译器** + 虚拟机寄存器栈。
- 支持: x86_64↔aarch64（SSE4/AVX2 ↔ NEON/SVE 映射表驱动）, 旧 32 位路径优先 x86→x64。
- 三层缓存: 内存 JIT 块 → 进程级翻译缓存 → **AOT 预翻译**写进 `.run` 的 CODE 节。
- 指标: 翻译器启动 < 30 ms、常驻开销 < 20 MB、SPEC 整型 2–4×（对照原生）。

## 7. 翻译质量守则

1. 一切 API 行为由公开文档/规范/逆向的**事实描述**实现，不引用专有头文件中的实现细节之外
   的代码；关键 api-set 以表（registry 目录 `specs/*.tbl`）驱动生成 Rust 源。
2. 每层都有"行为归因"：垫片错误与内核错误可区分（errno 分域）。
3. 灰盒测试: 用公开测试集（LTP 子集 / mingw smoke / 你的自有程序）逐里程碑固化回归。
