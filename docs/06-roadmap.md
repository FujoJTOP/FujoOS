# 06 · 路线图（M0 → v0.1）

## 里程碑总览

| 里程碑 | 周期 | 内容 | 验收标准 |
|---|---|---|---|
| **M0 奠基** | ✅ 已完成 | 格式规范、fujopack/fujorun、识别库、可启动内核、SDK 样本 | 本仓库: 三格式→.run 端到端 + QEMU 启动日志 |
| **M1 内核芯** | ✅ 已完成 | IDT(15 异常+IRQ0)、PIC/PIT 100Hz、GDT+用户段+TSS、syscall gate(LSTAR/STAR/SFMASK, Linux ABI)、ring3 用户态 iretq 进入、syscall write/exit 直通 | QEMU: PIT ticks=102；ring3 程序 syscall write 打印 + exit 内核接管（提交 cfb0a52 + M1 完成提交） |
| **M2 linuxsubsys v0** | ✅ 已完成 | 内核 ELF64 装载器(ET_EXEC, PT_LOAD 段复制+BSS 清零, 模块=QEMU -initrd)、Linux syscall 原生执行(write/exit/getpid)、syscall 命名日志、64MiB 恒等页表(全链 U=1) | QEMU: multiboot 模块交付 ELF → 内核解析装载 → ring3 运行 ELF 内 Linux syscall → exit 内核接管（提交见 M2） |
| M2 linuxsubsys v0 | 6w | ELF 动态加载(含动态链接)、Linux syscall gate 全表、musl/glibc 直跑 | busybox/dash/curl-static 原生运行 |
| M3 winsubsys v0 | 8w | PE 加载器、ntdll/kernel32/ws2_32 垫片、控制台程序、vcruntime 子集 | mingw hello + 控制台 TUI 应用 |
| M4 桌面 | 8w | fujocom 合成器 v0、fujokit v0、fujowm、输入/字体/IME | 桌面鼠标键盘窗口流畅, 120Hz |
| M5 游戏层 | 10w | DXVK/vkd3d 集成、XAudio2/XInput、游戏模式、沙箱存档 | 跑通 2 个开源 D3D11 游戏 |
| M6 darwinsubsys v0 | 10w | Mach-O 装载、libSystem/objc 最小集、Cocoa 薄层、apfs-ro | 终端+轻量 GUI 应用 |
| M7 交叉架构 | 12w | fujo-tcg dynarec、AOT 预翻译、翻译缓存、fujopack --translate | arm64 机跑 x86_64 基准 2–4× |
| M8 发布 | 8w | fupm、fujocc/fujo-rs、镜像工具、Ed25519 签名、fuji 文档 | v0.1 安装镜像 + 公开演示 |

## M0 已交付清单（对照）

- [x] `docs/01..06` 设计文档
- [x] FUJR v0.1 规范 + `fujo-compat`（PE/ELF/Mach-O 识别、容器读写、校验）
- [x] `fujopack` / `fujorun`（依赖零第三方, 纯 std）
- [x] `fujo-kernel`（Multiboot v1 → 长模式, 自建 GDT/页表, 串口/VGA, mmap 解析）—— QEMU 实机启动
- [x] 测试资产: fixtures(三格式) + clang 三平台同源样本 + rustc 裸机样本
- [x] `scripts`: setup / build-kernel / pack-demo 一键化

## 风险与对策

| 风险 | 等级 | 对策 |
|---|---|---|
| macOS GUI 兼容复杂度极高 | 高 | 分阶段: CLI/console 先行; GUI 用薄层子集; 重应用走 hypervisor 桥接 |
| Win32 面巨大 | 中 | 表驱动 API 清单, 按使用频率排序; 只做"用到的"子树 |
| Dynarec 性能 | 中 | AOT 预翻译优先; 参考 box64 的 SSE→NEON 表; 悲观估计 2–4× |
| 内核开发测试 | — | QEMU 无头启动 + 启动日志断言已固化; 后续 CI 镜像 |
| 法律 | — | clean-room 已写入 README; 不链接/不分发第三方专有二进制 |

## 贡献入口

1. 读 `docs/01` 和 `docs/02`（架构/格式）
2. `cargo test` + `scripts/pack-demo.ps1` 跑通
3. 从 M1 列表选题: syscall gate / VGA→framebuffer / IDT / PIT
4. 任何提议: 先补 `specs/*.tbl` 或 issue 带验收标准
