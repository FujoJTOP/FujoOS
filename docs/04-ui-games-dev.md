# 04 · UI / 游戏 / 开发者体验设计

> 四大支柱同等优先: UI · 性能 · 游戏 · 开发

## 1. 显示栈

```
应用
 ├─ fujokit (控件库, 渲染即指令)         ──┐
 ├─ Win32 程序 (user32/gdi32 → fujokit) ──┤
 ├─ GTK/Qt (Wayland 协议)                ──┤──> srv_disp (fujocom)
 └─ 游戏 (D3D/GL → Vulkan, 直连交换链)   ──┘       │
                                                   ▼
                                     GPU (Vulkan 前向合成器)
```

### fujocom（合成器）
- Vulkan 前向合成器 + 客户端渲染协议（类似 Wayland 的 wl_shm/zwlr 但带**栅栏同步**接口）。
- 损伤追踪、光标平面、HDR/VRR（自适应刷新）、多 GPU 透明。
- **游戏模式**: 全屏直连交换链（bypass 合成器）、输入线程提权、CPU 亲和限制、VRR 锁定。

### fujokit（控件库）
- Rust, 自定义布局引擎（约束求解, 参考 SwiftUI/Compose 的声明式 + 命令式混合）。
- 矢量绘制统一走 GPU 路径（曲线填充 SDF + 字体 glyph atlas, 参考 egui/zh 生态思路）。
- 设计令牌系统：主题/字体/间距 DSL（`fujo.toml`），明暗模式、无障碍（ARIA 思维映射到桌面）。
- 首套控件: 窗口/菜单/按钮/文本/滚动/树/表格/输入法(文本服务)。

### fujowm
- 平铺 + 浮动混合窗口管理（参考 i3/KWin）, 键盘驱动工作流, snap 布局。

## 2. 游戏兼容（这是胜负手之一）

| 组件 | 来源 | 策略 |
|---|---|---|
| D3D9/11 → Vulkan | DXVK (MIT) | 直接集成, 目标不改一行用户代码 |
| D3D12 → Vulkan | vkd3d-proton (MIT 系) | 同上 |
| OpenGL → Vulkan | Zink (MIT) | Linux GL 应用 |
| Metal → Vulkan | 自研薄层 (M7) | 参考 MoltenVK 语义 |
| MSVC 运行时 | 自实现 vcruntime (M3) | 控制台版先做 |
| **在线服务** | — | Steam/Epic 原生 Linux 客户端已可跑; 反作弊服务端按 M5 评估 |

**游戏性能预算**: 原生 1.0×；垫片路径 ≥0.95×（除 JIT 程序）；DXVK 路径按参考实现水平。
**输入**: 原始 HID 直通 + RawInput/XInput 兼容层；轮询环 <125 Hz → 事件推送模式, 附加延迟 <1 ms。
**音频**: XAudio2/OpenAL 垫片 → srv_aud（低延迟混合 + 空间音频扩展接口）。
**存档**: 沙箱字节集目录 `~/AppData` 自动重定向, 并提供**快照回滚**（M5, 用户可控）。

## 3. 开发者体验（fujo-sdk）

```
语言            C/C++ (fujocc = LLVM/clang, target triple x86_64-unknown-fujo)
               Rust  (fujo-rs = std 移植; cargo --target x86_64-unknown-fujo)
               Python/Node 经 fupm 顺路由 无钉死
构建/包管理     fupm: 解析依赖 -> sandbox 构建 -> 自动 fujopack 出 .run
运行/调试       fujorun (装载决策/翻译缓存/权限) + fujodebug (LLDB 移植, DWARF/PDB)
CI             fujo-ci 模板: build → test-fuso→ package → sign
```

### 一次开发三平台发布
```
fupm build                    # 本地 (原生速)
fupm package -f pe -T x86_64  # 给 Windows 用户的 .exe 容器? —— 不,
                              # .run 才是发布单位:
fupm package --platforms win32,linux,darwin
   -> app.run (内含三套 ABI 资产, 或同一 native 代码)
```

### fujopack/fujorun 已有能力（本仓库, M0 已验证）
- 识别 PE/ELF/Mach-O → `.run`（清单+EMBED+FNV 校验）
- `--dump` / `--validate` / `--extract`
- 后续: `--translate` (AOT 预翻译), `--sign` (Ed25519)

## 4. 无障碍与国际化

- 键盘焦点模型内置于 fujowm；系统级文本服务（输入法 IME 抽象, 中文拼音/仓颉/日语假名）。
- 对比度/字体缩放/屏幕阅读器 API（M4 追加 UI 无障碍规范附录）。

## 5. 性能仪表

- `fujostat`: CPU/内存/IO/翻译缓存命中率。
- 启动跟踪: bootchart 视角——因为我们要 <2s 桌面。
