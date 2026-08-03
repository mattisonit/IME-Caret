[CmdletBinding()]
param(
    [switch]$SkipTests
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Cargo was not found. Install Rust with the MSVC toolchain first."
}

if (-not $SkipTests) {
    Write-Host "[1/3] Running tests"
    cargo test
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }
}

Write-Host "[2/3] Building release"
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed" }

Write-Host "[3/3] Preparing distribution folder"
$dist = Join-Path $PSScriptRoot "dist"
New-Item -ItemType Directory -Force -Path $dist | Out-Null

$sourceExe = Join-Path $PSScriptRoot "target\release\ime-caret.exe"
$targetExe = Join-Path $dist "IMECaret.exe"
Copy-Item -Force $sourceExe $targetExe
Copy-Item -Force (Join-Path $PSScriptRoot "IMECaret.ini") $dist

foreach ($name in @("IMEE.wav", "IMEJ.wav", "IMEK.wav")) {
    $sound = Join-Path $PSScriptRoot ("assets\" + $name)
    if (Test-Path $sound) {
        Copy-Item -Force $sound $dist
    }
}

Write-Host ""
Write-Host "Done: $targetExe"
