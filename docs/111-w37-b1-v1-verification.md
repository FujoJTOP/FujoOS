# 111 · W37 — B-1 重验证 + BOX-BRIDGE v1（大产物/file2pdf/framebuf）+ B-29/30/31

> 里程碑: W37 · 目标: **LEGO 收尾波三连 —— B-1 判定闭合（无 GP 对照实验）、
> B-2 v1 大产物带外（512B→3072B，file2pdf 真实 Windows 盒面 + framebuf 像素
> 流通路版）、B-29 动词词汇表（接口完备性实证）/ B-30 黄金轨迹 / B-31 检疫门 fuzz。**
> 一句话: **17KB 大字源内核内编译无 GP（M-1 已解）；盒产物上限 6 倍扩容并
> 通过 %PDF/BMP 结构检疫；fujoregress 48→51，三重验证工具全绿。**

## 1. B-1 — tcc 内核内大字源重验证（docs/107 M-1 闭合）

**实验（核外源 + 合规 load_end，docs/106 §5 设计）**：
- 17KB 拼装源（sha256tool.c）经 `fujorun pack --lib` 作为 initrd 模块（核外传递，
  不做 include_str）→ VFS `/lib/sha256tool.c`
- 键盘注入 `mbuild /lib/sha256tool.c` → tcc-static（合规 load_end 0x2C0000）
  直读模块文件（绕开 tmpfs 2048B 限制）→ 编译

**结果**：tcc 启动 → mmap 正常 → open 17KB 源 → 逐行编译至源内 150/155 行
（源语法位置）——**全程无 GP**。原场景（W35）4KB 源即 GP at tcc 0x49f630。

**判定**：**M-1 确认 —— 原 GP = load_end 冲突（0x2F0000 使 initrd 顶入
0x400000）污染，非 tcc 内部 bug**（docs/107 更新为 ✅ 已解）。
剩余小项：tcc 0.9.27 无 `__builtin_va_list` 内建（W35 坑 #3 的完整版：
适配层 va 声明需 tcc 兼容写法——源码级微调，非阻断）。

## 2. B-2 v1 — 大产物带外

| 件 | 变更 |
|---|---|
| `kernel/src/boxbridge.rs` | BOX_ARG/BOX_BUF 移 **0xA10000 pin 页**（mem.rs 双 pin：demand_zero_init PT_HEAP1[16] + as_create 派生任务 PT1 同步）；产物上限 **512B→3072B**；DATA 行 **hex 编码**（PDF/BMP 内嵌 `\n` 不再折断行协议，行缓冲 128→256）；动词 **5 file2pdf / 6 framebuf**；检疫门扩 `%PDF-`（头+`%%EOF` 尾子串）/ BMP 结构（魔数+文件大小+数据偏移+DIB 尺寸+bpp）；0x8319 fb_info |
| `tools/box_server.py` | 动词 5 = **winword COM 转 PDF（本机 Office 探活缓存——真实 Windows 盒面！）** 超限回退零依赖微 PDF；动词 6 = BMP 32×24 RGB24；`--mode fuzz`（6 种畸形轮换）；`--golden/--golden-record`（B-30） |
| `sdk/linux/m156_boxv1.c` | v1 断言（hash/file2pdf>512B/framebuf 2358B/台账） |
| `sdk/linux/m157_boxfuzz.c` | B-31 fuzz 断言（6 轮全 -2/-3 + 审计 action=10 ≥6） |

**验证**（新用例，全 PASS）：
```
[48] box-v1-online  BX V1 RESULT: PASS   (pdf len=555 head=%PDF-1.4; fb len=2358 meta=32x24)
[49] box-golden     BX V1 RESULT: PASS   (B-30 黄金轨迹校验, 6 动词 sha256 表)
[50] box-fuzz       BOX FUZZ PASS        (6 轮全拒 + gate-audit>=6)
```

## 3. B-29/30/31 — 三重验证工具

