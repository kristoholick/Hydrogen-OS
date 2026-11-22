# Helper: run a command inside the Ubuntu build environment, from the repo root.
# Usage (PowerShell):  .\tools\wsl.ps1 make test
$env:WSL_UTF8 = 1
$cmd = ($args -join ' ')
wsl -d Ubuntu --cd "/mnt/c/codeplayer/Windows 12" -- bash -lc ". ~/.cargo/env; $cmd"
exit $LASTEXITCODE
