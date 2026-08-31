# 02 · FUJR (.run) 可执行格式规范 v0.1

> 状态: **已实现**（fujo-compat::run, fujopack, fujorun, 端到端验证通过）
> 文档地址: 本文件 + fujo-compat/src/run.rs（代码即规范, 二者必须同步修改）

## 1. 定位

`.run` 是 FujoOS 的**通用可执行容器**：无论二进制源自 PE、ELF 还是 Mach-O，装入 `.run`
后在所有 FujoOS 上运行。容器是"自包含"的：清单 + 原始二进制（或预翻译代码）+ 资源 + 签名。

```
原始二进制 ──fujopack──>  .run ──fujorun──> 执行
   PE        (打包器)      FUJR    (运行器/加载器)
   ELF                     容器
   Mach-O
```

## 2. 磁盘布局（小端）

```
+------------------+  offset 0
| Header  (64 B)   |
+------------------+  offset 64
| Section table    |  32 B × n
+------------------+  offset 64 + 32n
| Section 0 .. n-1 |  每个 section 默认 4 KiB 对齐
+------------------+
```

## 3. Header 字段

| off | size | 字段 | 说明 |
|---|---|---|---|
| 0 | 4 | magic | `"FUJR"` |
| 4 | 2 | version_major | 0 |
| 6 | 2 | version_minor | 1 |
| 8 | 4 | section_count | n |
| 12 | 8 | header_size | 64 + 32n |
| 20 | 8 | total_size | 文件总长 |
| 28 | 16 | uid | 构建随机标识 |
| 44 | 4 | flags | bit0: 已签名锁定 |
| 48 | 4 | manifest_index | 清单所在 section 下标 |
| 52 | 2 | target_arch | 1=x86_64 2=aarch64 3=i386 4=arm |
| 54 | 2 | base_arch | EMBED 原始格式的架构 |
| 56 | 4 | source_format | 1=PE 2=ELF 3=Mach-O 0=raw |
| 60 | 4 | reserved | 0 |

## 4. Section entry（32 B）

| off | size | 字段 |
|---|---|---|
| 0 | 4 | tag: 1=MANIFEST 2=CODE 3=IR 4=EMBED 5=DATA 6=SIGN 7=ICON |
| 4 | 4 | flags |
| 8 | 8 | offset |
| 16 | 8 | size |
| 24 | 4 | hash: FNV-1a-32（内容完整性） |
| 28 | 4 | reserved |

## 5. MANIFEST（v1, JSON）

```json
{
  "manifest": "fujo.os.run/v1",
  "name": "hello",
  "source": { "format": "elf", "arch": "x86_64", "bits": 64,
              "entry": "0x201220", "pie": false },
  "target": { "arch": "x86_64", "abi": "fujo" },
  "exec": "embed",
  "api": { "subsystems": ["linux"], "shim_modules": [] },
  "libs": [],
  "env": {},
  "signature": { "alg": "none", "note": "M8: ed25519" }
}
```

- `source` 由 fujopack 自动识别填充（PE/ELF/Mach-O + 架构 + 入口 + PIE 标志）。
- `exec` 执行路径选择: `embed`（运行 EMBED 内原始二进制，M2+ 生效）
  → `ir`（运行 IR 节, 跨架构） → `native`（运行预翻译 CODE 节, 冷启动最快）。
- `api.subsystems` 由导入符号诊断填充（M2/M3），驱动 fujo-sandbox 的能力授予。

## 6. 转译流水线（"一次打包，到处运行"）

```
阶段 A (M0, 已实现):  识别 + 封装
    输入 ──inspect──> 清单+EMBED ──write_run──> .run (embed)

阶段 B (M2/M3):  装载决策
    fujorun: source.format -> linuxsubsys/winsubsys/darwinsubsys 装载器

阶段 C (M7):  预翻译 (AOT)
    fujopack --translate:
       二进制 → fujo-tcg(离线) → CODE 节(native 代码) + IR 节(便携表示)
       运行时首启: native 路径, 无 JIT 开销; 换架构: 回退 IR JIT
       翻译缓存: 按 (elf-hash, 版本) 键控, 存放在 srv_fs 共享缓存
```

## 7. 版本与兼容

- magic 不变的前提下，字段只增不改；新字段见 `header_size` 扩展（子版本 0.2）。
- 加载器必须拒绝 `version_major != 0`（未来 1.x 走迁移器）。
- 签名节（M8, Ed25519）：数字签名覆盖 header+table+全部内容；签名后 `flags.bit0` 置位。

## 8. 附录：二进制定位

```
0000  46 55 4A 52 00 01 02 00 02 00 00 00 40 00 00 00  FUJR............
0010  00 20 00 00 00 00 00 00 C3 44 0D 92 6B C1 2C 1F  .....D..k.,.
0020  ...
```
