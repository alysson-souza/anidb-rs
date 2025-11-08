// swift-tools-version: 5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "AniDBClient",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        // Products define the executables and libraries a package produces, making them visible to other packages.
        .library(
            name: "AniDBClient",
            targets: ["AniDBClient"]),
        .executable(
            name: "anidb-example",
            targets: ["AniDBExample"]),
    ],
    targets: [
        // System library target that links to the C library
        .systemLibrary(
            name: "CAniDB",
            path: "Sources/CAniDB",
            pkgConfig: "anidb_client_core",
            providers: [
                .apt(["libanidb-dev"]),
                .brew(["anidb"])
            ]
        ),
        // Main Swift wrapper library
        .target(
            name: "AniDBClient",
            dependencies: ["CAniDB"],
            path: "Sources/AniDBClient",
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        // Test target
        .testTarget(
            name: "AniDBClientTests",
            dependencies: ["AniDBClient"],
            path: "Tests/AniDBClientTests",
            resources: [
                .copy("Resources")
            ]
        ),
        // Example application
        .executableTarget(
            name: "AniDBExample",
            dependencies: ["AniDBClient"],
            path: "Sources/AniDBExample"
        ),
    ]
)