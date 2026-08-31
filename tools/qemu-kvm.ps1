# qemu-kvm.ps1 — M57: KVM/加速启动开关 + 对照基准入口 (Windows 宿主 TCG; WSL2/原生 Linux 可 -EnableKvm)
param(
    [string]$Kernel = "C:\Users\hooya\Documents\FujoOS\kernel\fujo-kernel.bin",
    [string]$Initrd = "C:\Users\hooya\Documents\FujoOS\sdk\linux\m57_accel.elf",
    [switch]$EnableKvm
)
$args = @('-m','256M')
if ($EnableKvm) { $args += '-enable-kvm' }
$args += @('-kernel',$Kernel,'-initrd',$Initrd,
  '-serial','file:C:\Users\hooya\Documents\FujoOS\qemu-kvm.log',
  '-serial','tcp:127.0.0.1:4001,server=on,wait=off',
  '-monitor','telnet:127.0.0.1:4568,server,nowait',
  '-display','none','-no-reboot')
Write-Host "qemu-kvm: $($args -join ' ')"
Start-Process qemu-system-x86_64 -ArgumentList $args