| 任务 | 工具 | 输出 | 结论 |
|---|---|---|---|
| B-29 动词词汇表枚举 | `tools/verb_catalog.py` | `sdk/fixtures/verb_catalog.json` + 表 | **6 动词有限封顶**（schema/上限/用例列全）；3 候选显式声明（file2txt/screen/dbquery）= 声明边界 |
| B-30 盒黄金轨迹 | `box_server --golden-record` + `--golden` | `sdk/fixtures/box_golden.json`（6 动词 sha256） | 宿主自检 6/6 PASS；kernel 侧 = demo 常量镜像（hash 断言） |
| B-31 检疫门 fuzz | `box_server --mode fuzz` + m157 | 6 种畸形（ELF/MZ/非 ascii/超限/坏 PDF/坏 BMP） | 全拒（-2/-3）+ 审计计数 ✓ |

**检疫门 fuzz 覆盖对照**（B-31 有效性）：
| 畸形 | 门 | 结果 |
|---|---|---|
| ELF 魔数 0x7F 'E' 'L' 'F' | exec_magic | -2 |
| MZ 魔数 | exec_magic | -2 |
| 非 ascii 控制符 | ascii_ok | -2 |
| 4096B 超上限（>48 行） | BOX_BUF_MAX | -2 |
| `%PDF-9.9` 无 `%%EOF` | schema（头+尾） | -3 |
| BMP 头违例（尺寸字段错） | schema（结构） | -3 |

## 4. 波内坑（实录）

1. **DATA 行文本协议被产物内嵌 `\n` 折断**：micro_pdf（多行）→ 行错位 → off 不连续
   → 误拒收。→ **hex 编码块**（64B→128 hex；行缓冲 128→256）——顺便二进制安全
   （BMP 体），v0 四态用例天然兼容（hex 解码后原样）。
2. **BMP schema 字段错位**：0x0A 是像素数据偏移（=54），误当文件大小（=2358）
   对比 → 恒 false。修正 = 0x02 文件大小 + 0x0A 偏移双查。
3. **PDF schema 只查头不查尾**：`%PDF-9.9 not a real pdf`（28B ascii）被放行
   （fuzz case4 rc=0 暴露）→ 加 `%%EOF` 子串检查；demo 断言 ends_with 撞尾部 `\n`
   → 改 `"%%EOF\n"`。
4. **BSS 预算**：v1 缓冲外移页 + 代码增长 → BSS 尾 0x2BFBD0（与 v0 持平，
   网回收 640B 抵消增长）——**波级教训：新缓冲先问"能否放 pin 页/带外"，再问 BSS**。
5. winword COM 探活缓存在模块级（每次调用不重探活；转换失败（本机 Office 存在
   但 SaveAs2 路径问题）→ 回退微 PDF 并记录——诚实降级路径与 spec 设计一致）。

## 5. 现状汇总

- fujoregress: **48 → 51**（+box-v1-online/box-golden/box-fuzz）——全量见回归日志
- fujoci: 38/38（BSS 面 m73/m76/m77 复查）
- BSS 尾: 0x2BFBD0 < 0x2C0000 ✓
- 盒面: 6 动词 · 上限 3072B · 检疫门 5 道（ascii/魔数/超限/结构/schema）
- 真实 Windows 盒: 本机 Office 探活 TRUE（COM 路径就位，产物超限/失败回退微 PDF）

## 6. 后续（v2 候选）

- GUI 像素流呈现（B-3 人类窗口版：帧缓冲 → 内核 blit → 窗口）
- 大产物 tmpfs 落盘（/box 目录——需要 tmpfs 单文件 >3072B 或 FJFS 背板，
  前提 = load_end 检查点（docs/106 坑 #5）通过）
- tcc `__builtin_va_list` 适配（fujo_libc.h va 声明 tcc 兼容写法）→ 内编译
  sha256tool 完整闭环（B-1 的"编译到 runfile"最后一步）
- 盒黄金轨迹自动化（B-30 进 fujoregress 常驻 = box-golden 用例已做到）
