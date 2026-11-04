$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$outputPath = Join-Path $projectRoot 'target\phase0'
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null
$vswherePath = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswherePath)) { throw 'Visual Studio C++ tools required.' }
$vsPath = & $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsPath) { throw 'MSVC x64 C++ tools not found.' }
$devCommand = Join-Path $vsPath 'Common7\Tools\VsDevCmd.bat'
# One shell end-to-end; only compilation, no deletion/move or arbitrary payload.
$buildCommand = 'call "{0}" -arch=x64 -host_arch=x64 >nul && rc /nologo /fo "{1}\probe.res" "{2}\probe.rc" && cl /nologo /std:c++20 /EHsc /W4 /WX /MT /utf-8 /DUNICODE /D_UNICODE /Fo"{1}\probe.obj" /Fe"{1}\kagilink-phase0-probe.exe" "{2}\probe.cpp" "{1}\probe.res" /link windowsapp.lib user32.lib advapi32.lib ncrypt.lib /MANIFEST:EMBED /MANIFESTINPUT:"{2}\probe.manifest"' -f $devCommand, $outputPath, $PSScriptRoot
& $env:ComSpec /d /s /c $buildCommand
if ($LASTEXITCODE -ne 0) { throw "Windows probe build failed ($LASTEXITCODE)." }
Write-Output "Built $outputPath\kagilink-phase0-probe.exe"
