// swift-tools-version:5.7
import PackageDescription

let package = Package(
    name: "StateSet",
    platforms: [
        .iOS(.v13),
        .macOS(.v10_15),
        .tvOS(.v13),
        .watchOS(.v6)
    ],
    products: [
        .library(
            name: "StateSet",
            targets: ["StateSet"]
        ),
    ],
    targets: [
        // C FFI module
        .target(
            name: "StateSetC",
            path: "Sources/StateSetC",
            publicHeadersPath: "include"
        ),
        // Swift wrapper
        .target(
            name: "StateSet",
            dependencies: ["StateSetC"],
            path: "Sources/StateSet"
        ),
        .testTarget(
            name: "StateSetTests",
            dependencies: ["StateSet"],
            path: "Tests/StateSetTests"
        ),
    ]
)
