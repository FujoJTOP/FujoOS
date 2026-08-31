# FujoOS setup.ps1 — 一键环境准备
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host "== [1/4] rust toolchain =="
rustup --version 2>$null
rustup target add x86_64-unknown-none

Write-Host "== [2/4] optional: LLVM (clang/lld) =="
if (-not (Test-Path 'C:\Program Files\LLVM\bin\clang.exe')) {
    Write-Host "  LLVM not found. skip cross-compile demo (fixtures still cover the pipeline)"
}

Write-Host "== [3/4] build workspace =="
cargo build --release

Write-Host "== [4/4] verify: pack-demo =="
powershell -ExecutionPolicy Bypass -File scripts\pack-demo.ps1

Write-Host "fujo-sdk ready. Try: powershell scripts\build-kernel.ps1  (QEMU boot demo)"
