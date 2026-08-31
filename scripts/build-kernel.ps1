# FujoOS build-kernel.ps1 — 构建内核 -> 扁平化 -> QEMU 启动验证
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location "$root\kernel"

Write-Host "== [1/3] generate boot stub (32-bit stub + page tables + GDT) =="
python boot\gen_stub32.py

Write-Host "== [2/3] cargo build (x86_64-unknown-none) =="
cargo build --release

Write-Host "== [3/3] flatten + QEMU boot =="
python ..\tools\flatten_elf.py target\x86_64-unknown-none\release\fujo-kernel fujo-kernel.bin --pad 0x107000
qemu-system-x86_64 -m 128M -kernel fujo-kernel.bin -display none -serial stdio -monitor none -no-reboot
