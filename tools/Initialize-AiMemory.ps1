$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Test-AiMemoryAccess.ps1')
if ($LASTEXITCODE -ne 0) { exit 1 }
# Access is verified before any private-directory inspection.
$memoryPath = Join-Path $projectRoot 'shared\ai'
if (Test-Path -LiteralPath $memoryPath) {
    if (-not (Test-Path -LiteralPath (Join-Path $memoryPath '.git') -PathType Container)) {
        throw 'Memory directory exists without an independent Git repository; preserve files and migrate manually.'
    }
    $memoryTop = & git -C $memoryPath rev-parse --show-toplevel
    if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($memoryTop) -ne $memoryPath) {
        throw 'Memory Git root mismatch.'
    }
    $memoryRemote = & git -C $memoryPath remote get-url origin
    if ($LASTEXITCODE -ne 0 -or $memoryRemote -cnotin @(
        'https://github.com/areatmadm/pwreceiver-ai-memory.git',
        'git@github.com:areatmadm/pwreceiver-ai-memory.git')) {
        throw 'Unexpected private memory remote; no files were read.'
    }
    Write-Output 'Independent private memory repository verified; pull/commit separately after checking local edits.'
    exit 0
}
& git clone -- https://github.com/areatmadm/pwreceiver-ai-memory.git $memoryPath
if ($LASTEXITCODE -ne 0) { throw 'Private memory clone failed.' }
