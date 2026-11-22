# Install (or update) Bloom & Gather on Windows from the latest GitHub
# release, then start it. In PowerShell:
#
#   irm https://raw.githubusercontent.com/Vaios-Pechlevanidis/BLOOM-GATHER/main/scripts/install.ps1 | iex
#
# Downloads the installer (...-windows-x86_64-setup.exe) and runs it for the
# current user (no administrator rights needed): Start menu entry, uninstall
# via Settings > Apps. The installer is not code-signed yet; if SmartScreen
# asks, choose "More info" > "Run anyway".
#
# Options (environment variables): BLOOM_NO_LAUNCH=1 installs only;
# BLOOM_REPO=owner/name uses another fork's releases.
$ErrorActionPreference = "Stop"
$repo = if ($env:BLOOM_REPO) { $env:BLOOM_REPO } else { "Vaios-Pechlevanidis/BLOOM-GATHER" }

Write-Host "==> Looking up the latest release of $repo"
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -Headers @{ Accept = "application/vnd.github+json" }
$asset = $release.assets | Where-Object { $_.name -like "*-windows-x86_64-setup.exe" } | Select-Object -First 1
if (-not $asset) { throw "No Windows installer in the latest release (see https://github.com/$repo/releases)" }

$setup = Join-Path $env:TEMP $asset.name
Write-Host "==> Downloading $($asset.name)"
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $setup -UseBasicParsing

Write-Host "==> Installing"
# Per-user install, progress window only, no questions.
Start-Process -FilePath $setup -ArgumentList "/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER" -Wait
Remove-Item $setup -ErrorAction SilentlyContinue

$exe = Join-Path $env:LOCALAPPDATA "Programs\Bloom & Gather\bloom-and-gather.exe"
if (-not (Test-Path $exe)) { throw "Installation finished, but the game was not found at $exe" }
Write-Host "==> Installed $exe (Start menu: Bloom & Gather)"
if (-not $env:BLOOM_NO_LAUNCH) {
    Write-Host "==> Starting Bloom & Gather"
    Start-Process -FilePath $exe
}
