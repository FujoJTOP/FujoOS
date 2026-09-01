# 12 — 音频混音器/效果链 (M63, v0)

状态: ✅ 完成。验收: QEMU 串口 `M63 RESULT: PASS`, demo `sdk/linux/m63_mix.c`。

## 1. 接口

| 编号  | 签名 | 说明 |
|-------|------|------|
| 0x5F05 | mix_open(ch) | 重置通道 (len/k/y/gain) |
| 0x5F06 | mix_push(ch, ptr, n) | 追加样本 (i16 mono, n≤128) |
| 0x5F07 | mix_render(ptr, n, gain) | 混音全部通道 → i16 缓冲 (n≤256) |
| 0x5F08 | mix_effect(ch, kind, p) | kind 1=低通 k(0..256), 2=增益(0..256) |
| 0x5F09 | mix_status(ptr) | 写 (NCH, len0..3 打包) |

## 2. 效果链 (每通道, 采样级)

```
x ──→ [单极低通] y += k/256*(x-y) ──→ [增益] g/256 ──→ [混音累加] ──→ [饱和 i16]
```

- 低通: k=256 直通 (y 瞬变到 x); k=192 → 0.75 系数, 一阶收敛。
- 混音: 通道求和 (i64, saturating), 再乘全局 gain (0..256), clamp [-32768,32767]。

## 3. 实测 (m63_mix.elf)

| 步骤 | 输入 | 输出 | 预期 |
|------|------|------|------|
| 混音 gain=256 | ch0 64×10000, ch1 64×5000, ch2 32×4000 | mix0=0x4A38 (19000) | 10000+5000+4000 |
| 同上 | 同上 | mix40=0x3A98 (15000) | 10000+5000 (ch2 结束) |
| 低通 k=192, 仅 ch0=10000 | y0 | lp0=0x1D4C (7500) | 192*10000/256 |
| 低通收敛 | i=7 | lp7=0x270F (9999) | 10000-0.25⁸·10000 |
| 增益 50% (k 直通) | ch0=10000 | gain=0x1388 (5000) | 10000*128/256 |

## 4. 与 M52/AC97 的关系

- M52 的 `audio_enable/volume/playback` 保留 (硬件前门);
  M63 在软件侧建立**混音总线** —— 多路流 → 单路 PCM, 效果链可插拔。
- 硬件 FIFO 写 (AC97 BDL/BUFFER) 由后续里程碑接 `mix_render` 输出
  (路径: 效果链 → 混音缓冲 → DMA 描述符 → BUF0/BUF1 交替)。

## 5. 验证

```
python tools/flatten_elf.py kernel/target/x86_64-unknown-none/release/fujo-kernel \
    kernel/fujo-kernel.bin --pad 0x180000
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin \
    -initrd sdk/linux/m63_mix.elf -serial file:log \
    -monitor telnet:127.0.0.1:4568,server,nowait -display none -no-reboot
# monitor: sendkey o s spc r u n spc h e r m e s ret
```

串口尾: `m63: M63 RESULT: PASS`。
