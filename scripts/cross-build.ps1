# cross-build.ps1 — M81: 交叉编译一键脚本 (三源: ELF/Mach-O/PE32+)
#
# 用法: pwsh scripts/cross-build.ps1 [-Src sdk/hello.c] [-Out sdk/build/cross]
#   [-Mac sdk/mac/user_darwin.c] [-Win sdk/win/hello_win.c] [-Kernel kernel/fujo-kernel.bin]
# 默认三源 = SDK 示例。输出: app.elf / app.macho / app.exe (+ kernel.bin 若与
# 内核同目录构建外)。要求: clang/llvm-dlltool (LLVM) + qemu 可选。
param(
    [string]$Src = 'sdk/hello.c',
    [string]$Out = 'sdk/build/cross',
    [string]$Mac = 'sdk/mac/user_darwin.c',
    [string]$Win = 'sdk/win/hello_win.c'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$llvm = 'C:\Program Files\LLVM\bin'
New-Item -ItemType Directory -Force -Path (Join-Path $root $Out) | Out-Null

$fails = 0
Write-Host "== [1/3] ELF (linuxsubsys, ring3) =="
& "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static `
    -fno-pie -no-pie -fuse-ld=lld -fno-builtin "-Wl,-e,_start" `
    "-Wl,-T,$root\sdk\user\user.ld" "$root\$Src" -o "$root\$Out\app.elf"
if ($LASTEXITCODE -eq 0) { Write-Host "  ok: $Out\app.elf" } else { $fails += 1 }

Write-Host "== [2/3] Mach-O (darwinsubsys) =="
& "$llvm\clang.exe" --target=x86_64-apple-macos11 -O2 -nostdlib -fuse-ld=lld `
    "$root\$Mac" -o "$root\$Out\app.macho"
if ($LASTEXITCODE -eq 0) { Write-Host "  ok: $Out\app.macho" } else { $fails += 1 }

Write-Host "== [3/3] PE32+ (winsubsys, kernel32 垫片) =="
& "$llvm\llvm-dlltool.exe" -d "$root\sdk\win\kernel32.def" `
    -l "$root\sdk\win\kernel32.lib" -D kernel32.dll
& "$llvm\clang.exe" --target=x86_64-pc-windows-msvc -O2 -nostdlib -fuse-ld=lld `
    "-Wl,/entry:_start" "-Wl,/subsystem:console" "-Wl,/base:0x400000" `
    "$root\$Win" "$root\sdk\win\kernel32.lib" -o "$root\$Out\app.exe"
if ($LASTEXITCODE -eq 0) { Write-Host "  ok: $Out\app.exe" } else { $fails += 1 }

Write-Host "== 结果 =="
if ($fails -gt 0) {
    Write-Host "cross-build: $fails FAILED"
    exit 1
}
Write-Host "cross-build: 3/3 PASS ($Out)"
exit 0
