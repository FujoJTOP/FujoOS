# 30 — 交叉编译一键脚本 (M81, win/mac/linux 三源)

状态: ✅ 完成。`pwsh scripts/cross-build.ps1` → 3/3 PASS。

## 1. 用法

```
pwsh scripts/cross-build.ps1 [-Src sdk/hello.c] [-Mac sdk/mac/user_darwin.c]
      [-Win sdk/win/hello_win.c] [-Out sdk/build/cross]
```

## 2. 三个目标

| 目标 | clang 参数 | 链接脚本/库 | 加载器 |
|------|-----------|-------------|--------|
| ELF | `--target=x86_64-unknown-linux-gnu -nostdlib -static -fno-pie -fuse-ld=lld` | `sdk/user/user.ld` (-e _start) | linuxsubsys (M2) |
| Mach-O | `--target=x86_64-apple-macos11 -nostdlib -fuse-ld=lld` | 默认 | darwinsubsys (M6) |
| PE32+ | `--target=x86_64-pc-windows-msvc -nostdlib` | `kernel32.def → lib` /entry /subsystem /base:0x400000 | winsubsys (M3) |

## 3. 实测

```
== [1/3] ELF    →  ok: sdk/build/cross\app.elf
== [2/3] Mach-O →  ok: sdk/build/cross\app.macho
== [3/3] PE32+  →  ok: sdk/build/cross\app.exe
cross-build: 3/3 PASS
```

## 4. 依赖

- LLVM 15+ (clang/ld.lld/llvm-dlltool), 默认
  `C:\Program Files\LLVM\bin` (脚本可改 $llvm 变量);
- 三源默认取 SDK 示例 (可 -Src/-Mac/-Win 覆盖);
- 运行验证: 三个产物分别作为 QEMU initrd (fujoci/qemu-kvm 路径)。
