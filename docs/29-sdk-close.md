# 29 — SDK 文档闭环 (M80: 示例/模板/教程)

状态: ✅ 完成。SDK 示例索引 + 3 个脚手架模板 + 教程。

## 1. 源码布局

| 路径 | 内容 |
|------|------|
| sdk/hello.c | Hello (linux ABI, 最小 _start) |
| sdk/user/ | M1-M25 内核原语样例 (alloc/thread/fs/ipc/kobj/crash/fork…) |
| sdk/linux/ | M30-M77 里程碑样例 (linuxsubsys + fujo 原语) |
| sdk/win/ + sdk/mac/ | Win32 (kernel32 垫片) / Mach-O 样例 |
| sdk/kit/fujokit.h | 窗口/控件/输入 kit |
| sdk/ai/agent.c, sdk/hermes/hermes.c | 模型调用链 |

## 2. 模板 (sdk/templates/)

- `hello.tpl.c` — 最小可编译入口 (sys_write → exit);
- `game.tpl.c` — 帧循环 (timer + blit/gl 原语, 输入采样);
- `gui.tpl.c` — fujokit 窗口/按钮/列表。

## 3. 构建 (三格式)

```
# ELF (linuxsubsys / fujo 原语)
clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static \
  -fno-pie -no-pie -fuse-ld=lld -fno-builtin \
  "-Wl,-e,_start" "-Wl,-T,sdk/user/user.ld" app.c -o app.elf

# PE32+ (winsubsys) —— 见 scripts/build-kernel.ps1 [0c]
# Mach-O (darwinsubsys) —— [0b]

# 打包 (.run) / 多模块 (BootMulti)
python tools/fujopack.py pack -e app.elf -o app.run --name myapp
python tools/fujorun.py run -k kernel/fujo-kernel.bin -i app.run --keys ...
```

## 4. 教程 (快速开始)

1. **Hello**: 拷贝 `sdk/templates/hello.tpl.c` → 改字符串 →
   上面 ELF 构建 → `fujorun run` → 串口日志见输出。
2. **游戏**: `game.tpl.c` 帧循环 60fps (frame_wait), 用
   `0x6801 blit / 0x6202 rect / 0x6101 us` 原语; `0x6F01` 报输入延迟。
3. **GUI**: `gui.tpl.c` 用 fujokit (kt_button/textbox/list) +
   `0x55xx wm` 窗口表。
4. **多文件**: fujorun `--lib` 附加库模块 (BootMulti)。

## 5. 验证链

```
python tools/fujoregress.py   # 兼容矩阵 9/9
python tools/ci.py            # CI 25/25 (含全部 m 样例断言)
```
