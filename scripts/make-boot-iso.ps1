# make-boot-iso.ps1 — W30: 真机/虚拟 ISO 构建 (GRUB2 multiboot v1 + autostart)
# 产物: sdk/build/fujo-boot-<Demo>.iso —— grub.cfg:
#   multiboot /boot/fujo-kernel.bin fujo.run=<Demo>
#   module    /boot/<Demo>.elf
# 内核 boot_autostart 解析 cmdline fujo.run=<name> (模块名匹配) 直启 demo,
# 真机无 monitor sendkey 也可自动运行 (docs/93)。
# 依赖: WSL2 Ubuntu + grub-mkrescue (grub-pc-bin/xorriso/mtools; 已装)。
param(
    [string]$Demo = "m142_feedback",
    [string]$Out = "sdk\build\fujo-boot.iso"
)
$ErrorActionPreference = 'Stop'
$root = (Split-Path -Parent $PSScriptRoot).TrimEnd('\')
$inc = "$root\inc-w30-iso"
New-Item -ItemType Directory -Force -Path "$inc\boot\grub" | Out-Null
Copy-Item "$root\kernel\fujo-kernel.bin" "$inc\boot\fujo-kernel.bin" -Force
Copy-Item "$root\sdk\linux\$Demo.elf" "$inc\boot\$Demo.elf" -Force
$cfg = @"
set timeout=0
menuentry "FujoOS" {
    multiboot /boot/fujo-kernel.bin fujo.run=$Demo
    module /boot/$Demo.elf
}
"@
Set-Content -Path "$inc\boot\grub\grub.cfg" -Value $cfg -Encoding ascii
$iso = Join-Path $root $Out
$wslout = "/mnt/d/Dev/FujoOS/" + ($Out -replace '\\', '/')
$wslinc = "/mnt/d/Dev/FujoOS/inc-w30-iso"
wsl -u root -e bash -c "grub-mkrescue -o $wslout $wslinc 2>&1 | tail -2"
if ($LASTEXITCODE -ne 0) { throw "grub-mkrescue failed" }
Write-Host "iso: $iso (demo=$Demo)"
