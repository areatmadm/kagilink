// swift-tools-version:6.0
// Swift binding spike over the KagiLink C ABI (D021). Synthetic data only; not a product.
// Run `sh build-xcframework.sh` first to create Binary/KagiLinkC.xcframework.
import PackageDescription

let package = Package(
    name: "KagiLinkCore",
    platforms: [.macOS(.v14), .iOS(.v18)],
    products: [.library(name: "KagiLinkCore", targets: ["KagiLinkCore"])],
    targets: [
        .binaryTarget(name: "KagiLinkC", path: "Binary/KagiLinkC.xcframework"),
        .target(name: "KagiLinkCore", dependencies: ["KagiLinkC"]),
        .testTarget(name: "KagiLinkCoreTests", dependencies: ["KagiLinkCore", "KagiLinkC"]),
    ],
    swiftLanguageModes: [.v5]
)
