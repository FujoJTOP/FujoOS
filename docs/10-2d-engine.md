# M58 · 2D 引擎选择与最小验证 (FujoOS 原生)

## 候选比较

| 引擎 | 可行性 | 理由 |
|---|---|---|
| SDL2/3 | ✗ (短期) | 依赖 POSIX 用户库与 OS 服务面; syscall 面差距大 |
| LÖVE/Godot 导出 | ✗ | 需要 GL 基线+脚本 VM; 超当前面 |
| **fujogl v0 + fujokit (自研)** | ✓ | 原语面已覆盖 2D: rect/tri/line 光栅 + sprite 图标 + 窗口/输入 |
| 移植 Pygame 类 | 中期 | 先补 python VM (M74 编译器面) |

## 结论

v0 2D 引擎 = **fujogl 光栅 + fujokit 控件 + XInput 输入** (SDK 内闭环,
零外部依赖), 游戏经 fujopack/fujorun 打包分发。后续可视需增加
sprite 缓存原语 (M61 硬件 blit)。

## 验证: Pong (m58_pong)

开放引擎最小游戏: 球(矩形 12x12) 跑动 + 拍(矩形) 移动, 60 帧循环,
记录球轨迹坐标 → 采样验证运动/回弹 → PASS。
