# 34 — 工具链验收 (M85: hello/gui/game 一键构建运行)

状态: ✅ 完成。`onebuild: 3/3 PASS (hello/gui/game build+run)`。

## 1. scripts/onebuild.ps1

```
pwsh scripts/onebuild.ps1 [-BuildOnly] [-Kernel kernel/fujo-kernel.bin]

步骤:
  1) build: sdk/templates/{hello,game,gui}.tpl.c
     → clang (x86_64-unknown-linux-gnu, no-libc, user.ld -e _start)
     → sdk/build/one/{X}.elf
  2) pack: fujopack pack → {X}.run (--name X --type app)
  3) run: QEMU 256M → initrd X.elf → sendkey os run hermes →
     日志断言 (per-template needle) → 报告 3/3 或 n/3
```

## 2. 验收 (实测)

```
onebuild: hello.tpl / game.tpl / gui.tpl 构建+打包 ok
:: hello.tpl ... PASS   (hello: FujoOS template app)
:: game.tpl  ... PASS   (game: template frame loop)
:: gui.tpl   ... PASS   (gui: template (fujokit skeleton))
onebuild: 3/3 PASS
```

## 3. 工具链闭环 (M71-M85)

| 层 | 工具 | Milestone |
|----|------|-----------|
| as | 系统内汇编器 (2-pass, 子集) | M71 |
| ld | 系统内链接器 (ELF64 + 符号/重定位) | M72 |
| cc | fujocc 表驱动编译壳 (C 子集全链) | M74 |
| 运行时 | 分解器/追踪/计数/泄漏/转储 | M75-77/82-84 |
| 验收 | fujoci 25 用例 + onebuild 3/3 | M78/85 |
