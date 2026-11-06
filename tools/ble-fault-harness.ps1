<#
.SYNOPSIS
  TEST-ONLY harness: runs the Android fault-injection series from the PC (synthetic codes only).

.DESCRIPTION
  For each scenario it starts the receiver (`ble-receive --accept --seconds 6`) as a child process, reads the
  6-digit synthetic code from its output, sends it to the debug test APK with an adb broadcast, answers the
  receiver's pick prompt with the phone's KL-XXXX tag (read from logcat) and compares the receiver result and
  failure budget with the expectation. The code lifetime (60 s) is a security policy and is never changed or
  worked around. The code is never printed or written to a file (masked). It is sent to the phone through the
  adb shell's STDIN (not as an adb argument), so it is not in the PC process list or in the device logcat.

  Compatible with Windows PowerShell 5.1 and pwsh 7. Needs: phone connected by USB with debugging authorised
  and the debug test APK installed, adb on PATH (or -Adb), the receiver built. See docs/WINDOWS_TEST_STATION.md.

.PARAMETER Scenario        Comma list of scenario names or 'all' (default).
.PARAMETER SkipRepeat      Leave out repeat-b3.
.PARAMETER Serial          adb device serial (passed as -s). Never written to the report.
.PARAMETER Adb             adb executable (default: adb).
.PARAMETER ReceiverExe     Receiver executable (default: target\debug\areatm-kagilink-receiver.exe).
.PARAMETER WaitBetweenSeconds  Pause between scenarios only (default 5), never inside a code lifetime.
.PARAMETER OutFile         Markdown report (default: ble-fault-report.md in the current directory).
.PARAMETER PickByRow       Answer the pick prompt with the printed row number instead of the tag.
.PARAMETER RawLog          Append the receiver's raw stdout of every scenario to this file (the 6-digit code is masked,
                           tags are NOT masked because tags are public; for diagnosing list/pick problems).
.PARAMETER SelfTest        Run the parser self test only (no adb, no phone, no receiver).
#>
[CmdletBinding()]
param(
    [string[]]$Scenario = @('all'),
    [switch]$SkipRepeat,
    [string]$Serial = '',
    [string]$Adb = 'adb',
    [string]$ReceiverExe = 'target\debug\areatm-kagilink-receiver.exe',
    [int]$WaitBetweenSeconds = 5,
    [string]$OutFile = 'ble-fault-report.md',
    [switch]$PickByRow,
    [string]$RawLog = '',
    [switch]$SelfTest
)

$ErrorActionPreference = 'Stop'
$script:ScenarioTimeoutSec = 75
$script:TagWaitSec = 15
$script:Receiver = 'ble-receive --accept --seconds 6'

# ---------------------------------------------------------------- scenario table

function New-Def {
    param([string]$Name, [string]$Fault, [string]$Gatt, [string]$Kind, [string[]]$Patterns, [int]$Budget, [bool]$Repeat = $false)
    [pscustomobject]@{ Name = $Name; Fault = $Fault; Gatt = $Gatt; Kind = $Kind; Patterns = @($Patterns); Budget = $Budget; Repeat = $Repeat }
}

function Get-AllScenarios {
    @(
        (New-Def 'none-indicate' 'none' 'indicate' 'OK' @() -1),
        (New-Def 'none-notify' 'none' 'notify' 'OK' @() -1),
        (New-Def 'a' 'a' 'indicate' 'FAIL' @('Core(WrongState)') 0),
        (New-Def 'b-indicate' 'b' 'indicate' 'FAIL' @('Core(ConfirmationFailed)') 1),
        (New-Def 'b-notify' 'b' 'notify' 'FAIL' @('Core(ConfirmationFailed)') 1),
        (New-Def 'c' 'c' 'indicate' 'FAIL' @('Timeout', 'Link(Disconnected)') 1),
        (New-Def 'd1-notify' 'd1' 'notify' 'FAIL' @('Core(Framing)') 0),
        (New-Def 'd2' 'd2' 'indicate' 'FAIL' @('Framing') 0),
        (New-Def 'd3' 'd3' 'indicate' 'FAIL' @('Framing') 0),
        (New-Def 'e1' 'e1' 'indicate' 'FAIL' @('Timeout') 0),
        (New-Def 'e2' 'e2' 'indicate' 'OK' @() -1),
        (New-Def 'e3' 'e3' 'indicate' 'FAIL' @('Timeout') 0),
        # f: the handshake already succeeded (code consumed) and only the SECRET fragment is corrupted, so the
        # channel closes with Core(Authentication) but the failure budget is NOT used (expected 0/3; field run
        # 2026-11-06 showed 0/3 and the earlier expectation of 1/3 was wrong).
        (New-Def 'f-indicate' 'f' 'indicate' 'FAIL' @('Core(Authentication)') 0),
        (New-Def 'f-notify' 'f' 'notify' 'FAIL' @('Core(Authentication)') 0),
        (New-Def 'repeat-b3' 'b' 'indicate' 'FAIL' @('Core(ConfirmationFailed)') 3 $true)
    )
}

