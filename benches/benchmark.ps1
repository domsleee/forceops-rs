#!/usr/bin/env pwsh
# Benchmark script comparing fops (Rust) vs forceops (C#)
# Uses hyperfine for accurate timing

param(
    [int]$NumFiles = 100,
    [int]$FileSize = 1024,
    [int]$Warmup = 3,
    [int]$Runs = 10
)

$ErrorActionPreference = "Stop"

# Create temp directory for benchmarks
$benchDir = Join-Path $env:TEMP "fops-bench-$(New-Guid)"
New-Item -ItemType Directory -Path $benchDir -Force | Out-Null

function Setup-TestDirectory {
    param([string]$Path)
    
    if (Test-Path $Path) {
        Remove-Item -Path $Path -Recurse -Force
    }
    New-Item -ItemType Directory -Path $Path -Force | Out-Null
    
    # Create test files
    $content = [byte[]]::new($FileSize)
    for ($i = 0; $i -lt $NumFiles; $i++) {
        $fileName = Join-Path $Path ("{0:D8}.txt" -f $i)
        [System.IO.File]::WriteAllBytes($fileName, $content)
    }
}

Write-Host "Benchmark: fops (Rust) vs forceops (C#)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Files: $NumFiles, Size: $FileSize bytes each"
Write-Host "Warmup: $Warmup, Runs: $Runs"
Write-Host ""

# Test 1: Delete flat directory with files
Write-Host "Test 1: Delete directory with $NumFiles files" -ForegroundColor Yellow
$testDir = Join-Path $benchDir "test1"

hyperfine --warmup $Warmup --runs $Runs `
    --prepare "pwsh -c `"& { `$content = [byte[]]::new($FileSize); New-Item -ItemType Directory -Path '$testDir' -Force | Out-Null; for (`$i = 0; `$i -lt $NumFiles; `$i++) { [System.IO.File]::WriteAllBytes((Join-Path '$testDir' ('{0:D8}.txt' -f `$i)), `$content) } }`"" `
    --cleanup "if (Test-Path '$testDir') { Remove-Item '$testDir' -Recurse -Force }" `
    "fops rm '$testDir'" `
    "forceops rm '$testDir'"

Write-Host ""

# Test 2: Delete nested directories
Write-Host "Test 2: Delete nested directory structure ($NumFiles subdirs)" -ForegroundColor Yellow
$testDir2 = Join-Path $benchDir "test2"

hyperfine --warmup $Warmup --runs $Runs `
    --prepare "pwsh -c `"& { `$content = [byte[]]::new($FileSize); New-Item -ItemType Directory -Path '$testDir2' -Force | Out-Null; for (`$i = 0; `$i -lt $NumFiles; `$i++) { `$subdir = Join-Path '$testDir2' ('{0:D8}' -f `$i); New-Item -ItemType Directory -Path `$subdir -Force | Out-Null; [System.IO.File]::WriteAllBytes((Join-Path `$subdir 'file.txt'), `$content) } }`"" `
    --cleanup "if (Test-Path '$testDir2') { Remove-Item '$testDir2' -Recurse -Force }" `
    "fops rm '$testDir2'" `
    "forceops rm '$testDir2'"

Write-Host ""

# Test 3: Startup time (--help)
Write-Host "Test 3: Startup time (--help)" -ForegroundColor Yellow

hyperfine --warmup $Warmup --runs 50 `
    "fops --help" `
    "forceops --help"

Write-Host ""

# Test 4: Delete single file
Write-Host "Test 4: Delete single file" -ForegroundColor Yellow
$testFile = Join-Path $benchDir "single-file.txt"

hyperfine --warmup $Warmup --runs $Runs `
    --prepare "[System.IO.File]::WriteAllBytes('$testFile', [byte[]]::new(1024))" `
    "fops rm '$testFile'" `
    "forceops rm '$testFile'"

# Cleanup
Remove-Item -Path $benchDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "Benchmark complete!" -ForegroundColor Green
