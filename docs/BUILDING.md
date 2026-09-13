# Build workflows

Install mise, then run `mise install` from the repository root. This installs Rust and cargo-binstall. Other pinned tools install when their tasks run, so building Windows does not install Node, Gradle, or XcodeGen. Shared mise task templates hold the .NET, JavaScript, and binding-generator configuration.

Mise downloads BoltFFI's release binary. Cargo tools use cargo-binstall with source fallback disabled. Cargo still compiles the application's Rust code.

| Work | Command | System prerequisites |
| --- | --- | --- |
| Windows build, formatting and observation tests | `mise run check:windows` | Windows x64, Visual Studio C++ Build Tools and Windows SDK |
| Windows distributable folder | `mise run publish:windows` | Same as Windows build |
| Web development | `mise run surface:web` | None beyond mise |
| JavaScript checks and observation tests | `mise run check:ts` | None beyond mise |
| Linux app | `mise run surface:linux` | GTK 4.20+, C/C++ compiler and pkg-config |
| macOS build and Swift tests | `mise run check:apple` | Xcode |
| Android build | `mise run surface:android` | Android SDK 35, NDK and `ANDROID_NDK_HOME` |
| Kotlin observation tests | `mise run check:observers-kotlin` | No Android SDK required |

The root `build` task builds Linux, the daemon, and the web apps. Use the platform tasks on Windows or macOS. CI calls the same tasks and installs platform system dependencies separately.

## Generated code and incremental builds

Rust owns shared product behavior. BoltFFI generates native bindings; the handwritten binding adapters expose each language's observation conventions. Native package managers own their dependency graphs and compilation caches. Mise orders the steps and supplies their tools.

Binding packaging preserves existing generated directories before compilation. Cargo reuses unchanged compilation outputs, while BoltFFI regenerates and packages bindings on each invocation. Packaging is not transactional and still has a fixed cost. We do not use timestamp shortcuts around this step: copied DLLs retain their original timestamps, and a package manifest alone cannot establish that every generated file exists.

Windows Debug and Release packages use separate output directories selected by MSBuild's `Configuration`. `ffi:csharp` generates Debug; `ffi:csharp:release` generates Release. The corresponding build and publish tasks select them automatically. Run the platform mise task before using a native IDE on a fresh checkout or after changing Rust APIs.

`pnpm install` validates the workspace on every invocation and uses its existing store. Its lockfile is frozen automatically in CI. TypeScript 5.9.3 is pinned both for generated bindings and workspace checks. On Windows, the WASM task explicitly selects that package for BoltFFI's `npx tsc` subprocess.

XcodeGen uses its native `--use-cache` option. Xcode, Gradle, MSBuild, and Cargo keep their own build outputs between runs. `mise run i18n` uses the existing Rust generator's write-if-changed behavior for every platform's resources; `mise run check:i18n` verifies them without writing.

Use `mise run clean` only when intentionally discarding generated bindings, Cargo outputs, and JavaScript workspace installations/builds. Normal builds do not clean first. Native IDE build directories can be cleared through the corresponding native tool.

## Tool changes

Change tool versions in `mise.toml` and run `mise lock`. Keep pnpm's `packageManager` field and TypeScript's workspace catalog aligned with the build-tool versions. Package dependencies remain in Cargo, NuGet, pnpm, Gradle, and Swift manifests, with their native lockfiles where supported.

See [mise task configuration](https://mise.jdx.dev/tasks/task-configuration.html) for scoped tools and [XcodeGen usage](https://github.com/yonaskolb/XcodeGen/blob/2.46.0/README.md#usage) for project caching.
