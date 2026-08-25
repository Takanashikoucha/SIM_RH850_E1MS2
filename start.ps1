# RH850/E1M-S2 simulator — one-command launcher (Windows PowerShell).
#
#   .\start.ps1          # serve the prebuilt app with Python 3 (no Node.js needed)
#   .\start.ps1 -Port 9000
#   .\start.ps1 -Dev     # development mode (Vite hot reload; requires Node.js)
#
param(
    [int]$Port = 8080,
    [switch]$Dev
)

$ErrorActionPreference = "Stop"
Set-Location -Path $PSScriptRoot

if ($Dev) {
    # ---- Development mode: Vite + hot reload (requires Node.js + npm) ----
    try { $null = Get-Command npm -ErrorAction Stop } catch {
        Write-Error "--dev mode requires Node.js/npm. Install from https://nodejs.org/, or run without -Dev for the prebuilt app."
    }
    if (-not (Test-Path "web\node_modules")) {
        Write-Host "==> Installing frontend dependencies (first run)..."
        Push-Location web; npm install; Pop-Location
    }
    Write-Host "==> Development server at http://127.0.0.1:$Port (Ctrl+C to stop)"
    npm run dev --prefix web -- --port $Port
    exit $LASTEXITCODE
}

# ---- Static mode: prebuilt app via Python 3 ----
if (-not (Test-Path "web\dist")) {
    Write-Error "web\dist\ not found (prebuilt app missing). Build it with: npm install --prefix web; npm run build --prefix web — or use dev mode: .\start.ps1 -Dev"
}

$py = Get-Command python3 -ErrorAction SilentlyContinue
if (-not $py) { $py = Get-Command python -ErrorAction SilentlyContinue }
if (-not $py) {
    Write-Error "Python 3 not found. Install it, or use dev mode: .\start.ps1 -Dev"
}

& $py.Source "serve.py" $Port
