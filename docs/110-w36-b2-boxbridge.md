# 110 · W36 — BOX-BRIDGE v0：闭源 = 接线（LEGO 工程收尾波）

> 里程碑: W36 · 目标: **B-2 v0 通路验证 —— 内核外盒供应商（BOX）接进 FujoOS，
> 内核不装载、不翻译、只接线**。
> 一句话: **4 动词（hash/info/size/echo）经 COM2 行协议 + 共享页请求帧全链跑通：
> 盒在线 PASS / 盒缺席=声明 PASS / 坏产物=检疫拒收 PASS / 违约=降权 PASS ——
> 四态回归全绿，LEGO + BOX-BRIDGE + EXCLUDED 三轴补齐。**
> 理论: docs/108（拆/接/出界） · 规格: docs/109（五件 + 六边界 + v0.1 弱点闭合 + 决策记录）

## 1. 交付

| 部件 | 说明 |
|---|---|
| `kernel/src/boxbridge.rs` | 盒面：per-provider 注册表（up/hit/total/schema）+ 动词白名单（1..=4）+ 请求帧（共享页 kind=0xB0）+ COM2 行协议（RSP/DATA/END）+ **产物检疫门**（ascii 白名单 + 禁 ELF/MZ 魔数 + 动词 schema 列2a）+ 双列台账（duty7 履约/duty8 谓词）+ 域门（act7=BOX_CMD）+ TTL=8s 缺席声明 |
| `kernel/src/capability.rs` | `ALL_ACTS` 0x3F→**0x7F**（act7=BOX_CMD）；`exec_authorized` 上限 6→8；`aud_note` pub(crate)（盒审计入统一环：action=9 调用 / 10 产物检疫） |
| `kernel/src/ai.rs` | `QUAL_DUTY` 6→**8**（duty 7/8 = 盒双列，与模型同尺度；0x8314/0x8315 自动包含） |
| `kernel/src/syscall.rs` | **0x8316 box_run(verb, arg, len)** / **0x8317 box_stat** / **0x8318 box_result** |
| `tools/box_server.py` | 宿主盒（Adapter）：COM2 + monitor pmemsave；4 动词 FOSS 流程；modes = normal/badart/adapter（`--mode`） |
| `sdk/linux/m154_box.c` | 四态断言 demo（结果驱动，无需知道模式） |
| `tools/fujoregress.py` | +4 用例（box-online/offline/badart/adapter），44→**48** |
| BSS 回收 | -8.2KB（CLIP 8K→2K / editor 2K→1K / TRACE_COUNTS u64→u32 / wmsg QLEN 64→56）→ 尾 0x2BFBD0 < 0x2C0000 |

## 2. 验证（fujoregress 四态 + 全量，全部 PASS）

```
[44] box-online  ELF64 x box-online   PASS   BX V0 RESULT: PASS
[45] box-offline ELF64 x box-offline  PASS   BOX OFFLINE PASS
[46] box-badart  ELF64 x box-badart   PASS   BOX GATE PASS
[47] box-adapter ELF64 x box-adapter  PASS   BOX ADAPTER PASS
```

全量: **fujoregress 48/48**（m149-trust 断言随 ALL_ACTS 0x3F→0x7F 同步，
W36 语义演进——全权集含 act7 BOX_CMD）+ **fujoci 38/38**（m73-edit/m76-trace/
m77-win 三个 BSS 回收敏感面 + m91-cap/m92-route 权限面）。

正常模式链（demo 节选）:
```
m154: T1 provider domain=1 (act7 BOX_CMD)
box  : hash=d4a997afdb0442de74a37dd7cad5eed9822b2f3084f605fd6d172d00d28f1520
box  : info='ASCII text'
box  : size=18
box  : echo='fujobox-v0 payload'
m154: T4 ledger up=1 hit=4 total=4 schema=4
m154: BX V0 RESULT: PASS
```

## 3. 设计落点对照（spec → 实现，偏差见 docs/109 §13）

| 设计 | 落地 |
|---|---|
| 盒 = 内核外供应商，不装载 | ✅ boxbridge.rs 无任何盒代码/装载路径；协议=行 + 帧 |
| S1 动作+产物双门 | ✅ act7 域门（cap_exec 同构）+ 检疫门（ASCII/魔数/schema，A1'） |
| S2 双列台账 | ✅ duty7 履约（传输成功）/ duty8 谓词（机器可判定 schema）；与模型同入 0x8314 族、域宽 f(质量) 同消费（dom_admit） |
| S3 政策 = 动词清单 | ✅ 白名单 1..=4 内核常量 + 未知动词 -22 |
| 缺席 = 声明非错误 | ✅ TTL 超时 → PROV_UP=0 + 审计 + 返回 -4（demo 明确打印 OFFLINE，系统其余功能不受影响） |
| LEGO > BOX | ✅ 路由决策在调用方（demo/未来）；内核只提供 BOX 面，原生路径不动 |
| per-provider 域（D4） | ✅ 域 1 = provider 0 端口域（demo 建域绑域，perm=0x40） |

## 4. 坑（本波实录）

1. **QEMU monitor 单连接冲突**：box_server 的 pmemsave（读请求帧）与 fujoregress 的
   sendkey（键盘注入）抢同一 `-monitor telnet` 连接；后者被踢 → 注入失效。
   → 盒用例改 **autostart 直启**（`fujo.run=m154_box`，m148 同路径），monitor 独占给盒。
2. **Python bytes 格式化坑**：`b"pmemsave %s" % str` → TypeError（%s 需 bytes）。
   → f-string 构造命令再 encode。
3. **DATA 行产物含空格**（`ASCII text` / echo payload）：token 解析会把文本截到
   第一个空格 → 按"token4 到行尾"解析（boxbridge.rs: tok 定位 + 行尾切片）。
4. **BSS 对齐漂移**：新增模块文本增长 → bss 起点页对齐上移 → 尾一次性 +0x3030
   越过 0x2C0000。→ 回收 8.2KB（CLIP/editor/TRACE_COUNTS/wmsg QLEN），
   **教训 = 每波先量 bss 尾，超过预算先回收再写新件**（铁律 2 的波级版本）。
5. **`box` 是 Rust 关键字**：模块名不能用 → `boxbridge.rs`（syscall 分发用
   crate::boxbridge::）。

## 5. 后续（v1 候选，规格 docs/109 §8）

- **大产物**：BOXXFR 块流经 tmpfs/带外（产物 >512B；file2pdf/file2txt 动词；
  需先过 load_end 检查——docs/106 坑 #5）
- **真实 Windows 盒**：winword→PDF（Adapter 内容，v0 只验证了"内核外供应商通路"）
- **GUI 像素流**（B-3 帧缓冲搬移，远程桌面式；人类窗口 = "可用不原生"）
- **B-29 动词词汇表枚举**（接口完备性实证）/ B-30 黄金轨迹回放 / B-31 检疫门 fuzz
- **共享页 0xA10000 pin**（高带宽盒通道：当前请求帧复用模型窗、产物走 COM2 行）

## 6. 与家族的关系（论文二素材）

盒 = 外部智能体家族第二成员：**模型产文本，盒子产行为**；同一 S1（域门+检疫门）
× S2（双列台账入 0x8314 族）× S3（动词清单）信封；LEGO 是家族的"本地化路径"。
FUFORALL 三轴：拆得开的拆（LEGO）· 分不开的接（BOX-BRIDGE）· 接不得的出界
（EXCLUDED，有名有姓）——"使用"意义下的完备性声明（docs/108 §0）。
