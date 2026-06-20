$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $projectRoot

Write-Host "Rust frontend is served by the Axum Web service."
Write-Host "Open after startup: http://127.0.0.1:8000"

if (-not (Test-Path ".env")) {
    Copy-Item ".env.example" ".env"
    Write-Warning "Created .env from .env.example. Please configure database credentials and API keys if needed."
}

cargo run --release
