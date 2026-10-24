$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$apiDirectory = Join-Path $repoRoot "apps\api"
$envFile = Join-Path $apiDirectory ".env"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Cargo was not found. Install the stable Rust toolchain, then reopen PowerShell."
}

if (-not (Test-Path $envFile)) {
    Copy-Item (Join-Path $apiDirectory ".env.example") $envFile
    Write-Host "Created apps\api\.env from the example. OIDC values are placeholders; real sign-in will not work until you configure a real provider." -ForegroundColor Yellow
}

# Load the API configuration locally without printing connection strings or secrets.
Get-Content $envFile | ForEach-Object {
    $line = $_.Trim()
    if ($line -and -not $line.StartsWith("#") -and $line.Contains("=")) {
        $parts = $line -split "=", 2
        $name = $parts[0].Trim()
        $value = $parts[1].Trim()
        if (($value.StartsWith('"') -and $value.EndsWith('"')) -or ($value.StartsWith("'") -and $value.EndsWith("'"))) {
            $value = $value.Substring(1, $value.Length - 2)
        }
        if ($name -match '^[A-Za-z_][A-Za-z0-9_]*$') {
            [Environment]::SetEnvironmentVariable($name, $value, "Process")
        }
    }
}

$databaseUrl = [Environment]::GetEnvironmentVariable("DATABASE_URL", "Process")
if ([string]::IsNullOrWhiteSpace($databaseUrl)) {
    throw "DATABASE_URL is missing in apps\api\.env."
}

$isLocalDatabase = $databaseUrl -match '^postgres(?:ql)?://(?:[^/@]+@)?(?:localhost|127\.0\.0\.1|\[::1\])(?::\d+)?(?:/|$)'
if ($isLocalDatabase) {
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        throw "Docker was not found. Install Docker Desktop, or set DATABASE_URL to your existing PostgreSQL database in apps\api\.env."
    }

    Set-Location $repoRoot
    Write-Host "Starting local PostgreSQL (only the local container is affected)..." -ForegroundColor Cyan
    docker compose up -d postgres
    if ($LASTEXITCODE -ne 0) {
        throw "Could not start PostgreSQL. Open Docker Desktop and ensure its engine is running."
    }

    $ready = $false
    for ($attempt = 1; $attempt -le 30; $attempt++) {
        docker compose exec -T postgres pg_isready -U dawn -d dawnloader *> $null
        if ($LASTEXITCODE -eq 0) {
            $ready = $true
            break
        }
        Start-Sleep -Seconds 2
    }
    if (-not $ready) {
        throw "PostgreSQL did not become ready in 60 seconds. Check 'docker compose logs postgres'."
    }
} else {
    Write-Host "Using the PostgreSQL server configured in apps\api\.env. This script will not start or alter a local database container." -ForegroundColor Yellow
}

Set-Location $repoRoot
Write-Host "Starting Your Dawnloader API at http://127.0.0.1:8080 ..." -ForegroundColor Green
Write-Host "SQLx migrations under crates\storage\migrations are applied automatically on startup." -ForegroundColor DarkGray
Write-Host "OIDC placeholders do not authenticate users. Press Ctrl+C to stop the API." -ForegroundColor Yellow
cargo run -p dawn-api
if ($LASTEXITCODE -ne 0) {
    throw "The API stopped with an error. Read the message above; PostgreSQL logs for the local container: docker compose logs postgres"
}