function Resolve-Scenarios {
    param([string[]]$Requested, [bool]$NoRepeat)
    $all = Get-AllScenarios
    $names = @()
    foreach ($r in @($Requested)) { foreach ($p in ($r -split ',')) { $t = $p.Trim(); if ($t) { $names += $t } } }
    if ($names.Count -eq 0 -or ($names | Where-Object { $_ -ieq 'all' })) { $sel = $all }
    else {
        $sel = @()
        foreach ($n in $names) {
            $m = $all | Where-Object { $_.Name -ieq $n }
            if (-not $m) { throw "Unknown scenario '$n'. Known: $(($all | ForEach-Object { $_.Name }) -join ', ')" }
            $sel += $m
        }
    }
    if ($NoRepeat) { $sel = @($sel | Where-Object { -not $_.Repeat }) }
    , @($sel)
}

# ---------------------------------------------------------------- parsing (pure, testable)

function Get-CodeFromText {
    param([string]$Text)
    $m = [regex]::Match($Text, 'Code \(valid 60 s\): (\d{6})')
    if ($m.Success) { $m.Groups[1].Value } else { $null }
}

function Get-MaskedText {
    param([string]$Text)
    $t = [regex]::Replace($Text, '\b\d{6}\b', '******')
    [regex]::Replace($t, 'KL-[A-Za-z0-9]{4}', 'KL-****')
}

# Prompts are printed without a newline and stdin is not echoed, so the next output follows on the same line:
# nothing here is anchored to a line start.
function Parse-ReceiverOutput {
    param([string]$Text)
    $budgets = @()
    foreach ($m in [regex]::Matches($Text, 'Failure budget used: (\d+)/(\d+)')) { $budgets += [int]$m.Groups[1].Value }
    $attempts = @()
    $i = 0
    foreach ($m in [regex]::Matches($Text, '(Transfer OK|Attempt failed): ([^\r\n]*)')) {
        $isOk = ($m.Groups[1].Value -eq 'Transfer OK')
        $detail = $m.Groups[2].Value.Trim()
        if ($isOk) { $detail = 'Transfer OK' }
        $b = $null
        if ($i -lt $budgets.Count) { $b = $budgets[$i] }
        $attempts += [pscustomobject]@{ Kind = $(if ($isOk) { 'OK' } else { 'FAIL' }); Detail = $detail; Budget = $b }
        $i++
    }
    $others = @()
    foreach ($m in [regex]::Matches($Text, '(Scan failed|Connect failed|No KagiLink senders found|No valid row chosen|Refused|Gate refused|Blocked|Core error|Cancelled|Gate error)[^\r\n]*')) {
        $others += $m.Value.Trim()
    }
    $starts = @()
    foreach ($m in [regex]::Matches($Text, 'Attempt started: ([^\r\n]+)')) { $starts += $m.Groups[1].Value.Trim() }
    $created = $null
    $cm = [regex]::Match($Text, 'Code created: ([^\r\n]+)')
    if ($cm.Success) { $created = $cm.Groups[1].Value.Trim() }
    [pscustomobject]@{
        Attempts       = @($attempts)
        Budgets        = @($budgets)
        Others         = @($others)
        AttemptStarts  = @($starts)
        CodeCreated    = $created
        PickCount      = [regex]::Matches($Text, 'Pick the row whose tag matches your phone').Count
        RetryCount     = [regex]::Matches($Text, 'Retry with the same code\?').Count
        NoLongerActive = [bool]($Text -match 'The code is no longer active')
    }
}

