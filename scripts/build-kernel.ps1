# FujoOS build-kernel.ps1 — 构建用户测试程序 -> 内核 -> 扁平化 -> QEMU 启动验证
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location "$root\kernel"

$llvm = 'C:\Program Files\LLVM\bin'
Write-Host "== [0/4] build M1 user test (ring3, linux-x64 syscalls) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\user_test.c -o ..\sdk\user\user_test.elf
    python ..\tools\flatten_elf.py ..\sdk\user\user_test.elf src\user_test.bin
} else {
    Write-Host "  clang missing: cannot build user test (kernel build will fail on include_bytes)"
    exit 1
}

Write-Host "== [0b] build M6 darwin sample (Mach-O, darwin bsd syscalls) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-apple-macos11 -O2 -nostdlib -fuse-ld=lld `
        ..\sdk\mac\user_darwin.c -o ..\sdk\mac\user_darwin.macho
} else {
    Write-Host "  clang missing: darwin sample skipped"
}

Write-Host "== [0c] build M3 win sample (PE32+ w/ kernel32 import table) =="
if (Test-Path "$llvm\llvm-dlltool.exe") {
    & "$llvm\llvm-dlltool.exe" -d ..\sdk\win\kernel32.def -l ..\sdk\win\kernel32.lib -D kernel32.dll
    & "$llvm\clang.exe" --target=x86_64-pc-windows-msvc -O2 -nostdlib -fuse-ld=lld `
        "-Wl,/entry:_start" "-Wl,/subsystem:console" "-Wl,/base:0x400000" `
        ..\sdk\win\hello_win.c ..\sdk\win\kernel32.lib -o ..\sdk\win\hello_win.exe
} else {
    Write-Host "  dlltool missing: win sample skipped (PE loader regress via fixtures only)"
}

Write-Host "== [0d] build M9 agent (ring3, model-call client) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\ai\agent.c -o ..\sdk\ai\agent.elf
} else {
    Write-Host "  clang missing: agent skipped"
}

Write-Host "== [1/4] generate boot stub (32-bit stub + page tables + GDT) =="
python boot\gen_stub32.py

Write-Host "== [2/4] cargo build (x86_64-unknown-none) =="
cargo build --release

Write-Host "== [3/4] flatten + QEMU boot (module = PE win sample; ELF 回归: 换 -initrd user_test.elf) =="
python ..\tools\flatten_elf.py target\x86_64-unknown-none\release\fujo-kernel fujo-kernel.bin --pad 0x120000
qemu-system-x86_64 -m 128M -kernel fujo-kernel.bin -initrd ..\sdk\win\hello_win.exe -display none -serial stdio -monitor none -no-reboot
