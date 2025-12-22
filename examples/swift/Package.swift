// swift-tools-version:5.7
import PackageDescription

let package = Package(
    name: "StateSetExample",
    platforms: [
        .macOS(.v12),
        .iOS(.v15)
    ],
    dependencies: [
        .package(path: "../../bindings/swift")
    ],
    targets: [
        .executableTarget(
            name: "StateSetExample",
            dependencies: [
                .product(name: "StateSet", package: "swift")
            ],
            path: ".",
            sources: ["BasicUsage.swift"]
        )
    ]
)
