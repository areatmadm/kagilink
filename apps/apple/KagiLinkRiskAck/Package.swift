// swift-tools-version:6.0
// Risk acknowledgement screen and decision logic (D023, docs/RISK_ACK_SCREEN_SPEC.md).
// Pure Swift/SwiftUI with no Rust or UIKit dependency, so the iOS/iPadOS and macOS sender apps share it.
// Synthetic data only; the notice text is a draft pending legal review.
import PackageDescription

let package = Package(
    name: "KagiLinkRiskAck",
    platforms: [.macOS(.v14), .iOS(.v18)],
    products: [.library(name: "KagiLinkRiskAck", targets: ["KagiLinkRiskAck"])],
    targets: [
        .target(name: "KagiLinkRiskAck"),
        .testTarget(name: "KagiLinkRiskAckTests", dependencies: ["KagiLinkRiskAck"]),
    ],
    swiftLanguageModes: [.v5]
)
