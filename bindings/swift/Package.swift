// swift-tools-version: 6.2
//
// Tools 6.x means the Swift 6 language mode (strict concurrency) and SwiftPM's
// native Swift Testing support, which is what `swift test` runs here. 6.2 is
// the manifest level Xcode 26.0 ships, so the floor stays "Xcode 26 or newer".
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
            dependencies: [.product(name: "ArutFfi", package: "ArutFfi")],
            // Every type here exists to be read from a view body, so the module
            // is main-actor by default (SE-0466) and no declaration repeats it.
            swiftSettings: [.defaultIsolation(MainActor.self)]
        ),
        .testTarget(name: "ArutBindingsTests", dependencies: ["ArutBindings"]),
    ]
)
