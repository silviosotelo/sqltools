# Builds, self-signs and installs the NSIS bundle locally so the e2e checks can run against the
# INSTALLED app. Requires %USERPROFILE%\.tauri\sqltools.key (updater key) and signtool (Windows SDK).
#   powershell -File install-local.ps1
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")          # rust/
$cargo = Join-Path $env:USERPROFILE ".cargo\bin"
$env:Path = "$cargo;$env:Path"

$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -eq "CN=SQLTools Self-Signed" } | Select-Object -First 1
if (-not $cert) {
  $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=SQLTools Self-Signed" -CertStoreLocation Cert:\CurrentUser\My -KeyUsage DigitalSignature -KeyExportPolicy Exportable -NotAfter (Get-Date).AddYears(3) -FriendlyName "SQLTools Self-Signed"
}
$signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter signtool.exe | Where-Object { $_.FullName -match "\\x64\\" } | Select-Object -First 1 -ExpandProperty FullName
$config = Join-Path $env:TEMP "sqlt-sign.json"
@{ bundle = @{ windows = @{ certificateThumbprint = $cert.Thumbprint } } } | ConvertTo-Json -Depth 5 | Set-Content $config -Encoding ascii

Get-Process sqlt-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Push-Location $root
& npm.cmd --prefix web run build
if ($LASTEXITCODE -ne 0) { throw "web build failed" }
& cargo build -p sqlt-desktop --release
if ($LASTEXITCODE -ne 0) { throw "cargo release build failed" }
# The installer embeds the app binary, so sign it before bundling.
& $signtool sign /sha1 $cert.Thumbprint /fd SHA256 /d SQLTools "target\release\sqlt-desktop.exe" | Out-Null
Push-Location desktop
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content (Join-Path $env:USERPROFILE ".tauri\sqltools.key") -Raw).TrimEnd("`r", "`n")
& npx.cmd --yes "@tauri-apps/cli@^2" bundle --config $config --bundles nsis
if ($LASTEXITCODE -ne 0) { throw "tauri bundle failed" }
Pop-Location
$setup = Get-ChildItem "target\release\bundle\nsis\*-setup.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Pop-Location
$process = Start-Process -FilePath $setup.FullName -ArgumentList "/S" -Wait -PassThru
$installed = Join-Path $env:LOCALAPPDATA "SQLTools\sqlt-desktop.exe"
$signature = Get-AuthenticodeSignature $installed
Write-Output ("installed: {0}  setup exit={1}  size={2}  written={3}  signer={4}" -f $installed, $process.ExitCode, (Get-Item $installed).Length, (Get-Item $installed).LastWriteTime, $signature.SignerCertificate.Subject)
exit 0
