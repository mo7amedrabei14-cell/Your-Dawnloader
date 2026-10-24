$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$websiteDirectory = Join-Path $repoRoot "apps\website"

if (-not (Get-Command node -ErrorAction SilentlyContinue) -or -not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw "Node.js 22 or newer and npm are required. Install Node.js, reopen PowerShell, and rerun this script."
}

$nodeVersion = (node -p "Number(process.versions.node.split('.')[0])").Trim()
if ([int]$nodeVersion -lt 22) {
    throw "This project uses Node.js 22 or newer. Your current major version is $nodeVersion."
}

Set-Location $websiteDirectory
if (-not (Test-Path ".env.local")) {
    Copy-Item ".env.example" ".env.local"
    Write-Host "Created .env.local for local website development. It points to the API at 127.0.0.1:8080." -ForegroundColor Yellow
}

if (-not (Test-Path "node_modules")) {
    Write-Host "Installing website dependencies..." -ForegroundColor Cyan
    npm install --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { throw "npm install failed." }
}

Write-Host "Starting website at http://127.0.0.1:3000 ..." -ForegroundColor Green
Write-Host "This script starts only the website. Run scripts\dev\start-api.ps1 in another PowerShell window for the API." -ForegroundColor DarkGray
npm run dev
if ($LASTEXITCODE -ne 0) { throw "The website development server stopped with an error." }
