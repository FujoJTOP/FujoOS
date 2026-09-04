# qemu-kvm.ps1 — W29: 执行模式对照入口 (TCG / WHPX; WSL2/KVM 用 -Accel kvm)
# TODO(main.rs):
#   fujoregress.py --accel <tcg|whpx> 是全矩阵对照的权威入口; 本脚本保留为
#   单 demo 快速查看 (M57 兼容: -EnableKvm 仍可, Linux/WSL2 下 -enable-kvm)。
param(
    [string]$Kernel = "D:\Dev\FujoOS\kernel\fujo-kernel.bin",
    [string]$Initrd = "D:\Dev\FujoOS\sdk\linux\m142_feedback.elf",
    [string]$Accel = "tcg",
    [switch]$EnableKvm
)
if ($EnableKvm) { $Accel = "kvm" }
$args = @('-m','256M','-accel',$Accel)
$args += @('-kernel',$Kernel,'-initrd',$Initrd,
  '-serial','file:D:\Dev\FujoOS\qemu-kvm.log',
  '-serial','tcp:127.0.0.1:4001,server=on,wait=off',
  '-monitor','telnet:127.0.0.1:4568,server,nowait',
  '-display','none','-no-reboot')
Write-Host "qemu-kvm: accel=$Accel"
Start-Process qemu-system-x86_64 -ArgumentList $args
