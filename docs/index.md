# FujoOS — 官网 (docs/index.md)

> 一个零依赖的 x86_64 原生操作系统内核 + AI OS 独有层。
> 内核、驱动、窗口系统、游戏层、工具链、AI 服务全栈自研。

## 特性亮点

| 层 | 能力 |
|----|------|
| 内核 | x86_64 长模式 · IDT/GDT/TSS · PIT 100Hz · 抢占式多任务 (PIT round-robin, 亲和性/均衡统计) |
| 内存 | 虚拟内存 v0 · 按需零页 · 帧分配器 · U-位硬化 · 权重 mmap 按需页 |
| ABI | ELF64 / Mach-O / PE32+ 加载器子集 (静态样例原地运行) · Linux x86-64 39 syscalls · darwin/win32 shim 家族 |
| 存储 | ATA PIO + FJFS 4MiB 卷 (格式化/持久化) · 页缓存/预读 · 存档沙箱 8×8KiB |
| 图形 | VBE 1024x768x32 + LFB · 5x7 位图字体 · 软件光栅 (rect/tri/line) · blit/scale · 着色器 VM |
| 输入 | PS/2 键盘 IRQ1 · 鼠标 IRQ12 (8042 序列/命中测试) · XInput · IME |
| 音频 | AC97 探测/音量 · 4ch 混音器 + LPF/增益链 |
| 窗口 | wmsg 窗口类表/64 消息队/刷新 · fujokit (button/textbox/list) |
| 游戏 | 游戏模式 (前台调度/全屏) · Pong/Breakout 自研引擎 · 输入延迟基准 |
| AI OS | 权重 mmap · 模型卡 (权限/计费/审计) · 会话检查点 · fujoctx / 上下文压缩 · 能力表+审计 · 意图路由 · 推理执行器 · 模型注册表 (fupm) |
| 工具链 | 系统内汇编器 → 链接器 → fujocc 编译壳 (C 子集→ELF64 全链) · 调试器 (TF 单步/int3 断点) · trace/性能窗口/单位测试/泄漏/转储 |
| 交付 | fujopack/fujorun · fujoci 38 用例 · onebuild · live 镜像+安装器 · 签名/更新 |

## 运行

```
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin \
    -initrd sdk/linux/m30_linux.elf \
    -monitor telnet:127.0.0.1:4568,server,nowait -display none -no-reboot
# monitor: sendkey o s spc r u n spc h e r m e s ret
```

## 文档

- docs/08-roadmap-100.md — 100 里程碑路线图 (勾选状态)
- docs/09-dxvk-feasibility.md / docs/10-2d-engine.md — 可行性分析
- docs/11..48 — 每里程碑技术文档 (接口/实现/实测/踩坑)
- sdk/templates/ — hello/game/gui 模板; docs/29-sdk-close.md 教程
