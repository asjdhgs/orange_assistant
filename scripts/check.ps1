$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $projectRoot

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test

$rustLines = Get-ChildItem "src" -Recurse -Filter "*.rs" |
    Get-Content |
    Measure-Object -Line

Write-Host "Rust source lines: $($rustLines.Lines)"
if ($rustLines.Lines -lt 3000 -or $rustLines.Lines -gt 6000) {
    throw "Rust source size is outside the recommended 3000-6000 line range."
}
