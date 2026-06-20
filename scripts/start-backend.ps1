$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $projectRoot

Write-Host "Starting Rust Web service: http://127.0.0.1:8000"
Write-Host "This single process serves both frontend pages and /api/orange/* APIs."

if (-not (Test-Path ".env")) {
    Copy-Item ".env.example" ".env"
    Write-Warning "Created .env from .env.example. Please configure database credentials and API keys if needed."
}

cargo run --release
