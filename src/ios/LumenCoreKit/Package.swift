// swift-tools-version:5.9
// The Rust core (src/app) as a Swift package: the static library in an xcframework plus the
// uniffi-generated Swift. Both are build products of scripts/build-core.sh.
import PackageDescription

let package = Package(
    name: "LumenCoreKit",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "LumenCore", targets: ["LumenCore"])
    ],
    targets: [
        .binaryTarget(name: "LumenFFI", path: "Artifacts/LumenFFI.xcframework"),
        .target(name: "LumenCore", dependencies: ["LumenFFI"], path: "Sources/LumenCore"),
    ]
)
