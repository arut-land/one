// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ArutBindings",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        .library(name: "ArutBindings", targets: ["ArutBindings"]),
    ],
    dependencies: [
        .package(name: "ArutFfi", path: "../generated/apple"),
    ],
    targets: [
        .target(
            name: "ArutBindings",
            dependencies: [.product(name: "ArutFfi", package: "ArutFfi")]
        ),
        .testTarget(name: "ArutBindingsTests", dependencies: ["ArutBindings"]),
    ]
)
