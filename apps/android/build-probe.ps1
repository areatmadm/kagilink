[CmdletBinding()]
param([string]$AndroidSdk = $env:ANDROID_HOME, [string]$GradleExecutable)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
if (-not $AndroidSdk) { $AndroidSdk = $env:ANDROID_SDK_ROOT }
if (-not $AndroidSdk -or -not (Test-Path -LiteralPath (Join-Path $AndroidSdk 'platforms\android-37.2\android.jar'))) {
    throw 'Android SDK platform 37.2 required. Configure ANDROID_HOME or -AndroidSdk; no SDK/license setup is performed.'
}
if (-not (Test-Path -LiteralPath (Join-Path $AndroidSdk 'build-tools\37.0.0'))) { throw 'Android SDK build tools 37.0.0 are required.' }
if (-not $GradleExecutable) { throw 'Pass -GradleExecutable pointing to a verified Gradle 9.8.2 installation.' }
if (-not (Test-Path -LiteralPath $GradleExecutable -PathType Leaf)) { throw 'Gradle executable not found.' }
if (-not $env:JAVA_HOME -or -not (Test-Path -LiteralPath (Join-Path $env:JAVA_HOME 'bin\java.exe'))) {
    $javaExecutable = (Get-Command java -ErrorAction Stop).Source
    $javaRoot = Split-Path -Parent (Split-Path -Parent $javaExecutable)
    if (-not (Test-Path -LiteralPath (Join-Path $javaRoot 'bin\java.exe'))) { throw 'Configure JAVA_HOME to a JDK root, not its bin directory.' }
    $env:JAVA_HOME = $javaRoot # Process-local only; never changes system/user settings.
}
$env:ANDROID_HOME = [IO.Path]::GetFullPath($AndroidSdk)
$env:GRADLE_USER_HOME = Join-Path $projectRoot 'target\gradle-home'
# Gradle/SDK caches stay outside source tracking. Signing is disabled for both variants.
$versionText = (& $GradleExecutable --version) -join "`n"
if ($LASTEXITCODE -ne 0 -or $versionText -notmatch '(?m)^Gradle 9\.8\.2\s*$') { throw 'Verified Gradle 9.8.2 is required.' }
& $GradleExecutable -p $PSScriptRoot --no-daemon --console=plain :app:assembleDebug :app:lintDebug
if ($LASTEXITCODE -ne 0) { throw "Android probe build/lint failed ($LASTEXITCODE)." }
