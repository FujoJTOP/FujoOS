# 11 — 着色器内核评估 (M62, compute 子集 v0)

状态: ✅ 完成 (内核 `kernel/src/shader.rs`, demo `sdk/linux/m62_shader.elf`,
提交见日志)。验收: QEMU 串口 `M62 RESULT: PASS`。

## 1. 目标

着色器本质是"**每像素/每线程并行执行的程序**"。CPU 软渲染下先建立
**内核即程序** 的最小模型, 通过该模型评估:

- 把程序作为数据 (`load` → `run`) 的载入/执行架构是否成立;
- 解释执行 bytecode 的指令吞吐 (为后续 SIMD/多核/GPU 通道给出基线);
- 像素地址模型 (线性 idx = y*FBW + x) 是否与 framebuffer 布局吻合。

## 2. 接口 (fujo 原生 syscall, 编号 0x69xx)

| 编号  | 签名                        | 说明 |
|-------|-----------------------------|------|
| 0x6901 | `shader_load(ptr, n)`       | 载入字节码 (≤32 字 = 128B, 内生 TCB) |
| 0x6902 | `shader_run(x, y, w, h)`    | 对区域逐像素执行内核 → BACKBUFFER |
| 0x6903 | `shader_pixel(x, y)`        | 读回像素 (验证面) |
| 0x6904 | `shader_ops()`              | 累计执行指令数 (性能面) |

## 3. 字节码 v0 (栈机无栈, 8 个 u32 寄存器)

每条指令 = 一个 u32:
`op<<24 | r<<16 | a<<8 | b`

| op | 语义 | 说明 |
|----|------|------|
| 0  | halt | 结束本像素 |
| 1  | const r, v | r = b (v 为 0..255 立即) |
| 2  | add r,a,b | r = a + b |
| 3  | mul r,a,b | r = a * b |
| 4  | sub r,a,b | r = a - b |
| 5  | color r,a,b | r = (regs[a]&0xFF) \| ((b&0xFF)<<8) |
| 6  | idx | r0 = y*FBW + x (重载) |

每像素执行序:
```
r0 = y*FBW + x            // 索引加载
[指令流...]               // 直至 halt
r1 → 像素处 BACKBUFFER     // 输出寄存器 r1
```

## 4. 内核示例 (m62_shader.elf 内嵌 7 条)

```
const r4=0xFF | const r5=0 | const r6=1
r3 = r0 + r5   ; idx
r7 = r3 + r4   ; idx+255
r7 = r7 * r6   ; ×1
r1 = (r3&0xFF) | (0xFF<<8)   ; color
```

对 16x16 区域执行 → 每像素输出 `(idx&0xFF) | 0xFF00`:
- (0,0) → `0000ff00`  (idx=0)
- (1,0) → `0000ff01`  (idx=1)
- (5,0) → `0000ff05`  (idx=5)
- (15,15) → `0000ff0f` (idx=15*1024+15=15375, &0xFF=0x0F; FBW=1024)

## 5. 性能/架构评估

- 256 像素 × 8 轮 (7 指令 + halt 轮) = 2048 轮, `ops=0x800` 实证,
  与模型一致 → VM 解释循环正确、无超标路径。
- 单像素成本评分: 每像素 ~8 次携分支解释循环, 相对直接 C 算术约
  **10x 开销量级** (每指令 ≥1 次取指 + match)。
- 结论:
  1. **数据驱动内核成立** —— 载入-执行分离, 与真实着色器 API 形态一致;
  2. CPU 解释执行只适合调试/原型, 生产通路需
     a. **批量通道** (SIMD 128-bit 一次 4 像素, op 直接编译成
        x86 序列而非解释, 即 JIT 化);
     b. **多核扇出** (M64 调度亲和: each core 一个 tile);
     c. 或 **GPU 位** (图形设备可用后, VM 保持, 后端替换)。
- 与本路线关系: 该 VM 即 M69 2D 游戏#2 的自定义特效原语;
  M64+ 在统一接口下填并行后端。

## 6. 验证

```
python tools/flatten_elf.py kernel/target/x86_64-unknown-none/release/fujo-kernel \
    kernel/fujo-kernel.bin --pad 0x180000
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin \
    -initrd sdk/linux/m62_shader.elf -serial file:log \
    -monitor telnet:127.0.0.1:4568,server,nowait -display none -no-reboot
# monitor: sendkey o s spc r u n spc h e r m e s ret
```

串口尾:

```
m62: shader compute-kernel subset v0
m62: p00=0000ff00 p10=0000ff01 p50=0000ff05 p1515=0000ff0f
m62: ops=00000800
m62: M62 RESULT: PASS
```

## 7. BSS / 布局

- shader 模块新增静态: PROG[32]u32(128B) + 2 个标量 → BSS 尾
  `0x27ABC8`, pad 0x180000 (load_end 0x280000) 内, 仍有余量。
