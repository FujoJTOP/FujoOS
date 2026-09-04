# build-samples.ps1 — 从源码重建 fujoci/fujoregress 用例样本 (编译产物不入库)
#
# 产出: sdk/linux/m{30,33,35..77,82..95}.elf ... + sdk/build/{m31_res.run,
#       m32_multi.initrd,hello.*} + sdk/win/*.exe + sdk/mac/*.macho
param(
    [string]$Out = 'sdk'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$llvm = 'C:\Program Files\LLVM\bin'
$T = "-Wl,-T,$root\sdk\user\user.ld"

function Build-Elf([string]$name) {
    & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static `
        -fno-pie -no-pie -fuse-ld=lld -fno-builtin "-Wl,-e,_start" $T `
        "-I$root\$Out" "$root\sdk\linux\$name.c" -o "$root\sdk\linux\$name.elf"
    if ($LASTEXITCODE -ne 0) { throw "build failed: $name" }
}

# --- W10: 策略蒸馏字节码 (FJRU v1; 离线确定性; demo m120 内嵌) ---
python "$root\tools\distill_rules.py" --out "$root\sdk\rulebook\fjru.bin" --header "$root\sdk\rulebook\rulebook.h"
if ($LASTEXITCODE -ne 0) { throw "distill failed" }

# --- W13: virtio-blk 参考盘 (mk_vblk.py; demo m123 逐字节比对) ---
python "$root\tools\mk_vblk.py" "$root\sdk\vblk.img"
if ($LASTEXITCODE -ne 0) { throw "mk_vblk failed" }

# --- W20: AHCI 参考盘 (mk_ahci.py; demo m134 ich9-ahci) ---
python "$root\tools\mk_ahci.py"
if ($LASTEXITCODE -ne 0) { throw "mk_ahci failed" }

# --- ELF 样例 (fujoci MILESTONES + 矩阵) ---
foreach ($n in @('m30_linux','m33_trace','m35_bench','m36_mouse','m37_wm','m38_wm',
                 'm39_font','m40_ime','m41_kit','m42_gui','m43_clip','m44_icon','m45_term',
                 'm46_desk','m47_vbe','m48_ime2','m49_a11y','m50_bench','m51_disp','m52_audio',
                 'm53_xin','m54_timer','m55_gl','m56_dxwrap','m57_accel','m58_pong','m59_gamemode',
                 'm60_save','m61_blit','m62_shader','m63_mix','m64_smp','m65_tss','m66_pcache',
                 'm67_irq','m68_perf','m69_game2','m71_asm','m72_ld','m73_edit','m74_cc',
                 'm75_dbg','m76_trace','m77_win','m82_ut','m83_leak','m84_dump','m86_wmap',
                 'm87_mcard','m88_sess','m89_ctx','m90_ctx','m91_cap','m92_route','m93_infer',
                 'm94_fupm','m95_life','m96_acpi','m97_hw','m98_install','m99_upd',
                 'm112_ai','m113_plan','m114_nlc','m115_five','m118_r3','m119_inv','m116_dom','m120_distill','m121_isol','m122_dev','m123_vblk','m127_exec','m129_smp','m130_aud','m132_dirs','m133_plat','m134_ahci','m135_fs','m136_mem','m137_pci')) {
    Build-Elf $n
}
Write-Host "samples: elf ok"

# --- Mach-O / PE ---
& "$llvm\clang.exe" --target=x86_64-apple-macos11 -O2 -nostdlib -fuse-ld=lld `
    "$root\sdk\mac\m29_darwin.c" -o "$root\sdk\mac\m29_darwin.macho"
if ($LASTEXITCODE -ne 0) { throw "macho failed" }
& "$llvm\llvm-dlltool.exe" -d "$root\sdk\win\kernel32.def" -l "$root\sdk\win\kernel32.lib" -D kernel32.dll
foreach ($n in @('hello_win','m26_win','m30_win')) {
    & "$llvm\clang.exe" --target=x86_64-pc-windows-msvc -O2 -nostdlib -fuse-ld=lld `
        "-Wl,/entry:_start" "-Wl,/subsystem:console" "-Wl,/base:0x400000" `
        "$root\sdk\win\$n.c" "$root\sdk\win\kernel32.lib" -o "$root\sdk\win\$n.exe"
    if ($LASTEXITCODE -ne 0) { throw "pe failed: $n" }
}
# 注: m27_mingw.exe (mingw-w64 CRT) / m28_vc.exe (MSVC CRT) 需专用工具链,
# 保留为例外 (见 .gitignore)
Write-Host "samples: macho/pe ok"

# --- 打包 (m31 .run / m32 multi.initrd) ---
New-Item -ItemType Directory -Force -Path "$root\sdk\build" | Out-Null
python "$root\tools\fujopack.py" pack -e "$root\sdk\linux\m31_res.elf" `
    -r demo.txt:"$root\sdk\linux\m31_demo.txt" -o "$root\sdk\build\m31_res.run"
python "$root\tools\fujorun.py" pack -i "$root\sdk\linux\m32_lib.elf" `
    --lib "$root\sdk\linux\catlib.bin" -o "$root\sdk\build\m32_multi.initrd"
Write-Host "samples: pack ok"

# --- M107/M108: 桌面会话样本 (高地址窗口程序 user-high.ld @0x1000000; 代理 user.ld @0x400000) ---
$TH = "-Wl,-T,$root\sdk\user\user-high.ld"
& "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static `
    -fno-pie -no-pie -fuse-ld=lld -fno-builtin "-Wl,-e,_start" $TH `
    "$root\sdk\hermes\hermes.c" -o "$root\sdk\hermes\hermes-high.elf"
if ($LASTEXITCODE -ne 0) { throw "build failed: hermes-high" }
& "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static `
    -fno-pie -no-pie -fuse-ld=lld -fno-builtin "-Wl,-e,_start" $TH `
    "$root\sdk\linux\m107_tty.c" -o "$root\sdk\linux\m107_tty-high.elf"
if ($LASTEXITCODE -ne 0) { throw "build failed: m107_tty-high" }
Build-Elf 'm108_desk'
Write-Host "samples: desktop session ok (hermes-high / tty-high / m108_desk)"

Write-Host "build-samples: ALL OK"
