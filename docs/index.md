# FujoOS — 官网 (docs/index.md)

> 一个零依赖的 x86_64 原生操作系统内核 + AI OS 独有层。
> 内核、驱动、窗口系统、游戏层、工具链、AI 服务全栈自研。

## 特性亮点

| 层 | 能力 |
|----|------|
| 内核 | x86_64 长模式 · IDT/GDT/TSS · PIT 100Hz · 抢占式多任务 (PIT round-robin, 亲和性/均衡统计) |
| 内存 | 虚拟内存 v0 · 按需零页 · 帧分配器 · U-位硬化 · 权重 mmap 按需页 · 每任务页表链 + CR3 · >1GiB 恒等映射 |
| ABI | ELF64 / Mach-O / PE32+ 加载器子集 (静态样例原地运行) · Linux x86-64 39 syscalls · darwin/win32 shim 家族 · ABI v1 · .run (FUJR) |
| 存储 | ATA PIO + FJFS 4MiB 卷 (格式化/持久化) · AHCI/SATA 真盘 (W20) · 页缓存/预读 · 存档沙箱 8×8KiB |
| 网络 | virtio-net legacy · IPv4/UDP 往返 (ARP 应答) · 最小 TCP 服务器 SYN/ACK/PSH echo · UDP 克隆闭环 (W21) |
| 图形 | VBE 1024x768x32 + LFB · 5x7 位图字体 · 软件光栅 (rect/tri/line) · blit/scale · 着色器 VM |
| 输入 | PS/2 键盘 IRQ1 · 鼠标 IRQ12 (8042 序列/命中测试) · XInput · IME |
| 音频 | AC97 探测/音量 · 4ch 混音器 + LPF/增益链 |
| 窗口 | wmsg 窗口类表/64 消息队/刷新 · fujokit (button/textbox/list) |
| 游戏 | 游戏模式 (前台调度/全屏) · Pong/Breakout 自研引擎 · 输入延迟基准 avg 94µs |
| AI OS | 权重 mmap · 模型卡 (权限/计费/审计) · 会话检查点 · fujoctx / 上下文压缩 · 能力表+审计 · 意图路由 · 推理执行器 · 模型注册表 (fupm) · AI For Next: 事件环/权限域/五职责 · W22–W28: 三引擎对照/蒸馏闭环/对抗验证/IO 基线/自监督/事件哨兵 (m141–m147) |
| 平台 | W20 真机化 (平台检测/GRUB2/AHCI/PCI 多功能) · W29 双执行模式对照 (TCG / WHPX, `--accel whpx`) |
| 工具链 | 系统内汇编器 → 链接器 → fujocc 编译壳 (C 子集→ELF64 全链) · 调试器 (TF 单步/int3 断点) · trace/性能窗口/单位测试/泄漏/转储 · tcc 自托管编译 (W16) |
| 交付 | fujopack/fujorun · fujoci 38 用例 · onebuild 3/3 · live 镜像+安装器 · 签名/更新 |

## 运行

```
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin \
    -initrd sdk/linux/m30_linux.elf \
    -monitor telnet:127.0.0.1:4568,server,nowait -display none -no-reboot
# monitor: sendkey o s spc r u n spc h e r m e s ret
```

## 回归

```
python tools/fujoregress.py                # 全量 37/37 (TCG)
python tools/fujoregress.py --accel whpx   # 第二执行模式 36/37 (W29, m129=WHPX 平台限制)
```

## 文档

- **官网**: docs/index.html (单文件官方站点, GitHub Pages /docs)
- docs/08-roadmap-100.md — 100 里程碑路线图 (勾选状态)
- docs/51-project-status.md — 项目现状总览 (权威快照) · docs/57-long-roadmap.md — 长期路线图
- docs/58-handoff.md — 接手文档 (新对话从这里开始)
- docs/09-dxvk-feasibility.md / docs/10-2d-engine.md — 可行性分析
- docs/11..48 — 每里程碑技术文档 (接口/实现/实测/踩坑)
- docs/49-release-notes.md — v1.0 发布公告 · docs/61..92 — W10–W29 逐波文档 (含 W29 平台对照 docs/92)
- docs/89-w28-ai-vertical-summary.md — AI 垂直六波总结 · docs/90-ask2.md — 六波后自评
- sdk/templates/ — hello/game/gui 模板; docs/29-sdk-close.md 教程
