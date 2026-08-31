# demo-m10.ps1 — M10 端到端演示: Hermes CLI(ring3) -> COM2 模型链路 -> qwen2.5:0.5b
#
# 用法:
#   powershell -ExecutionPolicy Bypass -File scripts\demo-m10.ps1             # 构建+启动演示
#   powershell -ExecutionPolicy Bypass -File scripts\demo-m10.ps1 -NoBuild    # 跳过构建直接跑
#   powershell -ExecutionPolicy Bypass -File scripts\demo-m10.ps1 -Interactive # 开启 HMP monitor, 可用 sendkey 键入命令
#
# 期望输出 (关键行):
#   hermes: cmd='run'
#   ai   : classify('run') -> ... [engine=qwen; model=qwen2.5:0.5b; t=0x64 ms]
#   hermes: intent=0x1 RUN
#   hermes: tool=module(user_test) planned
#   hermes> ... (键入 exit -> 内核接管)
param(
    [switch]$NoBuild,        # 跳过 clang/cargo/flatten 构建
    [switch]$Interactive,    # 打开 HMP monitor (127.0.0.1:4567) 以便注入按键
    [int]$MonitorPort = 4567
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$llvm = 'C:\Program Files\LLVM\bin'

Write-Host "== [0] 前置检查: ollama + qwen2.5:0.5b =="
if (-not (Get-Command ollama -ErrorAction SilentlyContinue)) {
    Write-Error "缺少 ollama (https://ollama.com 安装), 中止"; exit 1
}
if ((ollama list 2>$null) -notmatch 'qwen2\.5:0\.5b') {
    Write-Host "  ollama pull qwen2.5:0.5b (~397MB, 首次下载后缓存) ..."
    ollama pull qwen2.5:0.5b
} else {
    Write-Host "  qwen2.5:0.5b 已就绪"
}

if (-not $NoBuild) {
    Write-Host "== [1] 构建 =="
    Set-Location "$root\kernel"   # 必须从 kernel/ 目录跑 cargo (build.rs 守卫: 目标必须是 x86_64-unknown-none)
    if (Test-Path "$llvm\clang.exe") {
        & "$llvm\clang.exe" --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie `
            -fuse-ld=lld -fno-builtin "-Wl,-e,_start" "-Wl,-T,$root\sdk\user\user.ld" `
            ..\sdk\hermes\hermes.c -o ..\sdk\hermes\hermes.elf
        Write-Host "  hermes.elf OK"
    } else {
        Write-Error "缺少 LLVM/clang (C:\Program Files\LLVM)"; exit 1
    }
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Write-Error "cargo 构建失败"; exit 1 }
    python ..\tools\flatten_elf.py target\x86_64-unknown-none\release\fujo-kernel fujo-kernel.bin --pad 0x120000
    Set-Location "$root"
    Write-Host "  内核镜像 OK"
} else {
    Write-Host "  -NoBuild: 使用 kernel\fujo-kernel.bin 与 sdk\hermes\hermes.elf (需已存在)"
}

Write-Host "== [2] 启动宿主机模型服务器 (qwen_model_server.py) =="
$srv = Start-Process -FilePath (Get-Command python).Source `
       -ArgumentList "$root\tools\qwen_model_server.py" -PassThru -WindowStyle Hidden
Start-Sleep -Milliseconds 500
Write-Host "  模型服务器 PID=$($srv.Id) — 连接 QEMU COM2 (127.0.0.1:4000)"

Write-Host "== [3] QEMU 启动 (COM1=日志, COM2=模型链路) =="
Write-Host "  预期: 约 4 秒后看到 hermes: ... 与 classify ... [engine=qwen] 行"
Write-Host "  Ctrl+C 退出后自动回收模型服务器进程"
$qargs = @('-m', '128M',
           '-kernel', "$root\kernel\fujo-kernel.bin",
           '-initrd', "$root\sdk\hermes\hermes.elf",
           '-serial', 'stdio',
           '-serial', 'tcp:127.0.0.1:4000,server=on,wait=off',
           '-display', 'none', '-no-reboot')
if ($Interactive) {
    $qargs += @('-monitor', "telnet:127.0.0.1:$MonitorPort,server,nowait")
    Write-Host "  交互模式: 另开终端用下方命令注入按键 (键盘扫描码):"
    Write-Host "    powershell -Command `"`$t=New-Object Net.Sockets.TcpClient;`$t.Connect('127.0.0.1',$MonitorPort);`$w=New-Object IO.StreamWriter(`$t.GetStream());`$w.AutoFlush=`$true;foreach(`$k in @('o','p','e','n','spc','f','i','l','e','ret')){`$w.WriteLine('sendkey '+`$k);Start-Sleep -Milliseconds 150};`$t.Close()`""
    Write-Host "  (键入 exit: e,x,i,t,ret)"
}
try {
    & qemu-system-x86_64 @qargs
} finally {
    if ($srv -and -not $srv.HasExited) { Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue }
}
