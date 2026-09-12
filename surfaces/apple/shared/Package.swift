// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ArutSurface",
    platforms: [
        .iOS(.v16),
        .macOS(.v13),
    ],
    products: [
        .library(name: "ArutSurface", targets: ["ArutSurface"]),
    ],
    dependencies: [
        .package(name: "ArutBindings", path: "../../../bindings/swift"),
    ],
    targets: [
        .target(
            name: "ArutSurface",
            dependencies: [.product(name: "ArutBindings", package: "ArutBindings")]
        ),
    ]
)
