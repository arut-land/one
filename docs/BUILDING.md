# Build workflows

Install mise, then run `mise install` from the repository root. This installs Rust and cargo-binstall. Other pinned tools install when their tasks run, so building Windows does not install Node, Gradle, or XcodeGen. Shared mise task templates hold the .NET, JavaScript, JVM, and binding-generator configuration; the JVM template pins one JDK (21) and one Gradle (8.12.1), and C# formatting uses `dotnet format` from the pinned SDK rather than a separate formatter tool.

Mise downloads BoltFFI's release binary. Cargo tools use cargo-binstall with source fallback disabled. Cargo still compiles the application's Rust code.

| Work | Command | System prerequisites |
| --- | --- | --- |
| Everything CI checks | `mise run check` | GTK 4.20+ for the Rust build; network for cargo-deny |
| Regenerate localization resources | `mise run generate` | None beyond mise |
| Windows build, formatting and observation tests | `mise run check:windows` | Windows x64, Visual Studio C++ Build Tools and Windows SDK |
| Windows distributable folder | `mise run publish:windows` | Same as Windows build |
| Web development | `mise run surface:web` | None beyond mise |
| JavaScript checks and store tests | `mise run check:ts` | None beyond mise |
| Linux app | `mise run surface:linux` | GTK 4.20+, C/C++ compiler and pkg-config |
| macOS build and Swift tests | `mise run check:apple` | Xcode |
| Android build | `mise run surface:android` | Android SDK 35, NDK and `ANDROID_NDK_HOME` |

The root `build` task builds Linux, the daemon, and the web apps. Use the platform tasks on Windows or macOS. CI calls the same tasks and installs platform system dependencies separately.

CI has four workflows' worth of jobs. The Linux job runs `mise run check` and `mise run build` in a Fedora container, because the GitHub Ubuntu image ships GTK 4.14 and the surface needs 4.20. Dependency policy is a separate job running `mise run check:deps`, so an advisory-database fetch failure neither masks a compile failure nor blocks the build. Windows runs `mise run check:windows`; macOS runs `mise run check:apple` on pull requests and version tags; Android has its own workflow, filtered by GitHub's `paths:`.

## Generated code and incremental builds

Rust owns shared product behavior. BoltFFI generates native bindings, including the async stream type each ecosystem expects, so no per-language observation class is written by hand. Native package managers own their dependency graphs and compilation caches. Mise orders the steps and supplies their tools.

Binding packaging preserves existing generated directories before compilation. Cargo reuses unchanged compilation outputs, while BoltFFI regenerates and packages bindings on each invocation. Packaging is not transactional and still has a fixed cost. We do not use timestamp shortcuts around this step: copied DLLs retain their original timestamps, and a package manifest alone cannot establish that every generated file exists.

Windows Debug and Release packages use separate output directories selected by MSBuild's `Configuration`. `mise run ffi:csharp` generates Debug and `mise run ffi:csharp -- --release` generates Release. The corresponding build and publish tasks select them automatically. Run the platform mise task before using a native IDE on a fresh checkout or after changing Rust APIs.

`pnpm install` validates the workspace on every invocation and uses its existing store. Its lockfile is frozen automatically in CI. TypeScript 5.9.3 is pinned both for generated bindings and workspace checks. On Windows, the WASM task explicitly selects that package for BoltFFI's `npx tsc` subprocess.

XcodeGen uses its native `--use-cache` option. Xcode, Gradle, MSBuild, and Cargo keep their own build outputs between runs. `mise run generate` writes every platform's resources only when their content changes; `arut-dev check`, inside `mise run check:rust`, regenerates them into a temporary location and fails when the checked-in output differs.

Use `mise run clean` only when intentionally discarding generated bindings, Cargo outputs, and JavaScript workspace installations/builds. Normal builds do not clean first. Native IDE build directories can be cleared through the corresponding native tool.

## Tool changes

Change tool versions in `mise.toml` and run `mise lock`. Keep pnpm's `packageManager` field and TypeScript's workspace catalog aligned with the build-tool versions. Package dependencies remain in Cargo, NuGet, pnpm, Gradle, and Swift manifests, with their native lockfiles where supported.

See [mise task configuration](https://mise.jdx.dev/tasks/task-configuration.html) for scoped tools and [XcodeGen usage](https://github.com/yonaskolb/XcodeGen/blob/2.46.0/README.md#usage) for project caching.
