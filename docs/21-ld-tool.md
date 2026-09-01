# 21 — 系统内链接器 (M72, ELF64 静态最小)

状态: ✅ 完成。验收: QEMU 串口 `M72 RESULT: PASS`, demo `sdk/linux/m72_ld.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7101 | ld_link(cfg) | cfg 表 → ELF64 静态输出; 返回字节数 |
| 0x7102 | ld_info() | 最后一次 link 长度 |

## 2. cfg 布局 (全部 u64)

```
[0] dst     输出缓冲        [1] text1    [2] n1
[3] text2   [4] n2         [5] syms     [6] nsyms
[7] relocs  [8] nrelocs

syms:    [name:32B][vma:8B] × nsyms      (绝对 VMA)
relocs:  [place:8B (输出偏移)][symidx:8B] × nrelocs
```

## 3. 输出

- ELF64: e_ident class64/LE, e_type=ET_EXEC, e_machine=x86-64,
  e_entry=0x400000, e_phoff=64, e_phnum=1;
- PT_LOAD @0x40: flags=7 (RWX), vaddr/paddr=0x400000, filesz=memsz
  = off2+n2, align=0x1000;
- text1 @ off1=0x8000; text2 @ off2=align16(off1+n1);
- 重定位: dst[place..] ← base + sym_vma (u64 LE)。

## 4. 实测 (m72_ld.elf)

```
ld   : linked 32785 bytes (elf64 static)
m72: total=00008011 reloc=00000040
m72: M72 RESULT: PASS
```

- text1=[90 C3] (nop ret), text2=[CC] (int3);
- 符号 foo vma=0x100 → 绝对 0x400100;
- reloc place=0x8003 → 字节 00 01 40 00... (0x400100);
- 校验: magic `7F 45 4C 46` / e_type=2 / e_entry=0x400000 /
  p_flags=7 / 段数据 / reloc 值全过。

## 5. BSS/pad 事件

M71+M72 后内核 BSS 尾 **0x2801F0 超出 pad 0x280000** → 升
`--pad 0x1A0000`, MB_HEADER load_end/bss_end 0x002A_0000
(build-kernel.ps1 同步)。教训: 每里程碑 BSS 检查仍是铁律。

## 6. 与 M71 的衔接 / 后续

- M71 (as → 字节) + M72 (字节 + 符号/重定位 → ELF) 构成系统内
  "as+ld" 最小链; M74 fujocc 把 C 翻译面接上; M85 一键构建验收。
