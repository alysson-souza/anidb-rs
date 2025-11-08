# PowerShell build script for Windows
param(
    [switch]$SkipTests = $false,
    [switch]$Debug = $false
)

Write-Host "Building AniDB Client for Node.js..." -ForegroundColor Cyan
Write-Host "====================================" -ForegroundColor Cyan

# Check if we're in the right directory
if (-not (Test-Path "package.json")) {
    Write-Host "Error: package.json not found. Please run from the nodejs bindings directory." -ForegroundColor Red
    exit 1
}

# Function to check if a command exists
function Test-CommandExists {
    param($Command)
    $null = Get-Command $Command -ErrorAction SilentlyContinue
    return $?
}

# Check prerequisites
Write-Host "`nChecking prerequisites..." -ForegroundColor Yellow

if (-not (Test-CommandExists "node")) {
    Write-Host "Error: Node.js is not installed" -ForegroundColor Red
    exit 1
}

if (-not (Test-CommandExists "npm")) {
    Write-Host "Error: npm is not installed" -ForegroundColor Red
    exit 1
}

if (-not (Test-CommandExists "python") -and -not (Test-CommandExists "python3")) {
    Write-Host "Warning: Python not found. It may be required for node-gyp" -ForegroundColor Yellow
}

$nodeVersion = node -v
Write-Host "Node.js version: $nodeVersion" -ForegroundColor Green

# Check Node.js version (14.0.0 minimum)
$nodeMajor = [int]($nodeVersion -replace 'v(\d+)\..*', '$1')
if ($nodeMajor -lt 14) {
    Write-Host "Error: Node.js 14.0.0 or higher is required" -ForegroundColor Red
    exit 1
}

# Build configuration
$buildType = if ($Debug) { "Debug" } else { "Release" }
$rustTarget = if ($Debug) { "debug" } else { "release" }

# Build the Rust library first if needed
$rustLibPath = "..\..\target\$rustTarget\anidb_client_core.lib"
if (-not (Test-Path $rustLibPath)) {
    Write-Host "`nRust library not found. Building..." -ForegroundColor Yellow
    Push-Location ..\..
    
    if ($Debug) {
        cargo build
    } else {
        cargo build --release
    }
    
    Pop-Location
    
    if (-not (Test-Path $rustLibPath)) {
        Write-Host "Error: Failed to build Rust library" -ForegroundColor Red
        exit 1
    }
}

Write-Host "✓ Rust library found" -ForegroundColor Green

# Clean previous builds
Write-Host "`nCleaning previous builds..." -ForegroundColor Yellow
if (Test-Path "build") { Remove-Item -Recurse -Force "build" }
if (Test-Path "dist") { Remove-Item -Recurse -Force "dist" }
if (Test-Path "node_modules") { Remove-Item -Recurse -Force "node_modules" }

# Install dependencies
Write-Host "`nInstalling dependencies..." -ForegroundColor Yellow
npm install

if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Failed to install dependencies" -ForegroundColor Red
    exit 1
}

# Build native module
Write-Host "`nBuilding native module ($buildType)..." -ForegroundColor Yellow
if ($Debug) {
    npm run build:native -- --debug
} else {
    npm run build:native
}

if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Native module build failed" -ForegroundColor Red
    exit 1
}

# Build TypeScript
Write-Host "`nBuilding TypeScript..." -ForegroundColor Yellow
npm run build:ts

if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: TypeScript build failed" -ForegroundColor Red
    exit 1
}

# Run tests
if (-not $SkipTests) {
    Write-Host "`nRunning tests..." -ForegroundColor Yellow
    npm test
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "`n✓ Build completed successfully!" -ForegroundColor Green
    } else {
        Write-Host "`nWarning: Tests failed but build completed" -ForegroundColor Yellow
    }
} else {
    Write-Host "`n✓ Build completed successfully! (tests skipped)" -ForegroundColor Green
}

# Display package info
Write-Host "`nPackage information:" -ForegroundColor Cyan
Write-Host "===================" -ForegroundColor Cyan
npm list --depth=0

Write-Host "`nBuild artifacts:" -ForegroundColor Cyan
Write-Host "Native module: build\$buildType\anidb_client.node" -ForegroundColor White
Write-Host "TypeScript output: dist\" -ForegroundColor White

Write-Host "`nYou can now:" -ForegroundColor Green
Write-Host "  - Run examples: npm run example:basic" -ForegroundColor White
Write-Host "  - Use in your project: const { AniDBClient } = require('./dist')" -ForegroundColor White