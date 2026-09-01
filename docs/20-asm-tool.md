# 20 — 系统内汇编器 (M71, 最小 .s 子集)

状态: ✅ 完成。验收: QEMU 串口 `M71 RESULT: PASS`, demo `sdk/linux/m71_asm.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7001 | asm_assemble(src, n, dst, cap) | 两遍汇编 → 字节数 (负=err) |
| 0x7002 | asm_verify(ptr, n) | 解码统计指令数 (遇 ret 停) |

## 2. 指令子集

| 指令 | 编码 |
|------|------|
| nop / ret / int3 | 90 / C3 / CC |
| syscall | 0F 05 |
| mov r64, imm64 | 48 B8+r [imm64 8B] |
| mov r64, r64 | 48 89 /r |
| add/sub/xor/cmp r64, imm8 | 48 83 /d imm8 (d=0/5/6/7) |
| add/sub/xor/cmp r64, r64 | 48 01/29/31/39 /r |
| inc/dec r64 | 48 FF C0+r / C8+r |
| push/pop r64 | 50+r / 58+r |
| jmp rel32 | E9 [rel32] |
| je/jne rel32 | 0F 84/85 [rel32] |

- 寄存器: rax rcx rdx rbx rsp rbp rsi rdi;
- 立即: `0x..` / 十进制 / `$` 前缀; 负数 `-`;
- 伪指令: `.text` `.byte n` `.word n` `.quad n`;
- 注释: `#` 或 `;` 至行尾;
- label: `L0:` .. `L15:` (跳转目标同名字), 两遍 (pass1 记录地址)。

## 3. 两遍结构

```
pass1: 逐行 → 去注释/逗号 → label 识别 (L#:) → 指令长度总汇
       → label_off[#] 记录 (label-only 行也要注册 — 已修)
pass2: 逐行重放 → emit 到 (dst) → skip 同 label;
       jcc 目标 = LABEL_AT[#] - (pc + 指令长) rel32
```

## 4. 实测 (m71_asm.elf)

程序: `.text; nop; mov rax, 0x42; xor rcx, rcx; add rcx, 3; je L0; L0:; inc rcx; ret`

```
asm  : assembled 28 bytes
m71: n=0000001c inst=00000007
m71: b0=00000090 b2=000000b8 b9=00000000 b18=0000000f b19=00000084
     b20=00000000 b24=00000048 b25=000000ff b27=000000c3
m71: M71 RESULT: PASS
```

布局: nop(1) mov(10: 48 B8 42 00×8) xor(3) add(4) je(6: 0F 84 00×4,
L0 紧跟 → rel=0) inc(3) ret(1) = 28B; verify 解码 7 条。

## 5. 踩坑记录

- 分词后操作数带逗号 (`rax,`) → 未匹配 reg_of (修: strip 尾逗号);
- label-only 行在 `continue` 前未注册 label → 跳转 rel 恒 0 或不解析
  (修: insn_n==0 分支先 flush);
- jcc 字节序: 第一版写成 `84 0F ...` (修: 0F 84);
- mov imm64 的 imm 从第 3 字节起 (48 B8 imm@2..9 → 实测校验 b3);
- cap 检查 `cap<1024` 拒绝 demo 256B 缓冲 (放宽 64)。

## 6. 后续

- M72+ 可扩展: 更多寻址 (mem), call/ret 链接, 单遍递归下降;
  M73 断开由此汇编器生成的小内核代码 (自举工具链面)。