# Row number of the LAST listing of a tag: "[2] KL-AB23  signal: ..."
function Find-TagRow {
    param([string]$Text, [string]$Tag)
    $row = $null
    foreach ($m in [regex]::Matches($Text, '\[(\d+)\] (KL-[A-Za-z0-9]{4})')) {
        if ($m.Groups[2].Value -ceq $Tag) { $row = [int]$m.Groups[1].Value }
    }
    $row
}

function Get-LogcatInfo {
    param([string]$Text)
    $started = @(); $ended = @(); $refused = @()
    foreach ($m in [regex]::Matches($Text, 'TEST started tag=(KL-[A-Za-z0-9]{4})')) { $started += $m.Groups[1].Value }
    foreach ($m in [regex]::Matches($Text, 'TEST ended\b[^\r\n]*?outcome=(\S+)')) { $ended += $m.Groups[1].Value }
    foreach ($m in [regex]::Matches($Text, 'TEST start refused: ([^\r\n]*)')) { $refused += $m.Groups[1].Value.Trim() }
    [pscustomobject]@{ Started = @($started); Ended = @($ended); Refused = @($refused) }
}

function Get-ScenarioVerdict {
    param($Def, $Attempts, [bool]$NoLongerActive)
    $att = @($Attempts)
    $expText = if ($Def.Kind -eq 'OK') { 'Transfer OK' } else { $Def.Patterns -join ' | ' }
    if ($att.Count -eq 0) { return [pscustomobject]@{ Status = 'ERROR'; Actual = '(no result line)'; BudgetActual = '-'; Expected = $expText } }
    $match = {
        param($a)
        if ($Def.Kind -eq 'OK') { return ($a.Kind -eq 'OK') }
        if ($a.Kind -ne 'FAIL') { return $false }
        foreach ($p in $Def.Patterns) { if ($a.Detail.IndexOf($p, [StringComparison]::Ordinal) -ge 0) { return $true } }
        $false
    }
    $actual = ($att | ForEach-Object { $_.Detail }) -join ' ; '
    $bAct = ($att | ForEach-Object { if ($null -eq $_.Budget) { '?' } else { "$($_.Budget)" } }) -join ','
    $ok = $true
    if ($Def.Repeat) {
        if ($att.Count -ne 3) { $ok = $false }
        else {
            for ($k = 0; $k -lt 3; $k++) {
                if (-not (& $match $att[$k])) { $ok = $false }
                if ($att[$k].Budget -ne ($k + 1)) { $ok = $false }
            }
            if (-not $NoLongerActive) { $ok = $false }
        }
        $bAct = "$bAct (inactive: $NoLongerActive)"
        $bExp = '1,2,3 + inactive'
    }
    else {
        if ($att.Count -ne 1) { $ok = $false }
        elseif (-not (& $match $att[0])) { $ok = $false }
        elseif ($Def.Budget -ge 0 -and $att[0].Budget -ne $Def.Budget) { $ok = $false }
        $bExp = if ($Def.Budget -ge 0) { "$($Def.Budget)/3" } else { 'any' }
        $b0 = if ($att[0].Budget -eq $null) { '?' } else { "$($att[0].Budget)" }
        $bAct = "$b0/3"
    }
    [pscustomobject]@{ Status = $(if ($ok) { 'PASS' } else { 'FAIL' }); Actual = $actual; BudgetActual = $bAct; Expected = $expText; BudgetExpected = $bExp }
}

function ConvertTo-ArgString {
    param([string[]]$Items)
    $parts = foreach ($a in $Items) {
        if ($a -eq '') { '""' }
        elseif ($a -notmatch '[\s"]') { $a }
        else { '"' + (($a -replace '(\\*)"', '$1$1\"') -replace '(\\+)$', '$1$1') + '"' }
    }
    $parts -join ' '
}

# ---------------------------------------------------------------- self test

