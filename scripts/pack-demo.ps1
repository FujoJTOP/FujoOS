# python make_fixtures
# cargo build --release
# 然后对每种格式 打包 -> 校验 -> 转储
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host "== [1/4] build workspace =="
cargo build --release

Write-Host "== [2/4] generate fixtures (PE/ELF/Mach-O) =="
python sdk\fixtures\make_fixtures.py

Write-Host "== [3/4] pack fixtures -> .run =="
New-Item -ItemType Directory -Force -Path sdk\fixtures\out\run | Out-Null
foreach ($f in @('sample-x64.elf', 'sample-x64-pe.exe', 'sample-x64-macho')) {
    $name = [IO.Path]::GetFileNameWithoutExtension($f)
    target\release\fujopack.exe "sdk\fixtures\out\$f" `
        -o "sdk\fixtures\out\run\$name.run" --name $name
}

Write-Host "== [4/4] validate + dump =="
foreach ($r in Get-ChildItem sdk\fixtures\out\run\*.run) {
    target\release\fujorun.exe $r.FullName --validate
}
target\release\fujorun.exe sdk\fixtures\out\run\sample-x64.run --dump

if (Test-Path 'C:\Program Files\LLVM\bin\clang.exe') {
    Write-Host "== [extra] clang cross-compile + pack =="
    New-Item -ItemType Directory -Force -Path sdk\build | Out-Null
    $llvm = 'C:\Program Files\LLVM\bin'
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -nostdlib -static -fno-pie -no-pie -fuse-ld=lld "-Wl,-e,_start" sdk\hello.c -o sdk\build\hello.elf
    if ($LASTEXITCODE -eq 0) {
        target\release\fujopack.exe sdk\build\hello.elf -o sdk\build\hello.run --name hello
        target\release\fujorun.exe sdk\build\hello.run --dump
    }
} else {
    Write-Host "clang not found; skipped cross-compile demo"
}

Write-Host "done."
