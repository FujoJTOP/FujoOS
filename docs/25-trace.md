# 25 — syscall trace 工具化 (M76)

状态: ✅ 完成。验收: QEMU 串口 `M76 RESULT: PASS`, demo `sdk/linux/m76_trace.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7701 | trace_bg(on) | 后台记录 (M33 的 0x5301 前台开关保留) |
| 0x7702 | trace_stats(ptr) | u64×4: (total, nonzero_nr, ring_pos, dropped) |
| 0x7703 | trace_filter(nr) | 0=全记录, 否则仅该 nr |

## 2. 实现

```
dispatch 头:
  rec = TRACE_ON || TRACE_BG
  if rec && (TRACE_FILTER==0 || TRACE_FILTER==nr):
      TRACE_COUNTS[nr%256]++ / TRACE_RING[pos%64]=(nr,a0,ticks)
      TRACE_TOTAL++ / deleted 环形覆盖计数 (pos 过 64 后)
```

## 3. 实测 (m76_trace.elf)

```
m76: syscall trace toolkit v0
m76: t0=00000002 t1=00000004 d_filter=00000003
m76: M76 RESULT: PASS
```

- 后台开启: 初始化调用自身即入账 (t0=2: trace_bg + filter);
- 写 sample + 读 stats → t1=4;
- filter(1)=仅 write → 3 次 wr → 差分恰 3 (对 0x7702 自身不记,
  过滤正确)。

## 4. 集成

- 诊断链: trace_stats 的非零 nr 映射函数名表 (LINUX_X64_SUBSET)
  即"syscall 热点面"; 与 M68 计数器互补: 计数值 + 调用轨迹。
