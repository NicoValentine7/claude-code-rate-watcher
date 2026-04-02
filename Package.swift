// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "ClaudeCodeRateWatcher",
    platforms: [
        .macOS(.v13),
    ],
    products: [
        .library(
            name: "CCRWCore",
            targets: ["CCRWCore"]
        ),
        .executable(
            name: "ccrw",
            targets: ["ccrw"]
        ),
    ],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle.git", from: "1.27.3"),
        .package(url: "https://github.com/apple/swift-testing.git", from: "6.2.4"),
    ],
    targets: [
        .target(
            name: "CCRWCore",
            path: "Sources/CCRWCore"
        ),
        .executableTarget(
            name: "ccrw",
            dependencies: [
                "CCRWCore",
                .product(name: "Sparkle", package: "Sparkle"),
            ],
            path: "Sources/ccrw"
        ),
        .testTarget(
            name: "CCRWCoreTests",
            dependencies: [
                "CCRWCore",
                .product(name: "Testing", package: "swift-testing"),
            ],
            path: "Tests/CCRWCoreTests"
        ),
    ]
)
