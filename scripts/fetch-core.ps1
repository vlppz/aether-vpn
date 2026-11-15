# Windows counterpart of fetch-core.sh: downloads the pinned Xray-core
# release, verifies its SHA-256 digest, and installs the sidecar, geo files
# and wintun.dll.
#
#   powershell -ExecutionPolicy Bypass -File scripts\fetch-core.ps1 [-Target x86_64-pc-windows-msvc]
param([string]$Target = "")

$ErrorActionPreference = "Stop"
$XrayVersion = if ($env:XRAY_VERSION) { $env:XRAY_VERSION } else { "v26.11.9" }
$WintunVersion = "0.14.1"
$Root = Split-Path -Parent $PSScriptRoot
$BinDir = Join-Path $Root "src-tauri\binaries"
$ResDir = Join-Path $Root "src-tauri\resources"

if (-not $Target) {
    $Target = ((rustc -vV) | Select-String "^host: (.+)$").Matches[0].Groups[1].Value
}
switch ($Target) {
    "x86_64-pc-windows-msvc"  { $Asset = "Xray-windows-64";        $WintunArch = "amd64" }
    "aarch64-pc-windows-msvc" { $Asset = "Xray-windows-arm64-v8a"; $WintunArch = "arm64" }
    default { throw "unsupported target: $Target (use fetch-core.sh for macOS/Linux)" }
}

$Work = Join-Path ([System.IO.Path]::GetTempPath()) ("aether-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $Work, $BinDir, $ResDir -Force | Out-Null
try {
    $Base = "https://github.com/XTLS/Xray-core/releases/download/$XrayVersion"
    Write-Host "-> downloading $Asset.zip ($XrayVersion)"
    Invoke-WebRequest "$Base/$Asset.zip" -OutFile "$Work\xray.zip"
    Invoke-WebRequest "$Base/$Asset.zip.dgst" -OutFile "$Work\xray.zip.dgst"

    $Expected = ((Get-Content "$Work\xray.zip.dgst") | Select-String "^SHA2-256=\s*(\w+)").Matches[0].Groups[1].Value.ToLower()
    $Actual = (Get-FileHash "$Work\xray.zip" -Algorithm SHA256).Hash.ToLower()
    if ($Expected -ne $Actual) { throw "checksum mismatch: expected $Expected got $Actual" }
    Write-Host "OK sha256 $Actual"

    Expand-Archive "$Work\xray.zip" -DestinationPath "$Work\x" -Force
    Copy-Item "$Work\x\xray.exe" (Join-Path $BinDir "xray-$Target.exe") -Force
    Copy-Item "$Work\x\geoip.dat", "$Work\x\geosite.dat" $ResDir -Force

    if (Test-Path "$Work\x\wintun.dll") {
        Copy-Item "$Work\x\wintun.dll" $ResDir -Force
    } else {
        Write-Host "-> downloading wintun $WintunVersion"
        Invoke-WebRequest "https://www.wintun.net/builds/wintun-$WintunVersion.zip" -OutFile "$Work\wintun.zip"
        Expand-Archive "$Work\wintun.zip" -DestinationPath "$Work\w" -Force
        Copy-Item "$Work\w\wintun\bin\$WintunArch\wintun.dll" $ResDir -Force
    }
    Write-Host "OK installed $(Join-Path $BinDir "xray-$Target.exe")"
} finally {
    Remove-Item $Work -Recurse -Force -ErrorAction SilentlyContinue
}
