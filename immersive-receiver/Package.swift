// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ImmersiveReceiver",
    platforms: [
        .macOS(.v14),
        .iOS(.v17)
    ],
    products: [
        .library(
            name: "ImmersiveReceiverCore",
            targets: ["ImmersiveReceiverCore"]
        ),
        .executable(
            name: "ImmersiveReceiverMac",
            targets: ["ImmersiveReceiverMac"]
        )
    ],
    dependencies: [],
    targets: [
        .target(
            name: "ImmersiveReceiverCore",
            dependencies: [],
            path: "Sources/ImmersiveReceiverCore"
        ),
        .executableTarget(
            name: "ImmersiveReceiverMac",
            dependencies: ["ImmersiveReceiverCore"],
            path: "ImmersiveReceiverMac",
            exclude: ["Assets.xcassets", "Info.plist"]
        ),
        .testTarget(
            name: "ImmersiveReceiverCoreTests",
            dependencies: ["ImmersiveReceiverCore"],
            path: "Tests/ImmersiveReceiverCoreTests"
        )
    ]
)

