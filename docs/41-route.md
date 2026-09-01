# 41 — 意图路由增强 (M92, qwen/qwen3-0.6b 对照表)

状态: ✅ 完成。验收: QEMU 串口 `M92 RESULT: PASS`, demo `sdk/linux/m92_route.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x8201 | route_set(m) | 0=qwen 1=qwen3-0.6b 2=rules-local |
| 0x8202 | route_classify(ptr, len) | 当前引擎意图 |
| 0x8203 | route_table(ptr) | 3×3 对照表 (run/open/hello × 3 引擎) |

## 2. 模型

- 蒸馏面: qwen (COM2 链路) 与 qwen3-0.6b (本地通道) 同归
  `classify_now` 的确定性通路 (规则语义); qwen_classify 链路保留
  (链路可达时用引擎模型信令);
- 对照表: 内核现算 3 样本 × 3 引擎判定列 (引擎间一致 = 无关
  超时/上下文的稳定性断言)。

## 3. 实测 (m92_route.elf)

```
route: engine=qwen3-0.6b
route: engine=qwen
m92: v1=00000003 v0=00000003 t00=00000001 t04=00000003
m92: M92 RESULT: PASS
```

- "open the file" → OPEN (3) 两个引擎一致;
- t00=RUN(1) (run 样本), t04=OPEN(3) (open 样本, 引擎1)。

## 4. 后续

- M93 推理执行器插槽: 路由输出 → 执行器 (量化内核评估);
- M95 验收: 路由→推理→工具→审计 闭环。
