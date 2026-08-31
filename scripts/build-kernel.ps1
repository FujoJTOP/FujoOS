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

Write-Host "== [1/4] generate boot stub (32-bit stub + page tables + GDT) =="
python boot\gen_stub32.py

Write-Host "== [2/4] cargo build (x86_64-unknown-none) =="
cargo build --release

Write-Host "== [3/4] flatten + QEMU boot =="
python ..\tools\flatten_elf.py target\x86_64-unknown-none\release\fujo-kernel fujo-kernel.bin --pad 0x110000
qemu-system-x86_64 -m 128M -kernel fujo-kernel.bin -display none -serial stdio -monitor none -no-reboot
