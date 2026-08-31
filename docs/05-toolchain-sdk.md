# 05 · 工具链与 SDK 参考

## 1. 仓库产物（M0）

| 产物 | 说明 |
|---|---|
| `fujo-compat` | 库: PE/ELF/Mach-O 识别 + FUJR 容器读写（零依赖, 纯 std） |
| `fujopack` | CLI: 任意格式 → `.run`（inspect → manifest → container） |
| `fujorun` | CLI: 校验 / 转储 / 导出 /（未来）执行 |
| `fujo-kernel` | x86_64 内核（multiboot→long mode）, QEMU 可启动 |
| `sdk/hello.c` | 三平台同源样本（Linux 侧为纯 syscall 二进制） |

## 2. 用法速查

```powershell
# 打包
fujopack app.exe -o app.run --name app          # 自动识别 PE/ELF/Mach-O
fujopack blob.bin --raw -o blob.run             # 不解析, 直接封装
fujopack --dump app.run                         # 查看容器

# 运行/校验
fujorun app.run --validate
fujorun app.run --dump
fujorun app.run --extract dir
fujorun app.run --run-embed                     # M2/M3 起: 装载并执行

# 内核开发
powershell scripts/build-kernel.ps1             # 桩生成 -> 构建 -> 扁平化 -> QEMU
```

## 3. manifest 字段字典

| 字段 | 类型 | 说明 |
|---|---|---|
| name | string | 应用名 |
| source.format | `pe|elf|macho|raw` | 原始格式（fujopack 自动） |
| source.arch / bits / entry / pie | — | 识别结果 |
| target.arch / abi | — | 目标机（x86_64: fujo） |
| exec | `embed|ir|native` | 执行路径；M7 后 `--translate` 产出 `native` |
| api.subsystems | [str] | `linux` / `win32` / `darwin` |
| api.shim_modules | [str] | 需要的垫片库（M3 自动填充） |
| libs | [{name, embedded}] | 捆绑共享库 |
| env | map | 运行环境覆盖 |
| signature | {alg, key...} | M8: ed25519 |

## 4. 三平台样本编译（开发机）

```bash
# ELF64 (linux syscall ABI, 零 libc)
clang --target=x86_64-unknown-linux-gnu -nostdlib -static -fno-pie -no-pie \
      -fuse-ld=lld -Wl,-e,_start sdk/hello.c -o sdk/build/hello.elf

# PE32+ (待 M3 装载器, 现仅验证容器链路)
clang --target=x86_64-pc-windows-msvc -nostdlib -fuse-ld=lld \
      -Wl,/entry:_start -Wl,/subsystem:console sdk/hello.c -o sdk/build/hello.exe

# Mach-O 64 (待 M6 装载器)
clang --target=x86_64-apple-macos11 -nostdlib -fuse-ld=lld \
      -Wl,-e,_start sdk/hello.c -o sdk/build/hello.macho

# 无 clang 时的真实工具链回归 (rustc 自带 rust-lld)
rustc --target x86_64-unknown-none -C linker=rust-lld \
      -C link-arg=-T sdk/fujo.ld -C panic=abort sdk/hello-fujo.rs \
      -o sdk/build/hello-fujo.elf
```

## 5. 兼容表维护（声明式映射）

- `fujo-compat/src/abi.rs` —— Linux x86_64 表（第一公民）、Darwin BSD 表、Win32 shim 模块表。
- `kernel/src/syscall.rs` —— 内核内嵌子集表 + dispatch 骨架, 与 fujo-compat 同步演进。
- M1 起: `tools/gen_syscall_tbl.py` 从规范文件再生成（避免手工漂移）。

## 6. 质量闸门

- `cargo test`（fujo-compat: 三种格式+容器往返, 不变量: 打包→校验 恒通过）
- `scripts/pack-demo.ps1` 端到端冒烟（fixtures + clang 真实编译）
- `scripts/build-kernel.ps1` QEMU 启动日志断言（`FujoOS` + `ready`）
