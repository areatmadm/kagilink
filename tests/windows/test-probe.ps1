param([string]$ProbePath = (Join-Path $PSScriptRoot '..\..\target\phase0\kagilink-phase0-probe.exe'))
$ErrorActionPreference = 'Stop'
$ProbePath = (Resolve-Path -LiteralPath $ProbePath).Path
function Assert-Invocation([string[]]$Arguments, [int]$ExpectedExit, [string]$Pattern, [string]$ForbiddenPattern = '') {
    $output = (& $ProbePath @Arguments) -join "`n"
    if ($LASTEXITCODE -ne $ExpectedExit -or ($Pattern -and $output -notmatch $Pattern) -or
        ($ForbiddenPattern -and $output -match $ForbiddenPattern)) {
        throw "Probe regression: $($Arguments -join ' ')"
    }
    Write-Output "PASS: $($Arguments -join ' ')"
}
Assert-Invocation @('--help') 0 '\-\-ble-test'
Assert-Invocation @('--version') 0 'KagiLink 0\.0\.0-phase0'
Assert-Invocation @('--policy') 0 'REAL_SECRET_TRANSFER_DISABLED\r?\nRDP_INPUT_UNSUPPORTED'
# Extra arguments must never activate scan/input or accept arbitrary payloads.
$rejection = 'REFUSED_NOT_NORMAL_MEDIUM_INTEGRITY|Usage:'
$executionMarkers = 'PHASE0_PUBLIC_UNAUTHENTICATED_BLE_TEST|SCANNING_DIAGNOSTIC_SERVICE|OWNED_EDIT_SYNTHETIC|NO_CREDENTIAL_UI_OR_RDP_SUPPORT_PROVEN'
Assert-Invocation @('--ble-test', 'SYNTHETIC-EXTRA') 2 $rejection $executionMarkers
Assert-Invocation @('--self-input', 'SYNTHETIC-EXTRA') 2 $rejection $executionMarkers
Assert-Invocation @('--unknown') 2 $rejection $executionMarkers
# No radio scan, foreground changes or input injection is performed by this script.
