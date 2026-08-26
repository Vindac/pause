// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "Pause",
    platforms: [
        .macOS(.v13)
    ],
    targets: [
        .executableTarget(
            name: "Pause",
            path: "Pause"
        ),
        .testTarget(
            name: "PauseTests",
            dependencies: ["Pause"],
            path: "Tests/PauseTests"
        )
    ]
)
