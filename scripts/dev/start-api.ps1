$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$apiDirectory = Join-Path $repoRoot "apps\api"
$envFile = Join-Path $apiDirectory ".env"

foreach ($tool in @("docker", "cargo")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "Required tool '$tool' was not found. Install Docker Desktop and the stable Rust toolchain, then reopen PowerShell."
    }
}

if (-not (Test-Path $envFile)) {
    Copy-Item (Join-Path $apiDirectory ".env.example") $envFile
    Write-Host "Created apps\api\.env from the example. OIDC values are placeholders; real sign-in will not work until you configure a real provider." -ForegroundColor Yellow
}

Set-Location $repoRoot
Write-Host "Starting local PostgreSQL (this does not touch a remote database)..." -ForegroundColor Cyan
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

# Load non-secret local environment values into this PowerShell process only.
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

Write-Host "Starting Your Dawnloader API at http://127.0.0.1:8080 ..." -ForegroundColor Green
Write-Host "SQLx migrations under crates\storage\migrations are applied automatically on startup." -ForegroundColor DarkGray
Write-Host "OIDC placeholders do not authenticate users. Press Ctrl+C to stop the API; PostgreSQL remains running." -ForegroundColor Yellow
cargo run -p dawn-api
if ($LASTEXITCODE -ne 0) {
    throw "The API stopped with an error. Read the message above; PostgreSQL logs: docker compose logs postgres"
}
