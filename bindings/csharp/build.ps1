# PowerShell build script for AniDB C# bindings

param(
    [string]$Configuration = "Release",
    [switch]$RunTests,
    [switch]$Pack,
    [switch]$Clean
)

$ErrorActionPreference = "Stop"

# Get script directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SolutionFile = Join-Path $ScriptDir "AniDBClient.sln"
$OutputDir = Join-Path $ScriptDir "artifacts"

Write-Host "AniDB C# Bindings Build Script" -ForegroundColor Cyan
Write-Host "==============================" -ForegroundColor Cyan

# Clean if requested
if ($Clean) {
    Write-Host "`nCleaning previous builds..." -ForegroundColor Yellow
    
    if (Test-Path $OutputDir) {
        Remove-Item -Path $OutputDir -Recurse -Force
    }
    
    & dotnet clean $SolutionFile --configuration $Configuration
    
    if ($LASTEXITCODE -ne 0) {
        throw "Clean failed"
    }
}

# Restore dependencies
Write-Host "`nRestoring dependencies..." -ForegroundColor Yellow
& dotnet restore $SolutionFile

if ($LASTEXITCODE -ne 0) {
    throw "Restore failed"
}

# Build solution
Write-Host "`nBuilding solution ($Configuration)..." -ForegroundColor Yellow
& dotnet build $SolutionFile --configuration $Configuration --no-restore

if ($LASTEXITCODE -ne 0) {
    throw "Build failed"
}

# Run tests if requested
if ($RunTests) {
    Write-Host "`nRunning tests..." -ForegroundColor Yellow
    
    $TestProject = Join-Path $ScriptDir "src\AniDBClient.Tests\AniDBClient.Tests.csproj"
    
    & dotnet test $TestProject `
        --configuration $Configuration `
        --no-build `
        --logger "console;verbosity=normal" `
        --collect:"XPlat Code Coverage"
    
    if ($LASTEXITCODE -ne 0) {
        throw "Tests failed"
    }
}

# Create NuGet package if requested
if ($Pack) {
    Write-Host "`nCreating NuGet package..." -ForegroundColor Yellow
    
    $ProjectFile = Join-Path $ScriptDir "src\AniDBClient\AniDBClient.csproj"
    
    # Ensure output directory exists
    if (!(Test-Path $OutputDir)) {
        New-Item -ItemType Directory -Path $OutputDir | Out-Null
    }
    
    & dotnet pack $ProjectFile `
        --configuration $Configuration `
        --no-build `
        --output $OutputDir
    
    if ($LASTEXITCODE -ne 0) {
        throw "Pack failed"
    }
    
    Write-Host "`nPackage created in: $OutputDir" -ForegroundColor Green
}

# Copy native libraries
Write-Host "`nCopying native libraries..." -ForegroundColor Yellow

$NativeLibsSource = Join-Path $ScriptDir "..\..\target\release"
$RuntimesDir = Join-Path $ScriptDir "src\AniDBClient\runtimes"

# Create runtime directories
$Runtimes = @(
    @{ Platform = "win-x64"; File = "anidb_client_core.dll" }
    @{ Platform = "linux-x64"; File = "libanidb_client_core.so" }
    @{ Platform = "osx-x64"; File = "libanidb_client_core.dylib" }
    @{ Platform = "osx-arm64"; File = "libanidb_client_core.dylib" }
)

foreach ($runtime in $Runtimes) {
    $runtimeDir = Join-Path $RuntimesDir "$($runtime.Platform)\native"
    
    if (!(Test-Path $runtimeDir)) {
        New-Item -ItemType Directory -Path $runtimeDir -Force | Out-Null
    }
    
    $sourceFile = Join-Path $NativeLibsSource $runtime.File
    
    if (Test-Path $sourceFile) {
        $destFile = Join-Path $runtimeDir $runtime.File
        Copy-Item -Path $sourceFile -Destination $destFile -Force
        Write-Host "  Copied $($runtime.File) to $($runtime.Platform)" -ForegroundColor Gray
    }
}

Write-Host "`nBuild completed successfully!" -ForegroundColor Green

# Show summary
Write-Host "`nSummary:" -ForegroundColor Cyan
Write-Host "  Configuration: $Configuration" -ForegroundColor Gray
Write-Host "  Tests Run: $($RunTests -eq $true)" -ForegroundColor Gray
Write-Host "  Package Created: $($Pack -eq $true)" -ForegroundColor Gray

if ($Pack) {
    Get-ChildItem -Path $OutputDir -Filter "*.nupkg" | ForEach-Object {
        Write-Host "  Package: $($_.Name)" -ForegroundColor Gray
    }
}