# Build script for AniDB Client Python bindings on Windows

$ErrorActionPreference = "Stop"

Write-Host "Building AniDB Client Python bindings..." -ForegroundColor Green

# Get script directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Join-Path $ScriptDir ".." ".."

# Check if in virtual environment
if (-not $env:VIRTUAL_ENV) {
    Write-Host "Warning: Not in a virtual environment. Consider using:" -ForegroundColor Yellow
    Write-Host "  python -m venv venv"
    Write-Host "  .\venv\Scripts\Activate.ps1"
    Write-Host ""
}

# Build Rust library
Write-Host "Building Rust library..." -ForegroundColor Green
Push-Location $ProjectRoot
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed"
    }
} finally {
    Pop-Location
}

# Copy library to Python package
Write-Host "Copying native library..." -ForegroundColor Green
$RustTargetDir = Join-Path $ProjectRoot "target" "release"
$PythonLibDir = Join-Path $ScriptDir "src" "anidb_client"

# Create directory if it doesn't exist
New-Item -ItemType Directory -Force -Path $PythonLibDir | Out-Null

# Copy DLL
$SourceDll = Join-Path $RustTargetDir "anidb_client_core.dll"
$DestDll = Join-Path $PythonLibDir "anidb_client_core.dll"

if (Test-Path $SourceDll) {
    Copy-Item $SourceDll $DestDll -Force
    Write-Host "Copied anidb_client_core.dll"
} else {
    Write-Host "Error: anidb_client_core.dll not found at $SourceDll" -ForegroundColor Red
    exit 1
}

# Install Python package in development mode
Write-Host "Installing Python package..." -ForegroundColor Green
Push-Location $ScriptDir
try {
    pip install -e .
    if ($LASTEXITCODE -ne 0) {
        throw "pip install failed"
    }
} finally {
    Pop-Location
}

# Install development dependencies if requested
if ($args -contains "--dev") {
    Write-Host "Installing development dependencies..." -ForegroundColor Green
    Push-Location $ScriptDir
    try {
        pip install -e ".[dev]"
    } finally {
        Pop-Location
    }
}

# Run tests if requested
if ($args -contains "--test") {
    Write-Host "Running tests..." -ForegroundColor Green
    Push-Location $ScriptDir
    try {
        pytest
    } finally {
        Pop-Location
    }
}

Write-Host "Build complete!" -ForegroundColor Green
Write-Host ""
Write-Host "To use the library:"
Write-Host "  from anidb_client import AniDBClient"
Write-Host ""
Write-Host "Run examples with:"
Write-Host "  python examples\basic_usage.py <file_path>"