$ErrorActionPreference = "Stop"

$Repo = "GodPuffin/squid"
$BinDir = Join-Path $HOME ".local\bin"
$IsArm = $env:PROCESSOR_ARCHITECTURE -eq "ARM64" -or $env:PROCESSOR_ARCHITEW6432 -eq "ARM64"

if ($IsArm) {
    throw "Windows ARM64 builds are not published yet."
}

$AssetName = "squid-x86_64-pc-windows-msvc.zip"
$Release = Invoke-RestMethod -Headers @{ "User-Agent" = "squid-installer" } -Uri "https://api.github.com/repos/$Repo/releases/latest"
$Asset = $Release.assets | Where-Object { $_.name -eq $AssetName } | Select-Object -First 1

if (-not $Asset) {
    throw "Could not find release asset '$AssetName' on the latest GitHub release."
}

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("squid-install-" + [guid]::NewGuid())
$ZipPath = Join-Path $TempDir $AssetName
$ExtractDir = Join-Path $TempDir "extract"
New-Item -ItemType Directory -Force -Path $TempDir, $ExtractDir | Out-Null

Invoke-WebRequest -Uri $Asset.browser_download_url -OutFile $ZipPath
Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force

$SourceBinary = Join-Path $ExtractDir "squid.exe"
if (-not (Test-Path $SourceBinary)) {
    throw "Downloaded archive did not contain squid.exe"
}

$TargetBinary = Join-Path $BinDir "squid.exe"
Copy-Item $SourceBinary $TargetBinary -Force

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$PathEntries = @()
if ($UserPath) {
    $PathEntries = $UserPath.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
}

if ($PathEntries -notcontains $BinDir) {
    $NewPath = if ([string]::IsNullOrWhiteSpace($UserPath)) { $BinDir } else { "$UserPath;$BinDir" }
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path = "$env:Path;$BinDir"
    $PathMessage = "Added $BinDir to your user PATH. Restart your terminal if needed."
} else {
    $PathMessage = "$BinDir is already on PATH."
}

Remove-Item -Recurse -Force $TempDir

Write-Host "Installed squid to $TargetBinary"
Write-Host $PathMessage
Write-Host "Run: squid path\to\database.sqlite"
