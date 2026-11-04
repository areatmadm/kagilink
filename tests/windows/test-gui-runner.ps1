# Synthetic subprocess fixtures only: no Bluetooth, UI, secrets or input injection.
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$fixtureRoot = Join-Path $projectRoot 'target\gui-runner-tests'
New-Item -ItemType Directory -Force -Path $fixtureRoot | Out-Null
@'
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <iostream>
#include <string>
#include <string_view>
int wmain(int argc, wchar_t** argv) {
    if (argc != 2) return 3;
    const std::wstring_view argument(argv[1]);
    if (argument != L"--inventory" && argument != L"--ble-test") return 3;
    wchar_t mode[32]{};
    GetEnvironmentVariableW(L"KAGILINK_SYNTHETIC_FIXTURE_MODE", mode, 32);
    if (std::wstring_view(mode) == L"overflow") {
        std::cout << std::string(20000, 'X') << std::flush;
        return 0;
    }
    if (std::wstring_view(mode) == L"sleep") {
        std::cout << "PID=" << GetCurrentProcessId() << '\n' << std::flush;
        Sleep(30000);
        return 0;
    }
    std::cout << (argument == L"--inventory" ? "FIXED_INVENTORY\n" : "FIXED_BLE\n");
    return 0;
}
'@ | Set-Content -LiteralPath (Join-Path $fixtureRoot 'fixture.cpp') -Encoding utf8
@'
#include "diagnostic-runner.h"
#include <iostream>
using diagnostic_gui::Action;
using diagnostic_gui::ProbeRunner;
bool finish(ProbeRunner& runner) {
    const auto deadline = GetTickCount64() + 5000;
    while (runner.Running() && GetTickCount64() < deadline) { runner.Poll(); Sleep(1); }
    return !runner.Running();
}
int main() {
    ProbeRunner runner;
    if (runner.Start(static_cast<Action>(99)) || runner.Error().empty()) return 1;
    std::cout << "PASS: unsupported action rejected\n";
    SetEnvironmentVariableW(L"KAGILINK_SYNTHETIC_FIXTURE_MODE", nullptr);
    if (!runner.Start(Action::Inventory) || !finish(runner) || runner.ExitCode() != 0 ||
        runner.Output() != "FIXED_INVENTORY\r\n") { std::wcerr << runner.Error(); return 2; }
    if (!runner.Start(Action::Ble) || !finish(runner) || runner.ExitCode() != 0 ||
        runner.Output() != "FIXED_BLE\r\n") { std::wcerr << runner.Error(); return 3; }
    std::cout << "PASS: fixed actions and final output capture\n";
    SetEnvironmentVariableW(L"KAGILINK_SYNTHETIC_FIXTURE_MODE", L"overflow");
    if (!runner.Start(Action::Inventory) || !finish(runner) || runner.ExitCode() != ERROR_BUFFER_OVERFLOW ||
        runner.Output().size() > 16384 || runner.Error().empty()) return 4;
    std::cout << "PASS: output cap terminates child\n";
    SetEnvironmentVariableW(L"KAGILINK_SYNTHETIC_FIXTURE_MODE", L"sleep");
    if (!runner.Start(Action::Ble)) return 5;
    const auto deadline = GetTickCount64() + 5000;
    while (runner.Output().find('\n') == std::string::npos && GetTickCount64() < deadline) { runner.Poll(); Sleep(1); }
    if (runner.Output().rfind("PID=", 0) != 0) return 6;
    const auto pid = static_cast<DWORD>(std::stoul(runner.Output().substr(4)));
    HANDLE child = OpenProcess(SYNCHRONIZE, FALSE, pid);
    if (!child) return 7;
    if (runner.Start(Action::Inventory)) { CloseHandle(child); return 8; }
    runner.Cancel();
    const auto stopped = WaitForSingleObject(child, 5000);
    CloseHandle(child);
    if (runner.Running() || !runner.Cancelled() || runner.ExitCode() != ERROR_CANCELLED || stopped != WAIT_OBJECT_0) return 9;
    std::cout << "PASS: cancellation terminates contained child\n";
    SetEnvironmentVariableW(L"KAGILINK_SYNTHETIC_FIXTURE_MODE", nullptr);
    if (!runner.Start(Action::Inventory) || !finish(runner) || runner.ExitCode() != 0 || runner.Cancelled()) return 10;
    std::cout << "PASS: explicit restart resets terminal result\n";
    return 0;
}
'@ | Set-Content -LiteralPath (Join-Path $fixtureRoot 'runner-test.cpp') -Encoding utf8
$vswherePath = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vsPath = & $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsPath) { throw 'MSVC x64 tools required.' }
$devCommand = Join-Path $vsPath 'Common7\Tools\VsDevCmd.bat'
$buildCommand = 'call "{0}" -arch=x64 -host_arch=x64 >nul && cl /nologo /std:c++20 /EHsc /W4 /WX /MT /utf-8 /Fo"{1}\fixture.obj" /Fe"{1}\kagilink-phase0-probe.exe" "{1}\fixture.cpp" && cl /nologo /std:c++20 /EHsc /W4 /WX /MT /utf-8 /I"{2}\apps\windows" /Fo"{1}\runner-test.obj" /Fe"{1}\runner-test.exe" "{1}\runner-test.cpp"' -f $devCommand, $fixtureRoot, $projectRoot
& $env:ComSpec /d /s /c $buildCommand
if ($LASTEXITCODE -ne 0) { throw "Synthetic runner fixture build failed ($LASTEXITCODE)." }
& (Join-Path $fixtureRoot 'runner-test.exe')
if ($LASTEXITCODE -ne 0) { throw "Synthetic runner checks failed ($LASTEXITCODE)." }
