# FujoOS build-kernel.ps1 — 构建用户程序 (agents) -> 内核 -> 扁平化 -> QEMU 启动验证
param(
    # 启动模块: 默认 M10 Hermes CLI; M3 回归用 ..\sdk\win\hello_win.exe
    [string]$Initrd = '..\sdk\hermes\hermes.elf'
)
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

Write-Host "== [0e] build M10 Hermes CLI (ring3, agent frontend + qwen model call) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\hermes\hermes.c -o ..\sdk\hermes\hermes.elf
} else {
    Write-Host "  clang missing: hermes skipped"
}

Write-Host "== [0f] build M11 alloc test (ring3, brk/mmap verification) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\alloc_test.c -o ..\sdk\user\alloc_test.elf
} else {
    Write-Host "  clang missing: alloc_test skipped"
}

Write-Host "== [0g] build M13 thread demo (ring3, timeslice tasks) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\thread_demo.c -o ..\sdk\user\thread_demo.elf
} else {
    Write-Host "  clang missing: thread_demo skipped"
}

Write-Host "== [0h] build M14 crash demo (ring3, process isolation) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\crash_demo.c -o ..\sdk\user\crash_demo.elf
} else {
    Write-Host "  clang missing: crash_demo skipped"
}

Write-Host "== [0i] build M15 VFS test (ring3, open/read/close) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\fs_test.c -o ..\sdk\user\fs_test.elf
} else {
    Write-Host "  clang missing: fs_test skipped"
}

Write-Host "== [0j] build M18 IPC test (ring3, pipe+shm+sig) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\ipc_test.c -o ..\sdk\user\ipc_test.elf
} else {
    Write-Host "  clang missing: ipc_test skipped"
}

Write-Host "== [0k] build M19 kobj test (ring3, kernel object table) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\kobj_test.c -o ..\sdk\user\kobj_test.elf
} else {
    Write-Host "  clang missing: kobj_test skipped"
}

Write-Host "== [0l] build M20 crash/isolation test (ring3, user exc -> survivor) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\m20_crash.c -o ..\sdk\user\m20_crash.elf
} else {
    Write-Host "  clang missing: m20_crash skipped"
}

Write-Host "== [0m] build M20 leak-stress test (ring3, pipe/kobj churn) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\m20_stress.c -o ..\sdk\user\m20_stress.elf
} else {
    Write-Host "  clang missing: m20_stress skipped"
}

Write-Host "== [0n] build M21 syscall-surface test (ring3, ~20 linux syscalls) =="
if (Test-Path "$llvm\clang.exe") {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie -no-pie `
        -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
        ..\sdk\user\m21_syscalls.c -o ..\sdk\user\m21_syscalls.elf
} else {
    Write-Host "  clang missing: m21_syscalls skipped"
}

Write-Host "== [1/4] generate boot stub (32-bit stub + page tables + GDT) =="
python boot\gen_stub32.py

Write-Host "== [2/4] cargo build (x86_64-unknown-none) =="
cargo build --release

Write-Host "== [3/4] flatten + QEMU boot (COM1=日志 stdio, COM2=模型链路 tcp:4000) =="
python ..\tools\flatten_elf.py target\x86_64-unknown-none\release\fujo-kernel fujo-kernel.bin --pad 0x1A0000
qemu-system-x86_64 -m 128M -kernel fujo-kernel.bin -initrd $Initrd `
    -serial stdio -serial tcp:127.0.0.1:4000,server=on,wait=off `
    -display none -monitor none -no-reboot
