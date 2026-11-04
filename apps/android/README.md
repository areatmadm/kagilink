# Android sender — planned

Kotlin + BLE Peripheral + mandatory StrongBox Keystore. No native target exists.
Require actual generation, SECURITY_LEVEL_STRONGBOX verification and successful
auth-bound operation. Fail on missing StrongBox, wrong key level or API failure;
never fallback to TEE/software. Test permissions/advertising on devices.
