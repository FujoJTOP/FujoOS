# onebuild.ps1 — M85: 工具链验收 (hello/gui/game 一键构建运行)
#
# 用法: pwsh scripts/onebuild.ps1 [-BuildOnly] [-Kernel kernel/fujo-kernel.bin]
# 输出: sdk/build/one/{hello,gui,game}.elf + .run
#   (--BuildOnly 跳过 QEMU 运行验证)
param(
    [switch]$BuildOnly,
    [string]$Kernel = 'kernel/fujo-kernel.bin'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$llvm = 'C:\Program Files\LLVM\bin'
$out = 'sdk/build/one'
New-Item -ItemType Directory -Force -Path (Join-Path $root $out) | Out-Null

function Build-Tpl([string]$src) {
    $name = [System.IO.Path]::GetFileNameWithoutExtension($src)
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static `
        -fno-pie -no-pie -fuse-ld=lld -fno-builtin "-Wl,-e,_start" `
        "-Wl,-T,$root\sdk\user\user.ld" (Join-Path $root $src) `
        -o (Join-Path $root "$out\$name.elf")
    if ($LASTEXITCODE -ne 0) { throw "build failed: $name" }
    & python (Join-Path $root 'tools\fujopack.py') pack `
        -e (Join-Path $root "$out\$name.elf") -o (Join-Path $root "$out\$name.run") `
        --name $name --type app
    Write-Host "  ok: $out\$name.elf / .run"
    return $name
}

Write-Host "== onebuild: hello/gui/game (templates -> elf -> run) =="
$names = @()
$names += Build-Tpl 'sdk/templates/hello.tpl.c'
$names += Build-Tpl 'sdk/templates/game.tpl.c'
$names += Build-Tpl 'sdk/templates/gui.tpl.c'

if ($BuildOnly) {
    Write-Host "onebuild: build-only 3/3 PASS ($out)"
    exit 0
}

Write-Host "== QEMU verify =="
$expect = @{
    'hello.tpl' = 'hello: FujoOS template app'
    'game.tpl'  = 'game: template frame loop'
    'gui.tpl'   = 'gui: template (fujokit skeleton)'
}
$pass = 0
foreach ($n in $names) {
    Write-Host ":: $n ..." -NoNewline
    $log = Join-Path $root ".one-$n.log"
    Remove-Item $log -ErrorAction SilentlyContinue
    Get-Process qemu-system-x86_64 -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 700
    $p = Start-Process -FilePath (Get-Command qemu-system-x86_64).Source -ArgumentList @(
        '-m', '256M', '-kernel', (Join-Path $root $Kernel),
        '-initrd', (Join-Path $root "$out\$n.elf"),
        '-serial', "file:$log",
        '-serial', 'tcp:127.0.0.1:4001,server=on,wait=off',
        '-monitor', 'telnet:127.0.0.1:4568,server,nowait',
        '-display', 'none', '-no-reboot'
    ) -PassThru
    Start-Sleep -Seconds 8
    try {
        $s = New-Object System.Net.Sockets.TcpClient('127.0.0.1', 4568)
        $enc = [Text.Encoding]::ASCII
        $b0 = New-Object byte[] 1024
        $null = $s.GetStream().Read($b0, 0, 1024)
        foreach ($k in @('o','s','spc','r','u','n','spc','h','e','r','m','e','s','ret')) {
            $b = $enc.GetBytes("sendkey $k`n")
            $s.GetStream().Write($b, 0, $b.Length)
            $s.GetStream().Flush()
            Start-Sleep -Milliseconds 110
        }
        $s.Close()
    } catch {}
    Start-Sleep -Seconds 3
    if (-not $p.HasExited) { $p.Kill() }
    $txt = if (Test-Path $log) { Get-Content $log -Raw } else { '' }
    if ($txt.Contains($expect[$n])) {
        Write-Host " PASS"
        $pass += 1
    } else {
        Write-Host " FAIL"
    }
}
Write-Host "== onebuild =="
if ($pass -eq 3) {
    Write-Host "onebuild: 3/3 PASS (hello/gui/game build+run)"
    exit 0
}
Write-Host "onebuild: $pass/3 PASS"
exit 1
