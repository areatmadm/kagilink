<#
.SYNOPSIS
  TEST-ONLY. Run INSIDE the isolated Phase 5 RDP test VM console (elevated PowerShell of the throwaway VM, never on the host).
  Enables logon auditing and lists/compares Security log logon events as server-side evidence for
  docs/PHASE5_RDP_INPUT_FEASIBILITY.md section 3.2. Passwords never appear in these events and are not handled here.

.DESCRIPTION
  Default mode: prints exactly what will change, asks y/n, then enables Success+Failure auditing of the "Logon" subcategory
  with auditpol. Nothing else is changed.

  -Report: lists events 4624 (successful logon, with logon type) and 4625 (failed logon) from the last -Minutes minutes with
  UTC time, source IP and account name only. With -RunsFile <harness CSV> it also applies the comparison rule:
    * every Positive run that sent its input must match EXACTLY ONE counted 4624 within 30 s after its UTC time;
    * every NegativeControl run that sent its input must match EXACTLY ONE 4625 within 30 s;
    * a run whose input was not sent (EventsSent 0 or no InputAttempted) must have NO event from the host in its window;
    * no event from the host IP may exist outside the run windows ("nothing else in between");
    * any mismatch is FAIL; one mismatch fails the whole study.
  Counted 4624 events are those with a LogonType in -CountLogonTypes (default 10, RemoteInteractive). With NLA a network logon
  (type 3) usually accompanies the session logon; such events are listed and flagged "ignored(type)", never silently dropped.
  Check the first calibration run by eye and report the observed pattern to the owner before a 50-run series.
  Clock: the VM and host clocks must agree within a few seconds; use -ClockSkewSeconds (VM minus host) if they do not.
