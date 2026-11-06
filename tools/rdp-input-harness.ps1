<#
.SYNOPSIS
  TEST-ONLY harness for the Phase 5 RDP credential-prompt input study (docs/PHASE5_RDP_INPUT_FEASIBILITY.md, section 3.2).
  Not part of the product receiver, not shipped, never used with real credentials.

.DESCRIPTION
  For each run: starts mstsc.exe against the isolated test VM (192.168.77.0/24 only), waits up to 15 s for the credential
  prompt (a visible dialog of that mstsc process that holds a password edit control, found by window enumeration plus UI
  Automation property checks, never by reading any text), requires that dialog to be the foreground window with the password
  edit focused, then sends the test string with SendInput followed by Enter, and closes mstsc again.

  Run types:
    Positive         the owner-supplied string (Read-Host -AsSecureString, once, never echoed, never stored, never logged).
    NegativeControl  a random wrong string the harness generates itself (letters only); it is never printed or stored.

  Modes:
    Unicode   KEYEVENTF_UNICODE events (layout independent; may be dropped by the prompt - that is something to measure).
    ScanCode  scan-code events built with VkKeyScanEx/MapVirtualKeyEx for the keyboard layout of the target window
              (layout dependent; refuses characters that need Ctrl/Alt/AltGr and refuses when Caps Lock is on).

  SendInput's return value is recorded as "input attempted" only. This harness cannot see whether anything reached the remote
  logon; the only evidence is the server-side Security log (tools/rdp-test-vm-evidence.ps1 -Report).

  Safety rules enforced here: no elevation (exits if elevated), no clipboard, no SetForegroundWindow/AttachThreadInput focus
  tricks, no UIPI/UAC/secure-desktop access, no command-line or process-list exposure of the string, target must be an
  address in 192.168.77.0/24 that also exists on a local interface (host-only network), refuses to start when another mstsc.exe
  is already running, aborts the whole batch when a foreground window other than the prompt/mstsc (or the console it was
  started from, before the prompt appears) is seen.

  Memory handling is best effort on a managed runtime: the string lives in a SecureString/BSTR and an unmanaged buffer that
  are zeroed after use; short-lived managed copies of single key events cannot be excluded. No claim of dump immunity.

.PARAMETER Target
  IPv4 address in 192.168.77.0/24 (default 192.168.77.2). Host names are refused.
.PARAMETER Runs
  Number of runs (default 5).
.PARAMETER Mode
  Unicode (default) or ScanCode. Run the two modes as separate series.
.PARAMETER NegativeEvery
  Every Nth run is a NegativeControl (default 5; 0 = none).
.PARAMETER NegativeOnly
  All runs are NegativeControl; no string is requested from the owner.
.PARAMETER PauseSeconds
  Wait between runs (default 40, must be >= 35 so server-side events of consecutive runs do not overlap the 30 s window).
.PARAMETER ReportPath
  CSV report file (default: .\rdp-input-harness-report-<UTC>.csv in the current directory). Must not exist. Contains no
  characters of any string. Do not commit it (ignored by .gitignore).
