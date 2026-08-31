# demo-m10.ps1 - M10 demo: Hermes CLI (ring3) -> COM2 model link -> Qwen2.5-0.5B (Ollama)
#
# Usage (Windows PowerShell 5.1 and pwsh 7 both OK, ASCII-only):
#   powershell -ExecutionPolicy Bypass -File scripts\demo-m10.ps1              # build + run demo
#   powershell -ExecutionPolicy Bypass -File scripts\demo-m10.ps1 -NoBuild     # skip build
#   powershell -ExecutionPolicy Bypass -File scripts\demo-m10.ps1 -Interactive # HMP monitor on 4567
#
# Expected key log lines:
#   hermes: cmd='run'
#   ai   : classify('run') -> ... [engine=qwen; model=qwen2.5:0.5b; t=0x64 ms]
#   hermes: intent=0x1 RUN
#   hermes: tool=module(user_test) planned
#   hermes> ... (type exit -> kernel takeover)
param(
    [switch]$NoBuild,        # skip clang/cargo/flatten build
    [switch]$Interactive,    # open HMP monitor (127.0.0.1:4567) for sendkey injection
    [int]$MonitorPort = 4567
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$llvm = 'C:\Program Files\LLVM\bin'

Write-Host '== [0] prereq check: ollama + qwen2.5:0.5b =='
if (-not (Get-Command ollama -ErrorAction SilentlyContinue)) {
    Write-Error 'ollama not found (install from https://ollama.com)'; exit 1
}
if ((ollama list 2>$null) -notlike '*qwen2.5*') {
    Write-Host '  pulling qwen2.5:0.5b (~397MB, cached after first download) ...'
    ollama pull qwen2.5:0.5b
} else {
    Write-Host '  qwen2.5:0.5b ready'
}

if (-not $NoBuild) {
    Write-Host '== [1] build =='
    Set-Location "$root\kernel"   # MUST run cargo from kernel\ (build.rs guard enforces x86_64-unknown-none)
    if (Test-Path "$llvm\clang.exe") {
        & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie `
            -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
            ..\sdk\hermes\hermes.c -o ..\sdk\hermes\hermes.elf
        if ($LASTEXITCODE -ne 0) { Write-Error 'clang hermes failed'; exit 1 }
        Write-Host '  hermes.elf OK'
    } else {
        Write-Error "LLVM/clang not found ($llvm)"; exit 1
    }
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Write-Error 'cargo build failed'; exit 1 }
    python ..\tools\flatten_elf.py target\x86_64-unknown-none\release\fujo-kernel fujo-kernel.bin --pad 0x120000
    if ($LASTEXITCODE -ne 0) { Write-Error 'flatten failed'; exit 1 }
    Set-Location "$root"
    Write-Host '  kernel image OK'
} else {
    Write-Host '  -NoBuild: using existing kernel\fujo-kernel.bin and sdk\hermes\hermes.elf'
}

Write-Host '== [2] start host model server (qwen_model_server.py) =='
$srv = Start-Process -FilePath (Get-Command python).Source `
       -ArgumentList "$root\tools\qwen_model_server.py" -PassThru -WindowStyle Hidden
Start-Sleep -Milliseconds 500
Write-Host "  model server PID=$($srv.Id) waiting for QEMU COM2 (127.0.0.1:4000)"

Write-Host '== [3] QEMU boot (COM1=log, COM2=model link) =='
Write-Host '  boot: logo ~2.5s -> os shell; type "os run hermes" (or wait 10s auto-run)'
Write-Host '  expect hermes banner + classify ... [engine=qwen] within ~20s'
Write-Host '  Ctrl+C exits QEMU and recycles the model server process'
$qargs = @('-m', '128M',
           '-kernel', "$root\kernel\fujo-kernel.bin",
           '-initrd', "$root\sdk\hermes\hermes.elf",
           '-serial', 'stdio',
           '-serial', 'tcp:127.0.0.1:4000,server=on,wait=off',
           '-display', 'none', '-no-reboot')
if ($Interactive) {
    $qargs += @('-monitor', "telnet:127.0.0.1:$MonitorPort,server,nowait")
    Write-Host "  interactive: in another terminal run:"
    Write-Host "    powershell -ExecutionPolicy Bypass -File $root\scripts\demo-m10-keys.ps1 -Command \"open file\""
    Write-Host "    powershell -ExecutionPolicy Bypass -File $root\scripts\demo-m10-keys.ps1 -Command \"exit\""
}
try {
    & qemu-system-x86_64 @qargs
} finally {
    if ($srv -and -not $srv.HasExited) { Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue }
}
