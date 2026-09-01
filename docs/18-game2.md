# 18 — 2D 游戏#2 + 输入延迟基准 (M69)

状态: ✅ 完成。验收: QEMU 串口 `M69 RESULT: PASS`, demo `sdk/linux/m69_game2.c`。

## 1. 游戏: Breakout v0

| 元素 | 原语 | 尺寸 |
|------|------|------|
| 球 | 0x6801 blit (16x16 圆形 pattern, M61) | 16×16 |
| 拍 | 0x6202 gl_rect (color 打包) | 20×60 |
| 砖块带 | 碰撞判定 (非渲染) | y∈[40,160] |

帧循环 (10 帧):
```
t0 = timer_us            ← 输入采样点
模拟输入: px = bx-10      (拍的期望位置; QEMU 无指针时自打)
物理: bx/by += 14px × (vx,vy), 四边反弹, 砖块命中 vy 翻转 + hits
渲染: blit(球) + rect(拍)
t1 = timer_us            ← 渲染完成
game2_latency(t1-t0)      (输入→渲染延迟)
frame_wait(20ms)
```

## 2. 输入延迟基准 (内核仪表 game2.rs)

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x6F01 | game2_latency(us) | 延迟累计 (N/SUM/MAX) |
| 0x6F02 | game2_stats(ptr) | u64×4: (n, avg_us, max_us, hits) |
| 0x6F03 | game2_hits(v) | 命中上报 |

## 3. 实测

```
m69: breakout v0 + input-latency bench
timer: calibrated cyc/us=3046
m69: frames=0000000a avg_lat=0000005e max_lat=000002cd hits=00000001
m69: M69 RESULT: PASS
```

- 帧数 10, 平均输入→渲染 94µs, 最坏 717µs (TCG 软渲染闭包);
- 1 次砖块命中 (球上行穿过砖块带);
- 该基准值进入 M70 游戏层性能验收报告。

## 4. 说明

- 帧表/延迟基准与 M68 工具联动 (0x6E01..04);
- 输入面: 键盘/鼠标事件 → 状态 → 渲染的完整管线延迟在无硬件指针
  下以"采样点→渲染完成"通道时序近似; 真机输入延迟 (键/鼠事件到
  LFB 刷新) 由 M70 给出的外测量面承接。