#>
[CmdletBinding()]
param(
    [string]$Target = '192.168.77.2',
    [ValidateRange(1, 500)][int]$Runs = 5,
    [ValidateSet('Unicode', 'ScanCode')][string]$Mode = 'Unicode',
    [ValidateRange(0, 500)][int]$NegativeEvery = 5,
    [switch]$NegativeOnly,
    [ValidateRange(35, 600)][int]$PauseSeconds = 40,
    [ValidateRange(8, 30)][int]$PromptTimeoutSeconds = 15,
    [ValidateRange(3, 30)][int]$PostSendSeconds = 8,
    [string]$ReportPath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

if ($PromptTimeoutSeconds -lt 8) { throw 'prompt timeout too small' }
if ($env:OS -ne 'Windows_NT') { throw 'Windows only.' }

# --- not elevated -------------------------------------------------------------------------------------------------
$id = [Security.Principal.WindowsIdentity]::GetCurrent()
if ((New-Object Security.Principal.WindowsPrincipal($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Refusing to run elevated (UIPI/elevation is out of scope). Start a normal, non-administrator PowerShell.'
}

# --- target validation: IPv4 literal inside 192.168.77.0/24 only ---------------------------------------------------
$ip = $null
if (-not [System.Net.IPAddress]::TryParse($Target, [ref]$ip) -or $ip.AddressFamily -ne [System.Net.Sockets.AddressFamily]::InterNetwork -or $ip.ToString() -ne $Target) {
    throw 'Target must be a canonical IPv4 literal (no host names, no port).'
}
$tb = $ip.GetAddressBytes()
if ($tb[0] -ne 192 -or $tb[1] -ne 168 -or $tb[2] -ne 77 -or $tb[3] -lt 1 -or $tb[3] -gt 254) {
    throw 'Target refused: only addresses inside 192.168.77.0/24 are allowed.'
}
$onLocalNet = $false
foreach ($ni in [System.Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces()) {
    foreach ($ua in $ni.GetIPProperties().UnicastAddresses) {
        $b = $ua.Address.GetAddressBytes()
        if ($ua.Address.AddressFamily -eq [System.Net.Sockets.AddressFamily]::InterNetwork -and $b[0] -eq 192 -and $b[1] -eq 168 -and $b[2] -eq 77) { $onLocalNet = $true }
    }
}
if (-not $onLocalNet) { throw 'Refused: no local interface is in 192.168.77.0/24 (host-only test network not present).' }

if ($NegativeOnly) { $NegativeEvery = 1 }

# --- report path ---------------------------------------------------------------------------------------------------
if ([string]::IsNullOrEmpty($ReportPath)) {
    $ReportPath = Join-Path (Get-Location).Path ('rdp-input-harness-report-' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmss') + 'Z.csv')
}
if ([IO.Path]::GetExtension($ReportPath) -ne '.csv') { throw 'ReportPath must end in .csv' }
if (Test-Path -LiteralPath $ReportPath) { throw 'ReportPath already exists; refusing to overwrite.' }
$reportDir = Split-Path -Parent ([IO.Path]::GetFullPath($ReportPath))
if (-not (Test-Path -LiteralPath $reportDir)) { throw 'Report directory does not exist.' }

# --- no leftover mstsc (never touch unrelated processes) -----------------------------------------------------------
if (@(Get-Process -Name mstsc -ErrorAction SilentlyContinue).Count -gt 0) {
    throw 'An mstsc.exe is already running. Close it yourself first; this harness never stops processes it did not start.'
}

# --- native helpers ------------------------------------------------------------------------------------------------
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace KlHarness
{
    public class WinInfo
    {
        public IntPtr Hwnd;
        public uint Pid;
        public string Class;
        public bool Visible;
    }

    public static class Native
    {
        [StructLayout(LayoutKind.Sequential)]
        struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
        [StructLayout(LayoutKind.Sequential)]
        struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
        [StructLayout(LayoutKind.Explicit)]
        struct INPUTUNION { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }
        [StructLayout(LayoutKind.Sequential)]
        struct INPUT { public uint type; public INPUTUNION u; }

        delegate bool EnumProc(IntPtr h, IntPtr l);

        [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
        [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
        [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassNameW(IntPtr h, StringBuilder sb, int max);
        [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
        [DllImport("user32.dll")] static extern uint SendInput(uint n, IntPtr inputs, int cbSize);
        [DllImport("user32.dll")] static extern IntPtr GetKeyboardLayout(uint thread);
        [DllImport("user32.dll")] static extern short VkKeyScanExW(char ch, IntPtr hkl);
        [DllImport("user32.dll")] static extern uint MapVirtualKeyExW(uint code, uint mapType, IntPtr hkl);
        [DllImport("user32.dll")] static extern short GetAsyncKeyState(int vk);
        [DllImport("user32.dll")] static extern short GetKeyState(int vk);
        [DllImport("kernel32.dll", EntryPoint = "RtlZeroMemory")] static extern void RtlZeroMemory(IntPtr p, UIntPtr n);

        public static IntPtr Foreground() { return GetForegroundWindow(); }

        public static uint PidOf(IntPtr h)
        {
            uint pid;
            if (h == IntPtr.Zero) return 0;
            GetWindowThreadProcessId(h, out pid);
            return pid;
        }

        public static string ClassOf(IntPtr h)
        {
            StringBuilder sb = new StringBuilder(256);
            GetClassNameW(h, sb, 256);
            return sb.ToString();
        }

        public static List<WinInfo> TopWindows()
        {
            List<WinInfo> list = new List<WinInfo>();
            EnumWindows(delegate(IntPtr h, IntPtr l)
            {
                WinInfo w = new WinInfo();
                w.Hwnd = h;
                w.Pid = PidOf(h);
                w.Class = ClassOf(h);
                w.Visible = IsWindowVisible(h);
                list.Add(w);
                return true;
            }, IntPtr.Zero);
            return list;
        }

        public static void PostClose(IntPtr h) { PostMessageW(h, 0x0010, IntPtr.Zero, IntPtr.Zero); }

        public static IntPtr LayoutOf(IntPtr h)
        {
            uint pid;
            uint t = GetWindowThreadProcessId(h, out pid);
            return GetKeyboardLayout(t);
        }

        // true when Shift, Ctrl, Alt or a Windows key is physically down
        public static bool ModifierHeld()
        {
            int[] keys = new int[] { 0x10, 0x11, 0x12, 0x5B, 0x5C };
            for (int i = 0; i < keys.Length; i++) { if ((GetAsyncKeyState(keys[i]) & 0x8000) != 0) return true; }
            return false;
        }

        public static bool CapsLockOn() { return (GetKeyState(0x14) & 1) != 0; }

        public static void Zero(IntPtr p, int bytes) { RtlZeroMemory(p, (UIntPtr)(uint)bytes); }

        class Ev { public ushort Vk; public ushort Scan; public uint Flags; }

        static void Add(List<Ev> l, ushort vk, ushort scan, uint flags)
        {
            Ev e = new Ev(); e.Vk = vk; e.Scan = scan; e.Flags = flags; l.Add(e);
        }

        // text: pointer to UTF-16 chars (BSTR or HGlobal), len chars. mode 0 = Unicode, 1 = ScanCode.
        // result[0] = events sent, result[1] = events expected, result[2] = status
        // status: 0 ok, 1 unmappable character (nothing sent), 2 SendInput returned fewer events than expected
        public static int[] Send(IntPtr text, int len, int mode, IntPtr hkl)
        {
            List<Ev> evs = new List<Ev>();
            const uint UNI = 0x4, UP = 0x2, SC = 0x8;
            int status = 0;
            if (mode == 0)
            {
                for (int i = 0; i < len; i++)
                {
                    ushort c = (ushort)Marshal.ReadInt16(text, i * 2);
                    Add(evs, 0, c, UNI);
                    Add(evs, 0, c, UNI | UP);
                }
            }
            else
            {
                ushort shiftScan = (ushort)MapVirtualKeyExW(0x10, 0, hkl);
                for (int i = 0; i < len && status == 0; i++)
                {
                    char ch = (char)Marshal.ReadInt16(text, i * 2);
                    short r = VkKeyScanExW(ch, hkl);
                    if (r == -1) { status = 1; break; }
                    int vk = r & 0xFF;
                    int st = (r >> 8) & 0xFF;
                    if ((st & ~1) != 0) { status = 1; break; }
                    ushort sc = (ushort)MapVirtualKeyExW((uint)vk, 0, hkl);
                    if (sc == 0) { status = 1; break; }
                    if (st == 1) Add(evs, 0, shiftScan, SC);
                    Add(evs, 0, sc, SC);
                    Add(evs, 0, sc, SC | UP);
                    if (st == 1) Add(evs, 0, shiftScan, SC | UP);
                }
            }
            if (status != 0)
            {
                evs.Clear();
                return new int[] { 0, 0, status };
            }
            ushort enterScan = (ushort)MapVirtualKeyExW(0x0D, 0, hkl);
            if (mode == 0)
            {
                Add(evs, 0x0D, enterScan, 0);
                Add(evs, 0x0D, enterScan, UP);
            }
            else
            {
                Add(evs, 0, enterScan, SC);
                Add(evs, 0, enterScan, SC | UP);
            }

            int size = Marshal.SizeOf(typeof(INPUT));
            int n = evs.Count;
            IntPtr buf = Marshal.AllocHGlobal(size * n);
            uint sent = 0;
            try
            {
                RtlZeroMemory(buf, (UIntPtr)(uint)(size * n));
                for (int i = 0; i < n; i++)
                {
                    INPUT inp = new INPUT();
                    inp.type = 1;
                    inp.u.ki.wVk = evs[i].Vk;
                    inp.u.ki.wScan = evs[i].Scan;
                    inp.u.ki.dwFlags = evs[i].Flags;
                    Marshal.StructureToPtr(inp, new IntPtr(buf.ToInt64() + (long)i * size), false);
                }
                sent = SendInput((uint)n, buf, size);
            }
            finally
            {
                RtlZeroMemory(buf, (UIntPtr)(uint)(size * n));
                Marshal.FreeHGlobal(buf);
                for (int i = 0; i < evs.Count; i++) { evs[i].Vk = 0; evs[i].Scan = 0; evs[i].Flags = 0; }
                evs.Clear();
            }
            return new int[] { (int)sent, n, (sent == (uint)n) ? 0 : 2 };
        }
    }
}
'@

# --- helpers -------------------------------------------------------------------------------------------------------
function Get-MstscWindows([int]$ProcId) {
    @([KlHarness.Native]::TopWindows() | Where-Object { $_.Pid -eq $ProcId -and $_.Visible })
}

# A visible dialog of the mstsc process that contains a password edit control. Property checks only, no text is read.
function Find-PasswordPrompt([int]$ProcId) {
    foreach ($w in (Get-MstscWindows $ProcId)) {
        if ($w.Class -ne '#32770') { continue }
        try {
            $root = [System.Windows.Automation.AutomationElement]::FromHandle($w.Hwnd)
            $cond = New-Object System.Windows.Automation.AndCondition(
                (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)),
                (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::IsPasswordProperty, $true)))
            $pw = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
            if ($null -ne $pw) { return [pscustomobject]@{ Hwnd = $w.Hwnd; Password = $pw } }
        } catch { }
    }
    return $null
}

function Test-PasswordFocused($Prompt, [int]$ProcId) {
    try {
        $f = [System.Windows.Automation.AutomationElement]::FocusedElement
        if ($null -eq $f) { return $false }
        if ($f.Current.ProcessId -ne $ProcId) { return $false }
        return ($f.GetRuntimeId() -join ',') -eq ($Prompt.Password.GetRuntimeId() -join ',')
    } catch { return $false }
}

function Close-Mstsc($Proc) {
    if ($null -eq $Proc) { return 'none' }
    try {
        if ($Proc.HasExited) { return 'exited' }
        foreach ($w in (Get-MstscWindows $Proc.Id)) { [KlHarness.Native]::PostClose($w.Hwnd) }
        if ($Proc.WaitForExit(8000)) { return 'closed' }
        # the "disconnect this session?" confirmation: invoke its default (OK, control id 1) button via UI Automation
        foreach ($w in (Get-MstscWindows $Proc.Id)) {
            if ($w.Class -ne '#32770') { continue }
            try {
                $root = [System.Windows.Automation.AutomationElement]::FromHandle($w.Hwnd)
                $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty, '1')
                $btn = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
                if ($null -ne $btn) { $btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
            } catch { }
        }
        if ($Proc.WaitForExit(8000)) { return 'closed-after-confirm' }
        $Proc.Kill()    # only the mstsc.exe this harness started
        $Proc.WaitForExit(5000) | Out-Null
        return 'forced-kill-of-own-child'
    } catch { return 'close-error' }
}

function New-NegativeBuffer([int]$Len) {
    $bytes = New-Object byte[] $Len
    $rng = New-Object System.Security.Cryptography.RNGCryptoServiceProvider
    $rng.GetBytes($bytes)
    $alphabet = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ'
    $ptr = [Runtime.InteropServices.Marshal]::AllocHGlobal(2 * $Len)
    for ($i = 0; $i -lt $Len; $i++) {
        [Runtime.InteropServices.Marshal]::WriteInt16($ptr, 2 * $i, [int][char]$alphabet[$bytes[$i] % $alphabet.Length])
        $bytes[$i] = 0
    }
    $rng.Dispose()
    return $ptr
}

# --- operator input and confirmation -------------------------------------------------------------------------------
Write-Host 'KagiLink RDP input harness (TEST ONLY). Target: ' $Target ' Mode: ' $Mode ' Runs: ' $Runs
Write-Host ('Host UTC now: ' + [DateTime]::UtcNow.ToString('o') + '  (compare with the VM clock before the study)')
Write-Host 'Do not touch the keyboard or mouse after starting. Report file: ' $ReportPath

$secure = $null
if (-not $NegativeOnly) {
    $secure = Read-Host -AsSecureString 'Enter the Positive test string for the THROWAWAY test account (not echoed)'
    if ($secure.Length -lt 1) { throw 'Empty string refused.' }
}
$go = Read-Host 'Type START to begin (then hands off keyboard and mouse)'
if ($go -ne 'START') { if ($null -ne $secure) { $secure.Dispose() }; throw 'Not started.' }
Start-Sleep -Seconds 3

$startFg = [KlHarness.Native]::Foreground()
$results = New-Object System.Collections.Generic.List[object]
$aborted = $null
$proc = $null

try {
    for ($run = 1; $run -le $Runs; $run++) {
        $type = 'Positive'
        if ($NegativeEvery -gt 0 -and ($run % $NegativeEvery) -eq 0) { $type = 'NegativeControl' }
        $row = [ordered]@{ Run = $run; Type = $type; Mode = $Mode; UtcTime = ''; EventsSent = 0; EventsExpected = 0; WindowFound = 'no'; Outcome = '' }
        $sendDone = $false

        $proc = Start-Process -FilePath (Join-Path $env:SystemRoot 'System32\mstsc.exe') -ArgumentList ('/v:' + $Target) -PassThru
        $deadline = [DateTime]::UtcNow.AddSeconds($PromptTimeoutSeconds)
        $prompt = $null
        $reason = 'FAIL:NoPrompt'
        while ([DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $fg = [KlHarness.Native]::Foreground()
            if ($fg -ne [IntPtr]::Zero -and $fg -ne $startFg -and [KlHarness.Native]::PidOf($fg) -ne $proc.Id) {
                throw 'ABORT:ForegroundChanged before prompt (a window other than the console or mstsc became foreground)'
            }
            if ($proc.HasExited) { $reason = 'FAIL:MstscExited'; break }
            $prompt = Find-PasswordPrompt $proc.Id
            if ($null -eq $prompt) { continue }
            $row.WindowFound = 'yes'
            if ($fg -ne $prompt.Hwnd) { $reason = 'FAIL:PromptNotForeground'; $prompt = $null; continue }
            if (-not (Test-PasswordFocused $prompt $proc.Id)) { $reason = 'FAIL:PasswordEditNotFocused'; $prompt = $null; continue }
            break
        }

        if ($null -ne $prompt) {
            if ([KlHarness.Native]::ModifierHeld()) { $reason = 'FAIL:ModifierHeld' }
            elseif ($Mode -eq 'ScanCode' -and [KlHarness.Native]::CapsLockOn()) { $reason = 'FAIL:CapsLockOn' }
            elseif ([KlHarness.Native]::Foreground() -ne $prompt.Hwnd) { $reason = 'FAIL:PromptNotForeground' }
            else {
                $hkl = [KlHarness.Native]::LayoutOf($prompt.Hwnd)
                $modeNum = 0
                if ($Mode -eq 'ScanCode') { $modeNum = 1 }
                $bstr = [IntPtr]::Zero
                $neg = [IntPtr]::Zero
                $len = 0
                try {
                    if ($type -eq 'Positive') {
                        $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
                        $len = $secure.Length
                        $row.UtcTime = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ss.fffZ')
                        $res = [KlHarness.Native]::Send($bstr, $len, $modeNum, $hkl)
                    } else {
                        $len = 12
                        $neg = New-NegativeBuffer $len
                        $row.UtcTime = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ss.fffZ')
                        $res = [KlHarness.Native]::Send($neg, $len, $modeNum, $hkl)
                    }
                } finally {
                    if ($bstr -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
                    if ($neg -ne [IntPtr]::Zero) { [KlHarness.Native]::Zero($neg, 2 * $len); [Runtime.InteropServices.Marshal]::FreeHGlobal($neg) }
                }
                $row.EventsSent = $res[0]
                $row.EventsExpected = $res[1]
                if ($res[2] -eq 1) { $reason = 'FAIL:UnmappableCharacterNothingSent' }
                elseif ($res[2] -eq 2) { $reason = 'FAIL:PartialSend'; $sendDone = $true }
                else { $reason = 'InputAttempted'; $sendDone = $true }
            }
        }

        if ($sendDone) {
            # watch focus after the input: only mstsc windows (or momentarily none) may be foreground
            $end = [DateTime]::UtcNow.AddSeconds($PostSendSeconds)
            while ([DateTime]::UtcNow -lt $end) {
                Start-Sleep -Milliseconds 250
                $fg = [KlHarness.Native]::Foreground()
                if ($fg -ne [IntPtr]::Zero -and [KlHarness.Native]::PidOf($fg) -ne $proc.Id) {
                    $row.Outcome = $reason
                    $results.Add([pscustomobject]$row)
                    throw 'ABORT:ForegroundChanged after input (a window other than mstsc became foreground)'
                }
            }
        }

        $closeState = Close-Mstsc $proc
        $proc = $null
        $row.Outcome = $reason + ' [mstsc ' + $closeState + ']'
        $results.Add([pscustomobject]$row)
        Write-Host ('run ' + $run + '/' + $Runs + ' ' + $type + ' -> ' + $row.Outcome)

        if ($run -lt $Runs) { Start-Sleep -Seconds $PauseSeconds }
    }
} catch {
    $aborted = $_.Exception.Message
} finally {
    if ($null -ne $proc) { Close-Mstsc $proc | Out-Null }
    if ($null -ne $secure) { $secure.Dispose() }
}

# --- report --------------------------------------------------------------------------------------------------------
$table = $results | Select-Object Run, Type, Mode, UtcTime, @{n = 'EventsSentOfExpected'; e = { [string]$_.EventsSent + '/' + [string]$_.EventsExpected } }, WindowFound, Outcome
$table | Format-Table -AutoSize | Out-String | Write-Host
$results | Export-Csv -LiteralPath $ReportPath -NoTypeInformation -Encoding UTF8
Write-Host ('Report written (no string characters inside): ' + $ReportPath)
Write-Host 'InputAttempted means SendInput accepted the events. It does NOT mean delivery or logon. Compare with the VM Security log:'
Write-Host '  tools/rdp-test-vm-evidence.ps1 -Report ... -RunsFile <this report>'
if ($null -ne $aborted) {
    Write-Host ('BATCH ABORTED: ' + $aborted)
    exit 2
}
