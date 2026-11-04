# KagiLink Android sender — Phase 0 probe source

Kotlin/Android native research target: `app`, provisional application ID
`in.minori.key.phase0`. Android 14/API 34+; compile SDK 37.2 and target API 37.
The UI advertises a separate
diagnostic service for at most 60 seconds and answers one fixed public GATT request.
Nearby Devices permission, radio state, explicit start/stop and foreground cleanup
are handled. There is no password field, vault, network permission, PAKE or production
transfer. Android SDK/build/device tests have NOT RUN on the Windows-only host.

Build and test instructions: [native BLE probes](../../docs/PHASE0_BLE_PROBE.md).
Pinned research tools: AGP 9.5.0 with built-in Kotlin, Gradle 9.8.2, SDK build tools
37.0.0 and JDK 17+. The stable `platforms;android-37.2` revision 2 was verified from
[Google's SDK repository](https://dl.google.com/android/repository/repository2-3.xml)
on 2026-11-04. The minor SDK is explicit, so builds do not select whichever 37.x
platform happens to be installed. No signing, SDK installation/license acceptance or device install
is automated. Production package/signing identity remains unallocated.

Liquid Glass-style UI uses native Android drawables: a luminous backdrop, layered
frosted panels, reflected rims and ripple controls. It has no Apple/private API or
additional UI SDK. Opaque-enough surfaces preserve text contrast; sp text, scroll,
system-bar/cutout insets, heading semantics, status announcements and 56dp minimum
actions support accessibility. No decorative motion is used. Actual device,
TalkBack, large-font and screenshot/layout QA are still NOT RUN.

SDK-independent Gradle configuration/dependency resolution passed using the exact
tool pins and generated both lockfiles. This does not compile Kotlin app sources,
run Android lint, build an APK or validate BLE. On Windows the helper is:

```powershell
.\apps\android\build-probe.ps1 -AndroidSdk C:\Android\Sdk -GradleExecutable C:\Tools\gradle-9.8.2\bin\gradle.bat
```

On Mac use a verified Gradle 9.8.2 installation and existing SDK 37.2/build tools
37.0.0, then run `sh apps/android/build-probe.sh /path/to/gradle-9.8.2/bin/gradle`
from the project root. Normal builds consume
locks; lock updates must be explicitly reviewed. Both debug and release signing
are disabled, producing unsigned research APKs only.

StrongBox is mandatory for the future product. This public diagnostic reports only
the feature flag, explicitly unverified. It creates no key and enables no sensitive
transfer on any hardware. Actual generation, STRONGBOX level, auth-bound use and
negative devices still gate production; no TEE/software fallback is allowed.
