# RS-AI-DevOps installer for Windows.
# Honest scope: builds from source with cargo (no code-signed prebuilt binary
# yet), installs to $env:LOCALAPPDATA\rs-ai-devops, and writes a convenience
# launch script. Requires git and a Rust toolchain (installs rustup
# automatically if missing, via winget if available, otherwise prompts the
# user to install manually).

$ErrorActionPreference = "Stop"

$InstallDir = if ($env:RS_AI_DEVOPS_HOME) { $env:RS_AI_DEVOPS_HOME } else { Join-Path $env:LOCALAPPDATA "rs-ai-devops" }
$RepoUrl = "https://github.com/aon-co-jp/RS-AI-DevOps.git"

Write-Host "== RS-AI-DevOps installer (Windows) =="

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust toolchain not found."
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        Write-Host "Installing via winget (Rustlang.Rustup)..."
        winget install --id Rustlang.Rustup -e --source winget
    } else {
        Write-Host "Please install Rust manually from https://rustup.rs and re-run this script."
        exit 1
    }
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Host "git is required but was not found on PATH. Install Git for Windows first: https://git-scm.com/download/win"
    exit 1
}

if (-not (Test-Path $InstallDir)) {
    git clone $RepoUrl $InstallDir
} else {
    Write-Host "Existing install found at $InstallDir, pulling latest..."
    git -C $InstallDir pull --ff-only
}

Set-Location $InstallDir
cargo build --release

$ExePath = Join-Path $InstallDir "target\release\rs-ai-devops.exe"
Write-Host ""
Write-Host "Build complete: $ExePath"

$LaunchScript = Join-Path $InstallDir "run-rs-ai-devops.ps1"
@"
`$env:RS_AI_DEVOPS_PORT = "8097"
`$env:RS_AI_DEVOPS_SCAN_ROOT = "$env:USERPROFILE"
& "$ExePath"
"@ | Set-Content -Encoding utf8 $LaunchScript

Write-Host "Launch script written to $LaunchScript"
Write-Host "Run it with: powershell -ExecutionPolicy Bypass -File `"$LaunchScript`""
Write-Host "Then open http://127.0.0.1:8097/ in a browser."
Write-Host ""
Write-Host "Note: this does not register a Windows service. For always-on use,"
Write-Host "register $LaunchScript with Task Scheduler (run at log on) yourself,"
Write-Host "or run it in a terminal window when needed."
