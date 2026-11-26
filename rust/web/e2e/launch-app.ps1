# Launches the app detached from the caller (WMI), with the WebView2 remote debugging port open,
# and waits until the port answers. A nonzero exit code in the calling shell can otherwise kill
# processes started with Start-Process.
#   powershell -File launch-app.ps1                       installed app
#   powershell -File launch-app.ps1 -Exe ..\..\target\debug\sqlt-desktop.exe
param(
  [string]$Exe = (Join-Path $env:LOCALAPPDATA "SQLTools\sqlt-desktop.exe"),
  [int]$Port = 9222
)
$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
Get-Process sqlt-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
$commandLine = "cmd.exe /c set WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=$Port&& start `"`" `"$Exe`""
$result = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = $commandLine }
if ($result.ReturnValue -ne 0) { throw "Win32_Process.Create failed: $($result.ReturnValue)" }
$deadline = (Get-Date).AddSeconds(40)
while ((Get-Date) -lt $deadline) {
  try {
    $null = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/version" -TimeoutSec 2
    Write-Output "app ready on port $Port ($Exe)"
    exit 0
  } catch { Start-Sleep -Milliseconds 400 }
}
Write-Output "app did not open port $Port within 40 s"
exit 1
