# demo-m10-keys.ps1 - inject a typed command into the QEMU HMP monitor
# Use together with: scripts\demo-m10.ps1 -Interactive
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\demo-m10-keys.ps1 -Command "open file"
#   powershell -ExecutionPolicy Bypass -File scripts\demo-m10-keys.ps1 -Command "exit"
param(
    [string]$Command = 'open file',
    [int]$MonitorPort = 4567
)
$tcp = New-Object System.Net.Sockets.TcpClient
$tcp.Connect('127.0.0.1', $MonitorPort)
$stream = $tcp.GetStream()
$writer = New-Object System.IO.StreamWriter($stream)
$writer.AutoFlush = $true
$writer.NewLine = "`r`n"
foreach ($c in $Command.ToCharArray()) {
    if ($c -eq ' ') { $k = 'spc' }
    elseif ($c -eq '.') { $k = 'dot' }
    elseif ($c -eq '-') { $k = 'minus' }
    else { $k = [string]$c }
    $writer.WriteLine("sendkey $k")
    Start-Sleep -Milliseconds 120
}
$writer.WriteLine('sendkey ret')
Start-Sleep -Milliseconds 300
$writer.Close()
$tcp.Close()
Write-Output "sent: $Command + enter"
