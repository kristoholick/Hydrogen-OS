# Helper: run a command inside the Ubuntu build environment, from the repo root.
# Usage (PowerShell):  .\tools\wsl.ps1 make test
# The repo root is found from this script's own location, so it works from any clone or worktree.
$env:WSL_UTF8 = 1
$root = Split-Path -Parent $PSScriptRoot
$cmd = ($args -join ' ')
wsl -d Ubuntu --cd "$root" -- bash -lc ". ~/.cargo/env; $cmd"
exit $LASTEXITCODE
