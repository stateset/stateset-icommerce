// swift-tools-version:5.7
import PackageDescription

// The StateSet engine is a Rust library. Build it first, then point the
// linker at it:
//
//   cargo build -p stateset-swift --release
//   swift build -Xlinker -L$(pwd)/../../target/release
//   LD_LIBRARY_PATH=$(pwd)/../../target/release swift test -Xlinker -L$(pwd)/../../target/release
//
// (On macOS use DYLD_LIBRARY_PATH, or link the static libstateset_swift.a.)
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
        // C declarations of the native library (module map links stateset_swift).
        .systemLibrary(
            name: "StateSetC",
            path: "Sources/StateSetC"
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
