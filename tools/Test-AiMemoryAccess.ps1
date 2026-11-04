# Source-side bootstrap. Must not inspect private files or print auth secrets.
$ErrorActionPreference = 'Stop'
try {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw 'GitHub CLI unavailable' }
    $identityText = & gh api user --jq .login 2>$null
    if ($LASTEXITCODE -ne 0 -or ($identityText -join "`n").Trim() -cne 'areatmadm') {
        throw 'Owner identity not verified'
    }
    $repoText = & gh api repos/areatmadm/pwreceiver-ai-memory 2>$null
    if ($LASTEXITCODE -ne 0) { throw 'Private repository access not verified' }
    $repo = ($repoText -join "`n") | ConvertFrom-Json
    if ($repo.full_name -cne 'areatmadm/pwreceiver-ai-memory' -or
        $repo.owner.login -cne 'areatmadm' -or
        $repo.private -isnot [bool] -or $repo.private -ne $true -or
        $repo.permissions.pull -isnot [bool] -or $repo.permissions.pull -ne $true) {
        throw 'Expected private repository not verified'
    }
    Write-Output 'AI_MEMORY_ACCESS_ALLOWED: authenticated areatmadm, expected private repository verified.'
    exit 0
} catch {
    # Do not include external errors: they can contain private URLs or diagnostics.
    Write-Output 'AI_MEMORY_ACCESS_DENIED: skip all private memory; continue with public source documentation.'
    exit 1
}
