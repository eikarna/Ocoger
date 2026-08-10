# One-line installer for ocoger (Windows, PowerShell 5.1+ / pwsh 7+).
#   irm https://raw.githubusercontent.com/eikarna/Ocoger/main/install.ps1 | iex
# Env-style overrides via global vars before iex if downloading the file first:
#   $env:OCOGER_VERSION = 'v0.1.3'
#   $env:OCOGER_PREFIX  = "$env:LOCALAPPDATA\Programs\ocoger"
$ErrorActionPreference = 'Stop'

$Repo   = 'eikarna/Ocoger'
$Prefix = if ($env:OCOGER_PREFIX) { $env:OCOGER_PREFIX } else { "$env:LOCALAPPDATA\Programs\ocoger" }
$Ver    = if ($env:OCOGER_VERSION) { $env:OCOGER_VERSION } else { 'latest' }

$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { 'x86_64' }
    'ARM64' { 'aarch64' }
    default { throw "unsupported arch: $env:PROCESSOR_ARCHITECTURE" }
}
$target = "$arch-pc-windows-msvc"

if ($Ver -eq 'latest') {
    $rel = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Ver = $rel.tag_name
    if (-not $Ver) { throw 'could not resolve latest release tag' }
}

$url = "https://github.com/$Repo/releases/download/$Ver/ocoger-$Ver-$target.zip"
$tmp = New-Item -ItemType Directory -Path ([IO.Path]::Combine([IO.Path]::GetTempPath(), "ocoger-$([Guid]::NewGuid().ToString('N'))"))
try {
    $zip = Join-Path $tmp.FullName 'ocoger.zip'
    Write-Host "==> downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing

    Expand-Archive -Path $zip -DestinationPath $tmp.FullName -Force
    $bin = Get-ChildItem -Path $tmp.FullName -Recurse -Filter ocoger.exe | Select-Object -First 1
    if (-not $bin) { throw 'archive did not contain ocoger.exe' }

    New-Item -ItemType Directory -Force -Path $Prefix | Out-Null
    Copy-Item $bin.FullName (Join-Path $Prefix 'ocoger.exe') -Force
    Write-Host "==> installed $Prefix\ocoger.exe ($Ver)"

    # Prepend to user PATH if not already present.
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (($userPath -split ';') -notcontains $Prefix) {
        [Environment]::SetEnvironmentVariable('Path', "$Prefix;$userPath", 'User')
        Write-Host 'note: PATH updated for new shells; restart your terminal.'
    }
} finally {
    Remove-Item -Recurse -Force $tmp.FullName -ErrorAction SilentlyContinue
}