#>
[CmdletBinding()]
param(
    [switch]$Report,
    [ValidateRange(1, 1440)][int]$Minutes = 30,
    [string]$RunsFile,
    [string]$SourceIp = '192.168.77.1',
    [int[]]$CountLogonTypes = @(10),
    [string]$Account,
    [int]$ClockSkewSeconds = 0,
    [ValidateRange(5, 120)][int]$WindowSeconds = 30
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$LogonGuid = '{0CCE9215-69AE-11D9-BED3-505054503030}'   # Audit Logon subcategory (locale independent)

$id = [Security.Principal.WindowsIdentity]::GetCurrent()
if (-not (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this inside the test VM in an elevated PowerShell (reading the Security log needs it).'
}

if (-not $Report) {
    Write-Host 'This will change ONE setting on THIS machine (it must be the throwaway test VM):'
    Write-Host ('  auditpol /set /subcategory:"' + $LogonGuid + '" /success:enable /failure:enable')
    Write-Host '  (Advanced Audit Policy > Logon/Logoff > Audit Logon: Success and Failure; current value shown below)'
    & auditpol.exe /get /subcategory:$LogonGuid
    $a = Read-Host 'Apply this change? (y/n)'
    if ($a -ne 'y') { Write-Host 'No change made.'; exit 0 }
    & auditpol.exe /set /subcategory:$LogonGuid /success:enable /failure:enable
    if ($LASTEXITCODE -ne 0) { throw 'auditpol failed' }
    & auditpol.exe /get /subcategory:$LogonGuid
    Write-Host ('VM UTC now: ' + [DateTime]::UtcNow.ToString('o') + ' (compare with the host before the study)')
    exit 0
}

Write-Host ('VM UTC now: ' + [DateTime]::UtcNow.ToString('o'))
$since = (Get-Date).AddMinutes(-$Minutes)
try {
    $raw = @(Get-WinEvent -FilterHashtable @{ LogName = 'Security'; Id = 4624, 4625; StartTime = $since } -ErrorAction Stop)
} catch {
    if ($_.Exception.Message -like '*No events were found*') { $raw = @() } else { throw }
}

$events = foreach ($e in $raw) {
    $x = [xml]$e.ToXml()
    $d = @{}
    foreach ($n in $x.Event.EventData.Data) { $d[$n.Name] = $n.'#text' }
    $lt = 0
    if ($d.ContainsKey('LogonType')) { [void][int]::TryParse($d['LogonType'], [ref]$lt) }
    [pscustomobject]@{
        UtcTime   = $e.TimeCreated.ToUniversalTime()
        EventId   = $e.Id
        LogonType = $lt
        SourceIp  = $d['IpAddress']
        Account   = $d['TargetUserName']
    }
}
$events = @($events | Sort-Object UtcTime)
if (-not [string]::IsNullOrEmpty($Account)) { $events = @($events | Where-Object { $_.Account -eq $Account }) }

Write-Host ''
Write-Host 'Security events (4624 success, 4625 failure); fields: UTC time, id, logon type, source IP, account only'
$events | ForEach-Object { [pscustomobject]@{ UtcTime = $_.UtcTime.ToString('yyyy-MM-ddTHH:mm:ss.fffZ'); EventId = $_.EventId; LogonType = $_.LogonType; SourceIp = $_.SourceIp; Account = $_.Account } } | Format-Table -AutoSize | Out-String | Write-Host

if ([string]::IsNullOrEmpty($RunsFile)) { exit 0 }

# --- comparison ----------------------------------------------------------------------------------------------------
if (-not (Test-Path -LiteralPath $RunsFile)) { throw 'RunsFile not found.' }
$runs = @(Import-Csv -LiteralPath $RunsFile)
$fromHost = @($events | Where-Object { $_.SourceIp -eq $SourceIp })
$used = New-Object 'System.Collections.Generic.HashSet[int]'
$fail = 0
$out = New-Object System.Collections.Generic.List[object]

foreach ($r in $runs) {
    $t = [DateTime]::Parse($r.UtcTime, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::AdjustToUniversal -bor [Globalization.DateTimeStyles]::AssumeUniversal).AddSeconds($ClockSkewSeconds)
    $sent = ([int]$r.EventsSent -gt 0) -and ($r.Outcome -like 'InputAttempted*')
    $lo = $t.AddSeconds(-2)
    $hi = $t.AddSeconds($WindowSeconds)
    $inWin = @()
    for ($i = 0; $i -lt $fromHost.Count; $i++) {
        if ($fromHost[$i].UtcTime -ge $lo -and $fromHost[$i].UtcTime -le $hi) { $inWin += , $i }
    }
    $ok4624 = @($inWin | Where-Object { $fromHost[$_].EventId -eq 4624 -and $CountLogonTypes -contains $fromHost[$_].LogonType })
    $ign4624 = @($inWin | Where-Object { $fromHost[$_].EventId -eq 4624 -and -not ($CountLogonTypes -contains $fromHost[$_].LogonType) })
    $f4625 = @($inWin | Where-Object { $fromHost[$_].EventId -eq 4625 })
    foreach ($i in $inWin) { [void]$used.Add($i) }

    $verdict = 'FAIL'
    if (-not $sent) {
        if ($ok4624.Count -eq 0 -and $f4625.Count -eq 0) { $verdict = 'PASS(no input sent, no event)' }
    } elseif ($r.Type -eq 'Positive') {
        if ($ok4624.Count -eq 1 -and $f4625.Count -eq 0) { $verdict = 'PASS' }
    } elseif ($r.Type -eq 'NegativeControl') {
        if ($f4625.Count -eq 1 -and $ok4624.Count -eq 0) { $verdict = 'PASS' }
    }
    if ($verdict -eq 'FAIL') { $fail++ }
    $out.Add([pscustomobject]@{ Run = $r.Run; Type = $r.Type; Mode = $r.Mode; Sent = $sent; Counted4624 = $ok4624.Count; Ignored4624 = $ign4624.Count; Event4625 = $f4625.Count; Verdict = $verdict })
}

$extra = @()
for ($i = 0; $i -lt $fromHost.Count; $i++) { if (-not $used.Contains($i)) { $extra += $fromHost[$i] } }

$out | Format-Table -AutoSize | Out-String | Write-Host
if ($extra.Count -gt 0) {
    Write-Host ('FAIL: ' + $extra.Count + ' event(s) from ' + $SourceIp + ' outside every run window:')
    $extra | ForEach-Object { Write-Host ('  ' + $_.UtcTime.ToString('yyyy-MM-ddTHH:mm:ss.fffZ') + ' id ' + $_.EventId + ' type ' + $_.LogonType + ' account ' + $_.Account) }
    $fail++
}
if ($fail -eq 0) { Write-Host 'RESULT: all runs consistent with the rule. This is evidence for these runs only; see the 50-run requirement.' }
else { Write-Host ('RESULT: FAIL (' + $fail + ' mismatch group(s)). No "supported" claim may be made.'); exit 1 }
