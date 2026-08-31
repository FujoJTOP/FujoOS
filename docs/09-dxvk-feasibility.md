# M56 · DXVK 式翻译可行性评估(方案+原型)

> 目标: 评估把 **D3D(9/11) 类命令流翻译到 FujoOS 原生图形层(fujogl)** 的
> 可行性, 给出分层方案与缺口清单, 并交付一个最小翻译原型。

## 1. 现状盘点 (FujoOS M55)

| 层 | 已有 | 缺失 |
|---|---|---|
| 光栅 | fujogl v0: clear/rect/三角形(重心法)/line, backbuffer, 软件 | 深度/纹理/雾/混合/着色器 |
| 显示 | Bochs VBE 1024x768x32 (软件双缓冲@0xC00000) | virtio-gpu 硬件加速(M61 路线) |
| 输入 | XInput 抽象(M53)、鼠标/键(M36)、窗口(M37/M38) | 无手柄硬件 |
| 音频 | AC97 播放入口(M52) | 混音器(M63) |
| 算力 | 单核 64 位内核, 无 SIMD 用户库 | 多核(M64+) |

## 2. DXVK 架构回顾 (参考)

DXVK = D3D11->Vulkan 命令翻译器 (用户态):
`D3D11 app -> dxvk (命令翻译) -> Vulkan driver`
关键点: **翻译器不含光栅**——把高层语义降为底层 API 调用,
底层(驱动)做细节。FujoOS 对应: `D3D 子集 -> dxwrap -> fujogl 命令`。

## 3. FujoOS 可行性结论

**中等可行性(分阶段)**:
- 可行: 固定功能管线子集 (FFP): 顶点变换矩阵 / 三角形列表 /
  颜色填充 / 视口 —— fujogl v0 已具备 50%。
- 缺口(路线): 深度缓冲(软件 z-buffer)、纹理采样(双线性)、
  混合(alpha)、光照(数学生成)、着色器(compute 子集 M62)。
- 硬件加速: virtio-gpu (M51 已探测, M61 接入 blit/缩放; 着色器/
  compute 走 M62 评估)。

## 4. 分层方案 (原型即第 1 层)

```
D3D 子集 app
  -> layer0: dxwrap (命令缓冲/顶点缓冲/矩阵状态)   [M56 原型: 已交付]
  -> layer1: fujogl 光栅 (clear/rect/tri/line)      [M55 交付]
  -> layer2: 显示合成 (backbuffer -> VBE/virtio)    [M51 抽象 + M47 分辨率]
```
每个 layer 有**独立系统调用面**(0x63xx / 0x62xx / 0x5Cxx),
对齐 DXVK 的"翻译层不直接碰硬件"边界。

## 5. 原型论证 (m56_dxwrap)

顶点缓冲(3 顶点)+ 仿射矩阵(sx,sy / tx,ty 2x2) → 翻译 →
fujogl 三角形光栅 → 像素采样验证（变换前后坐标符合 (x*sx+tx)）。
**结论: 命令翻译的最小闭环 (D3D 状态 → fujogl) 在软件路径成立。**

## 6. 后续里程碑对接

- M61 图形加速: blit/缩放硬件路径 (VBE→virtio-gpu)
- M62 着色器内核评估(compute 子集)
- M63 音频混音器/效果链 (播放原语已就绪)
- M69 2D 游戏#2 + 输入延迟基准 (XInput 已就绪)
