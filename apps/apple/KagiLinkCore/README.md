# KagiLinkCore — Swift binding spike (D021)

Swift Package that calls the Rust C ABI (`core/kagilink-ffi/include/kagilink.h`) and ports the
scenarios of `tests/c/consumer.c`. Synthetic data only; not a product, no UI, no BLE, no Keychain
or Secure Enclave. CPace lives in the shared Rust core (no Swift port).

```sh
sh apps/apple/KagiLinkCore/build-xcframework.sh   # needs rustup 1.92.0 + the 3 Apple targets
cd apps/apple/KagiLinkCore && swift test           # macOS
```

* `Sources/KagiLinkCore/Native.swift` is the only file that handles raw pointers; secrets live in
  `SecretBuffer`, which wipes itself with `kl_secure_wipe` on release.
* Failure counting follows D026 (only attempts that sent M3 spend the three-failure budget).
* `Binary/` (the XCFramework) is a build output and is git-ignored.
* `swift test` cannot run on a device without a host app; device runs used a scratch app shell
  signed with the AreaTM development certificate (see the private session record).