function Invoke-SelfTest {
    $fails = 0
    $check = {
        param($name, $actual, $expected)
        if ("$actual" -ceq "$expected") { Write-Host "  ok   $name" }
        else { Write-Host "  FAIL $name : expected [$expected] got [$actual]"; $script:stFails++ }
    }
    $script:stFails = 0

    $okSample = "Code (valid 60 s): 123456`r`nCode created: 2026-11-06 01:02:03 UTC`r`nEnter it on the phone...`r`n[1] KL-AB23  signal: Strong`r`n[2] KL-CD45  signal: Weak`r`nPick the row whose tag matches your phone: Attempt started: 2026-11-06 01:02:09 UTC`r`nFailure budget used: 0/3 (an attempt abandoned after M3 is included)`r`nTransfer OK: 36 bytes received in 3 fragments (plaintext not shown).`r`n"
    $p = Parse-ReceiverOutput $okSample
    & $check 'ok: code' (Get-CodeFromText $okSample) '123456'
    & $check 'ok: created' $p.CodeCreated '2026-11-06 01:02:03 UTC'
    & $check 'ok: attempts' $p.Attempts.Count 1
    & $check 'ok: kind' $p.Attempts[0].Kind 'OK'
    & $check 'ok: budget' $p.Attempts[0].Budget 0
    & $check 'ok: pick prompts' $p.PickCount 1
    & $check 'ok: attempt start' $p.AttemptStarts[0] '2026-11-06 01:02:09 UTC'
    & $check 'ok: not inactive' $p.NoLongerActive $false
    & $check 'ok: row of KL-CD45' (Find-TagRow $okSample 'KL-CD45') 2

    $failSample = "Code (valid 60 s): 654321`nCode created: 2026-11-06 02:00:00 UTC`n[1] KL-EF67  signal: Strong`nPick the row whose tag matches your phone: Attempt started: 2026-11-06 02:00:08 UTC`nFailure budget used: 1/3 (an attempt abandoned after M3 is included)`nAttempt failed: Core(ConfirmationFailed)`nRetry with the same code? Type y to scan again (anything else ends): "
    $p = Parse-ReceiverOutput $failSample
    & $check 'fail: kind' $p.Attempts[0].Kind 'FAIL'
    & $check 'fail: detail' $p.Attempts[0].Detail 'Core(ConfirmationFailed)'
    & $check 'fail: budget' $p.Attempts[0].Budget 1
    & $check 'fail: retry prompts' $p.RetryCount 1
    $def = (Get-AllScenarios | Where-Object { $_.Name -eq 'b-indicate' })
    & $check 'fail: verdict b-indicate' (Get-ScenarioVerdict $def $p.Attempts $false).Status 'PASS'
    $defA = (Get-AllScenarios | Where-Object { $_.Name -eq 'a' })
    & $check 'fail: verdict a (wrong error/budget)' (Get-ScenarioVerdict $defA $p.Attempts $false).Status 'FAIL'

    $retrySample = $failSample + "[1] KL-GH89  signal: Strong`nPick the row whose tag matches your phone: Attempt started: 2026-11-06 02:00:20 UTC`nFailure budget used: 2/3 (x)`nAttempt failed: Core(ConfirmationFailed)`nRetry with the same code? Type y to scan again (anything else ends): [1] KL-JK23  signal: Strong`nPick the row whose tag matches your phone: Attempt started: 2026-11-06 02:00:32 UTC`nFailure budget used: 3/3 (x)`nAttempt failed: Core(ConfirmationFailed)`nRetry with the same code? Type y to scan again (anything else ends): The code is no longer active (expired, used up or consumed).`n"
    $p = Parse-ReceiverOutput $retrySample
    & $check 'retry: attempts' $p.Attempts.Count 3
    & $check 'retry: budgets' ($p.Budgets -join ',') '1,2,3'
    & $check 'retry: picks' $p.PickCount 3
    & $check 'retry: retry prompts' $p.RetryCount 3
    & $check 'retry: inactive' $p.NoLongerActive $true
    & $check 'retry: row of KL-JK23' (Find-TagRow $retrySample 'KL-JK23') 1
    $defR = (Get-AllScenarios | Where-Object { $_.Name -eq 'repeat-b3' })
    & $check 'retry: verdict repeat-b3' (Get-ScenarioVerdict $defR $p.Attempts $p.NoLongerActive).Status 'PASS'
    & $check 'retry: verdict without inactive' (Get-ScenarioVerdict $defR $p.Attempts $false).Status 'FAIL'

    $inact = "Code (valid 60 s): 111222`nThe code is no longer active (expired, used up or consumed).`n"
    $p = Parse-ReceiverOutput $inact
    & $check 'inactive: flag' $p.NoLongerActive $true
    & $check 'inactive: no attempts' $p.Attempts.Count 0
    & $check 'inactive: verdict ERROR' (Get-ScenarioVerdict $defA $p.Attempts $true).Status 'ERROR'

    $other = Parse-ReceiverOutput "Pick the row whose tag matches your phone: No valid row chosen. Nothing was connected.`n"
    & $check 'other: error line' $other.Others.Count 1
    $none = Parse-ReceiverOutput "Code (valid 60 s): 222333`nNo KagiLink senders found`n"
    & $check 'other: no senders' $none.Others[0] 'No KagiLink senders found'

    $log = "11-06 10:00:00.100 I/KLV1 (1): TEST started tag=KL-AB23 fault=B gatt=indicate`n11-06 10:00:09.000 I/KLV1 (1): TEST ended tag=KL-AB23 outcome=SENDER_FINISH_FAILED`n11-06 10:00:20.000 I/KLV1 (1): TEST started tag=KL-CD45 fault=B"
    $li = Get-LogcatInfo $log
    & $check 'logcat: started' ($li.Started -join ',') 'KL-AB23,KL-CD45'
    & $check 'logcat: ended' ($li.Ended -join ',') 'SENDER_FINISH_FAILED'
    $log2 = "I/KLV1 (1): TEST started tag=KL-AB23 fault=TAMPER_M4 gatt=indicate utc=x`nI/KLV1 (1): TEST ended fault=TAMPER_M4 outcome=failed utc=x`nI/KLV1 (1): TEST start refused: app not in foreground"
    $li2 = Get-LogcatInfo $log2
    & $check 'logcat: new-format ended' ($li2.Ended -join ',') 'failed'
    & $check 'logcat: refused' ($li2.Refused -join ',') 'app not in foreground'
    & $check 'logcat: none refused' $li.Refused.Count 0

    $masked = Get-MaskedText 'Code (valid 60 s): 123456 tag KL-AB23 bytes 36'
    & $check 'mask: code and tag' $masked 'Code (valid 60 s): ****** tag KL-**** bytes 36'

    & $check 'args: plain' (ConvertTo-ArgString @('a', 'b')) 'a b'
    & $check 'args: space' (ConvertTo-ArgString @('a b')) '"a b"'
    & $check 'args: trailing backslash' (ConvertTo-ArgString @('C:\x y\')) '"C:\x y\\"'
    & $check 'args: empty' (ConvertTo-ArgString @('')) '""'

    $all = Resolve-Scenarios @('all') $false
    & $check 'scenarios: all' $all.Count 15
    & $check 'scenarios: skip repeat' (Resolve-Scenarios @('all') $true).Count 14
    & $check 'scenarios: comma list' (Resolve-Scenarios @('a,b-notify') $false).Count 2
    $threw = $false
    try { Resolve-Scenarios @('nope') $false | Out-Null } catch { $threw = $true }
    & $check 'scenarios: unknown throws' $threw $true

    if ($script:stFails -gt 0) { Write-Host "SELF TEST FAILED: $($script:stFails) check(s)"; return 1 }
    Write-Host 'SELF TEST PASSED'
    return 0
}

if ($SelfTest) { exit (Invoke-SelfTest) }

# ---------------------------------------------------------------- process helpers

function Initialize-Pump {
    if ('KlStreamPump' -as [type]) { return }
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.Threading;
public class KlStreamPump {
    private readonly StringBuilder sb = new StringBuilder();
    private readonly object gate = new object();
    private bool done;
    public KlStreamPump(TextReader r) {
        Thread t = new Thread(delegate() {
            char[] buf = new char[256];
            try { int n; while ((n = r.Read(buf, 0, buf.Length)) > 0) { lock (gate) { sb.Append(buf, 0, n); } } }
            catch (Exception) { }
            lock (gate) { done = true; }
        });
        t.IsBackground = true;
        t.Start();
    }
    public string Text { get { lock (gate) { return sb.ToString(); } } }
    public bool IsDone { get { lock (gate) { return done; } } }
}
'@
}

function Invoke-Native {
    param([string]$Exe, [string[]]$ArgList, [int]$TimeoutMs = 20000)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Exe
    $psi.Arguments = ConvertTo-ArgString $ArgList
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $p = [System.Diagnostics.Process]::Start($psi)
    $errTask = $p.StandardError.ReadToEndAsync()
    $outTask = $p.StandardOutput.ReadToEndAsync()
    if (-not $p.WaitForExit($TimeoutMs)) { try { $p.Kill() } catch {}; return [pscustomobject]@{ ExitCode = -1; Out = ''; Err = 'timeout' } }
    $p.WaitForExit()
    [pscustomobject]@{ ExitCode = $p.ExitCode; Out = $outTask.Result; Err = $errTask.Result }
}

function Invoke-AdbCmd {
    param([string[]]$AdbArgs)
    $full = @()
    if ($Serial) { $full += @('-s', $Serial) }
    $full += $AdbArgs
    Invoke-Native -Exe $Adb -ArgList $full
}

function Send-TestStart {
    param([string]$Code, [string]$Fault, [string]$Gatt)
    # All values are validated here; nothing untrusted is interpolated.
    if ($Code -notmatch '^\d{6}$') { throw 'code format' }
    if ($Fault -notmatch '^(none|a|b|c|d1|d2|d3|e1|e2|e3|f)$') { throw 'fault name' }
    if ($Gatt -notmatch '^(indicate|notify)$') { throw 'gatt mode' }
    # The code goes through the adb shell's STDIN only: an argument list would be logged by adbd into the
    # device logcat and be visible in the PC process list.
    $cmd = 'am broadcast -a com.areatm.kagilink.spike.peripheral.TEST_START -n com.areatm.kagilink.spike.peripheral/.TestStartReceiver --es code ' + $Code + ' --es fault ' + $Fault + ' --es gatt ' + $Gatt
    $full = @()
    if ($Serial) { $full += @('-s', $Serial) }
    $full += 'shell'
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Adb
    $psi.Arguments = ConvertTo-ArgString $full
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $p = [System.Diagnostics.Process]::Start($psi)
    $errTask = $p.StandardError.ReadToEndAsync()
    $outTask = $p.StandardOutput.ReadToEndAsync()
    $p.StandardInput.NewLine = "`n"
    $p.StandardInput.Write($cmd + "`n" + 'exit' + "`n")
    $p.StandardInput.Close()
    if (-not $p.WaitForExit(20000)) { try { $p.Kill() } catch {}; return 'adb shell broadcast timed out' }
    $p.WaitForExit()
    $o = $outTask.Result; $e = $errTask.Result
    if ($p.ExitCode -ne 0 -or $o -match 'Error|Exception') { return (Get-MaskedText ('adb broadcast failed: ' + $o + $e)) }
    $null
}

function Start-PhoneApp {
    # The app screen must be open: a background session fails closed.
    $r = Invoke-AdbCmd @('shell', 'am', 'start', '-n', 'com.areatm.kagilink.spike.peripheral/.PeripheralActivity')
    if ($r.ExitCode -ne 0 -or $r.Out -match 'Error') { return ('could not start the phone app: ' + $r.Out + $r.Err) }
    Start-Sleep -Seconds 2
    $null
}

function Get-PhoneLog {
    $r = Invoke-AdbCmd @('logcat', '-d', '-s', 'KLV1:I')
    Get-LogcatInfo $r.Out
}

# ---------------------------------------------------------------- one scenario

function Invoke-Scenario {
    param($Def)
    Initialize-Pump
    $notes = New-Object System.Collections.ArrayList
    $timedOut = $false
    $parsed = $null
    $text = ''
    $phoneEnded = @()

    $null = Invoke-AdbCmd @('logcat', '-c')

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $script:ReceiverPath
    $psi.Arguments = $script:Receiver
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    $outPump = New-Object KlStreamPump($proc.StandardOutput)
    $errPump = New-Object KlStreamPump($proc.StandardError)

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $code = $null
    $codeSent = $false
    $seen = @{}
    $curTag = $null
    $needTag = $false
    $bcastAt = 0.0
    $lastLog = -10.0
    $picks = 0
    $retries = 0
    $abort = $false

    try {
        while (-not $abort) {
            if ($sw.Elapsed.TotalSeconds -gt $script:ScenarioTimeoutSec) { $timedOut = $true; break }
            $text = $outPump.Text
            $parsed = Parse-ReceiverOutput $text

            if (-not $code) {
                $c = Get-CodeFromText $text
                if ($c) {
                    $code = $c
                    $err = Send-TestStart $code $Def.Fault $Def.Gatt
                    if ($err) { [void]$notes.Add($err); $abort = $true; break }
                    $codeSent = $true; $needTag = $true; $bcastAt = $sw.Elapsed.TotalSeconds
                    Write-Host '  Code sent (masked)'
                }
            }

            if ($needTag -and ($sw.Elapsed.TotalSeconds - $lastLog) -ge 1.0) {
                $lastLog = $sw.Elapsed.TotalSeconds
                $info = Get-PhoneLog
                if ($info.Refused.Count -gt 0) { [void]$notes.Add('phone refused the test start: ' + $info.Refused[0]); $abort = $true; break }
                foreach ($t in $info.Started) { if (-not $seen.ContainsKey($t)) { $curTag = $t; $needTag = $false; break } }
                if ($needTag -and ($sw.Elapsed.TotalSeconds - $bcastAt) -gt $script:TagWaitSec) {
                    [void]$notes.Add('phone "TEST started" line not seen within 15 s'); $abort = $true; break
                }
            }

            if ($parsed.PickCount -gt $picks -and $curTag) {
                $answer = $curTag
                if ($PickByRow) {
                    $row = Find-TagRow $text $curTag
                    if ($null -eq $row) { [void]$notes.Add('phone tag not in the receiver list'); $abort = $true; break }
                    $answer = "$row"
                }
                $proc.StandardInput.WriteLine($answer); $proc.StandardInput.Flush()
                $seen[$curTag] = $true; $curTag = $null; $picks++
                Write-Host '  Tag sent to receiver'
            }

            if ($parsed.RetryCount -gt $retries) {
                $retries++
                if ($Def.Repeat) {
                    if ($parsed.Attempts.Count -lt 3) {
                        $err = Send-TestStart $code $Def.Fault $Def.Gatt
                        if ($err) { [void]$notes.Add($err); $abort = $true; break }
                        $needTag = $true; $curTag = $null; $bcastAt = $sw.Elapsed.TotalSeconds; $lastLog = -10.0
                        Write-Host '  Code re-sent (masked), same code'
                    }
                    $proc.StandardInput.WriteLine('y')
                }
                else { $proc.StandardInput.WriteLine('n') }
                $proc.StandardInput.Flush()
            }

            if ($proc.HasExited) {
                Start-Sleep -Milliseconds 400
                $text = $outPump.Text
                $parsed = Parse-ReceiverOutput $text
                break
            }
            Start-Sleep -Milliseconds 200
        }
    }
    catch { [void]$notes.Add((Get-MaskedText "harness error: $($_.Exception.Message)")) }
    finally {
        if (-not $proc.HasExited) { try { $proc.Kill() } catch {} }
    }
    if ($null -eq $parsed) { $parsed = Parse-ReceiverOutput $outPump.Text }
    if ($RawLog) {
        try {
            $raw = [regex]::Replace($outPump.Text, '(Code \(valid 60 s\): )\d{6}', '${1}******')
            Add-Content -LiteralPath $RawLog -Value ("=== " + $Def.Name + "`n" + $raw + "`n") -Encoding UTF8
        } catch { }
    }
    if (-not $code) { [void]$notes.Add('receiver printed no code') }
    $errText = $errPump.Text
    if ($errText.Trim()) { [void]$notes.Add((Get-MaskedText ('receiver stderr: ' + $errText.Trim().Split("`n")[0]))) }

    if ($codeSent) {
        $want = [Math]::Max(1, $parsed.Attempts.Count)
        $end = [System.Diagnostics.Stopwatch]::StartNew()
        while ($true) {
            $info = Get-PhoneLog
            $phoneEnded = @($info.Ended)
            if ($phoneEnded.Count -ge $want -or $end.Elapsed.TotalSeconds -gt 10) { break }
            Start-Sleep -Milliseconds 1000
        }
    }

    $verdict = Get-ScenarioVerdict $Def $parsed.Attempts $parsed.NoLongerActive
    $status = $verdict.Status
    if ($timedOut) { $status = 'TIMEOUT' }
    elseif ($notes.Count -gt 0 -and $parsed.Attempts.Count -eq 0) { $status = 'ERROR' }
    elseif ($parsed.Attempts.Count -eq 0 -and $parsed.Others.Count -gt 0) { $status = 'ERROR'; $verdict.Actual = Get-MaskedText ($parsed.Others -join ' ; ') }
    elseif ($notes.Count -gt 0 -and $status -eq 'PASS') { $status = 'ERROR' }

    $raw = @()
    foreach ($ln in ($text -split "`r?`n")) {
        if ($ln -match 'Code created:|Attempt started:|Failure budget used:|Transfer OK:|Attempt failed:|no longer active|Scan failed|Connect failed|No KagiLink senders|Refused|Gate refused|Blocked') {
            $raw += (Get-MaskedText $ln.Trim())
        }
    }
    [pscustomobject]@{
        Def = $Def; Status = $status; Expected = $verdict.Expected; Actual = $verdict.Actual
        BudgetExpected = $verdict.BudgetExpected; BudgetActual = $verdict.BudgetActual
        Phone = ((@($phoneEnded | ForEach-Object { Get-MaskedText $_ })) -join ',')
        Notes = @($notes); Raw = @($raw)
    }
}

# ---------------------------------------------------------------- main

$selected = Resolve-Scenarios $Scenario ([bool]$SkipRepeat)
if (-not (Test-Path -LiteralPath $ReceiverExe)) { Write-Host "Receiver not found: $ReceiverExe (build it, or use -ReceiverExe)"; exit 2 }
$script:ReceiverPath = (Resolve-Path -LiteralPath $ReceiverExe).Path
try { $state = Invoke-AdbCmd @('get-state') } catch { Write-Host "adb not usable: $($_.Exception.Message)"; exit 2 }
if ($state.ExitCode -ne 0 -or $state.Out.Trim() -ne 'device') { Write-Host 'No authorised adb device (adb get-state did not return "device").'; exit 2 }

$appErr = Start-PhoneApp
if ($appErr) { Write-Host $appErr; exit 2 }
Write-Host "BLE fault harness (test-only, synthetic codes). Scenarios: $(($selected | ForEach-Object { $_.Name }) -join ', ')"
$results = @()
$first = $true
foreach ($def in $selected) {
    if (-not $first -and $WaitBetweenSeconds -gt 0) { Start-Sleep -Seconds $WaitBetweenSeconds }
    $first = $false
    Write-Host "== $($def.Name) (fault=$($def.Fault), gatt=$($def.Gatt))"
    $r = Invoke-Scenario $def
    Write-Host "   -> $($r.Status): $($r.Actual)"
    $results += $r
}

$lines = @()
$lines += '| Scenario | Expected | Actual | Budget actual / expected | Phone outcome | Result |'
$lines += '|---|---|---|---|---|---|'
foreach ($r in $results) {
    $lines += "| $($r.Def.Name) | $($r.Expected) | $($r.Actual) | $($r.BudgetActual) / $($r.BudgetExpected) | $($r.Phone) | $($r.Status) |"
}
Write-Host ''
$lines | ForEach-Object { Write-Host $_ }

$md = @()
$md += '# BLE fault-injection report'
$md += ''
$md += "Generated (UTC): $([DateTime]::UtcNow.ToString('yyyy-MM-dd HH:mm:ss')). Synthetic codes only; codes and tags are masked."
$md += ''
$md += $lines
$md += ''
$md += '## Raw receiver result lines'
foreach ($r in $results) {
    $md += ''
    $md += "### $($r.Def.Name) - $($r.Status)"
    $md += '```'
    foreach ($l in $r.Raw) { $md += $l }
    foreach ($n in $r.Notes) { $md += "note: $n" }
    $md += '```'
}
[System.IO.File]::WriteAllText($OutFile, (($md -join "`r`n") + "`r`n"), (New-Object System.Text.UTF8Encoding($false)))
Write-Host "Report written to $OutFile"

$bad = @($results | Where-Object { $_.Status -ne 'PASS' })
if ($bad.Count -eq 0) { exit 0 } else { exit 1 }
