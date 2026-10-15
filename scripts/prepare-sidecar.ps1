# Build and stage the native Windows daemon for Tauri.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $hostLine = rustc -vV | Where-Object { $_.StartsWith('host: ') }
    if ($LASTEXITCODE -ne 0 -or !$hostLine) { throw 'Cannot determine Rust host triple' }
    $triple = $hostLine.Substring(6)
    if (!$triple.EndsWith('windows-msvc')) { throw 'Use a native Windows MSVC Rust toolchain' }
    # Build settings are FLOTILLA_*; the HERMES_* names from before the
    # rename still count when the new one is unset.
    foreach ($name in 'TEAM_ID', 'DEV_BUILD', 'RELEASE_BUILD') {
        $old = [Environment]::GetEnvironmentVariable("HERMES_$name")
        if (-not [Environment]::GetEnvironmentVariable("FLOTILLA_$name") -and $old) {
            [Environment]::SetEnvironmentVariable("FLOTILLA_$name", $old)
        }
    }
    # Where to build, and then copy from, in this one step (H-029, CE-013
    # G2). A release (no FLOTILLA_DEV_BUILD, or FLOTILLA_RELEASE_BUILD=1 as
    # build-nsis.ps1 sets), as in prepare-sidecar.sh, never trusts
    # $env:CARGO_TARGET_DIR: it builds into this checkout's own target, which
    # no other bot may write. A dev build uses the session's target.
    $release = (-not $env:FLOTILLA_DEV_BUILD) -or ($env:FLOTILLA_RELEASE_BUILD -eq '1')
    $target = if (-not $release -and $env:CARGO_TARGET_DIR) {
        $env:CARGO_TARGET_DIR
    } else {
        Join-Path $repoRoot 'target'
    }
    cargo build --release --locked -p flotillad --target-dir $target
    if ($LASTEXITCODE -ne 0) { throw 'Daemon build failed' }
    $destination = Join-Path $repoRoot 'apps/desktop/src-tauri/binaries'
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -LiteralPath (Join-Path $target 'release/flotillad.exe') -Destination (Join-Path $destination "flotillad-$triple.exe")
    Write-Output "Staged flotillad-$triple.exe"
} finally {
    Pop-Location
}
