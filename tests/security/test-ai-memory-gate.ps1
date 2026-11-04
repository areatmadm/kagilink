# Deterministic negative tests, no real network or private file access.
$ErrorActionPreference = 'Stop'
$gatePath = Join-Path $PSScriptRoot '..\..\tools\Test-AiMemoryAccess.ps1'
$gateCode = Get-Content -LiteralPath $gatePath -Raw
$scenarios = @('owner', 'other-user', 'network-failure', 'public-repo', 'wrong-owner', 'wrong-repo', 'no-access', 'invalid-json', 'string-private')
foreach ($scenario in $scenarios) {
    # Isolated runspace: mock gh and exit without affecting this test process.
    $runner = [PowerShell]::Create()
    try {
        $null = $runner.AddScript({ param($caseName, $source)
            $script:testCase = $caseName
            function gh {
                $global:LASTEXITCODE = 0
                if ($script:testCase -eq 'network-failure') { $global:LASTEXITCODE = 1; return }
                if ($args[1] -eq 'user') {
                    if ($script:testCase -eq 'other-user') { return 'someone-else' }
                    return 'areatmadm'
                }
                if ($script:testCase -eq 'invalid-json') { return '{invalid' }
                $record = @{full_name='areatmadm/pwreceiver-ai-memory'; owner=@{login='areatmadm'}; private=$true; permissions=@{pull=$true}}
                switch ($script:testCase) {
                    'public-repo' { $record.private = $false }
                    'wrong-owner' { $record.owner.login = 'other' }
                    'wrong-repo' { $record.full_name = 'areatmadm/other' }
                    'no-access' { $record.permissions.pull = $false }
                    'string-private' { $record.private = 'true' }
                }
                return $record | ConvertTo-Json -Depth 4
            }
            & ([scriptblock]::Create($source))
        }).AddArgument($scenario).AddArgument($gateCode)
        $output = ($runner.Invoke() | Out-String)
        $allowed = $output.Contains('AI_MEMORY_ACCESS_ALLOWED:')
        $denied = $output.Contains('AI_MEMORY_ACCESS_DENIED:')
        if ($runner.HadErrors -or $allowed -ne ($scenario -eq 'owner') -or $denied -eq $allowed) {
            throw "Gate test failed: $scenario"
        }
        Write-Output "PASS: $scenario"
    } finally { $runner.Dispose() }
}
