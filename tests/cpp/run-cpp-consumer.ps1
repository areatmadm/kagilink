# Builds the FFI static library, compiles tests/cpp/consumer.cpp (MSVC, strict warnings) and runs it.
# Synthetic data only. Exit code 0 only if the build and every check passed.
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
Push-Location $root
try {
    cargo build -p areatm-kagilink-ffi --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = & $vswhere -latest -property installationPath
    if (-not $vs) { throw 'Visual Studio not found' }
    $vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'
    $outDir = Join-Path $root 'target\cpp-consumer'
    New-Item -ItemType Directory -Force $outDir | Out-Null
    $exe = Join-Path $outDir 'consumer.exe'
    $libs = 'kernel32.lib ntdll.lib userenv.lib ws2_32.lib dbghelp.lib bcrypt.lib advapi32.lib'
    $cmd = "`"$vcvars`" >nul 2>&1 && cl /nologo /W4 /WX /std:c++20 /EHsc /Icore\kagilink-ffi\include tests\cpp\consumer.cpp target\release\areatm_kagilink_ffi.lib $libs /Fe:`"$exe`" /Fo:`"$outDir\consumer.obj`" && `"$exe`""
    cmd /c $cmd
    if ($LASTEXITCODE -ne 0) { throw "C++ consumer failed (exit $LASTEXITCODE)" }
} finally {
    Pop-Location
}
