# 23 — fujocc 编译壳 (M74, 表驱动/跨 ABI 选项)

状态: ✅ 完成。验收: QEMU 串口 `M74 RESULT: PASS`, demo `sdk/linux/m74_cc.c`。

## 1. 链路 (全链)

```
C 子集文本 ──表驱动翻译──> fujo-asm 文本 (M71 语法)
        ──asm_assemble──> 字节码
        ──ld_link(cfg)──> ELF64 静态 (M72)
```

## 2. 表

- KEYWORD_T: int / return / main / void;
- OP_T (v0): 常量 (hex/dec) → `mov rax, IMM64` + `ret`;
- ABI_T: linux(0x01) / mac(0x02) / win(0x04) → 输出参数选项面
  (v0: 同构 ELF, abi 表校验入口)。

## 3. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7501 | cc_compile(src, n, dst, cap, abi) | 全链编译 → 字节数 |
| 0x7502 | cc_version() | 表版本 = 1 |

## 4. 实测 (m74_cc.elf)

```
src: int main() { return 0x41; }   (abi=linux)
asm  : assembled 11 bytes          (mov rax, 0x41 + ret)
ld   : linked 32784 bytes (elf64 static)
m74: total=00008010 b0=00000048 b2=00000041 b10=000000c3
m74: M74 RESULT: PASS
```

字节: `48 B8 41 00×8 C3` @0x8000, ELF 头/段/入口校验通过。

## 5. 踩坑记录

- `write_fmt` 的分段 write_str 调用会从 out[0] 覆盖 → 改为游标式
  AsmOut (pos 递增);
- 常量 token 判定顺序: `0x..` 必须早于纯十进制分支 (否则 "0x41"
  被截成 '0' → imm=0);
- ld 无第二段时 total = off2 = 0x8010 (对齐), demo 期望值修正。

## 6. 与 M85 的关系

- M85 (工具链验收): hello/gui/game 一键构建 = M71 asm + M72 ld +
  M74 cc 壳的整合命令行; 本里程碑提供最小单函数路径。
