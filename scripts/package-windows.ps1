# Windows packages: a zip and an installer (Inno Setup).
#   pwsh scripts/package-windows.ps1
# Needs the Rust MSVC toolchain and, for the installer, Inno Setup 6
# (iscc.exe on PATH or in its default folder). The game is not code-signed;
# SmartScreen may ask once ("More info" > "Run anyway").
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
New-Item -ItemType Directory -Force -Path dist | Out-Null

cargo build --profile dist
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$name = "Bloom-and-Gather-$version-windows-x86_64"
$stage = Join-Path dist $name
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item target\dist\bloom-and-gather.exe $stage
Copy-Item packaging\icon\icon.ico (Join-Path $stage "bloom-and-gather.ico")
Copy-Item README.md $stage
Compress-Archive -Path $stage -DestinationPath "dist\$name.zip" -Force
Write-Host "Built dist\$name.zip"

$iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) { $iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" }
if (Test-Path $iscc) {
    & $iscc "/DVersion=$version" packaging\windows\bloom-and-gather.iss
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed" }
    Write-Host "Built dist\$name-setup.exe"
} else {
    Write-Host "Inno Setup not found; skipping the installer"
}
