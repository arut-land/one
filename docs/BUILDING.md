# Build workflows

Install mise, then run `mise install` from the repository root. That installs Rust, cargo-binstall, and mr-boxington. Every other pinned tool installs with the task that consumes it, so building Windows never installs Node, Gradle, or XcodeGen. Shared mise task templates hold the .NET, JavaScript, JVM, and binding-generator configuration; the JVM template pins one JDK (21) and one Gradle (8.12.1), and C# formatting uses `dotnet format` from the pinned SDK rather than a separate formatter tool.

Mise downloads BoltFFI's release binary. Cargo tools use cargo-binstall with source fallback disabled. Cargo still compiles the application's Rust code.

| Work | Command | System prerequisites |
| --- | --- | --- |
| Everything CI checks | `mise run check` | GTK 4.22+ for the Rust build; network for cargo-deny |
| Build everything this machine can build | `mise run build` | Same as above |
| Regenerate localization resources and handle descriptors | `mise run generate` | None beyond mise |
| Windows build, formatting and observation tests | `mise run check:windows` | Windows x64, Visual Studio C++ Build Tools and Windows SDK |
| Windows distributable folder | `mise run publish:windows` | Same as Windows build |
| Web development | `mise run surface:web` | None beyond mise |
| JavaScript checks and store tests | `mise run check:ts` | None beyond mise |
| Linux app | `mise run surface:linux` | GTK 4.22+, C/C++ compiler, pkg-config and `glib-compile-resources` (the GLib development package) |
| macOS build and Swift tests | `mise run check:apple` | Xcode |
| Android build | `mise run surface:android` | Android SDK 35, NDK and `ANDROID_NDK_HOME` |
| Wasm, Apple, Android, .NET bindings | `mise run ffi:wasm` and `ffi:apple`, `ffi:android`, `ffi:csharp` | Per target: Xcode, the NDK, the .NET SDK |

`mise run check` depends on `check:rust`, `check:core-graph`, `check:proto`, `check:deps` and `check:ts`. `mise run build` builds the Linux surface, the daemon, and the web, Chromium, and VS Code bundles. Use the platform tasks on Windows or macOS. CI calls the same tasks and installs platform system dependencies separately.

CI has four workflows' worth of jobs. The Linux job runs `mise run check` and `mise run build` in a Fedora container, because the GitHub Ubuntu image ships GTK 4.14 and the surface needs 4.22. Dependency policy is a separate job running `mise run check:deps`, so an advisory-database fetch failure neither masks a compile failure nor blocks the build. Windows runs `mise run check:windows`; macOS runs `mise run check:apple` on pull requests and version tags; Android has its own workflow, filtered by GitHub's `paths:`.

## Where tasks live

Tasks that belong to one surface live in that surface's own `mise.toml`: `surfaces/linux`, `surfaces/web`, `surfaces/android`, `surfaces/apple`, `surfaces/windows`, and `bindings/ffi`. mise addresses them as `//surfaces/linux:run`, and each one carries an `alias` that keeps the repository-wide name working from the root, so `mise run surface:linux` and `mise run //surfaces/linux:run` are the same task. Repository-wide tasks stay in the root `mise.toml`, and the two long check scripts are executable files under `mise-tasks/`, where their contents are hashed like any other source.

```
mise tasks ls          # tasks defined at the root
mise tasks ls --all    # every task in the repository, with its path
mise tasks deps check  # the dependency tree behind a task
mise tasks graph       # the inferred project graph behind --affected
mise watch check:ts    # re-run a task when its declared sources change
```

Inside one of those six directories a bare task name resolves in that directory's namespace, so `mise run check` there reports `no task //surfaces/linux:check`. Write `mise run //:check` for a repository-wide task from inside a surface, or run it from the root.

`mise run --affected //surfaces/android:build` runs a surface task only when the project graph says a change reaches it. The graph comes from Cargo path dependencies and the pnpm workspace; the directories no package manager describes (`bindings/kotlin`, `bindings/swift`, `bindings/dotnet`, and the three native surfaces) are declared under `[monorepo.projects]` in the root config. Revisions default to `HEAD~1...HEAD`; against a branch point use `mise run --affected --affected-base origin/main <task>`. The repository-wide gates do not take part: they belong to the root project, so run `mise run check` unconditionally.

## How caching works

Each task declares the files it reads as `sources`. mise compares a blake3 hash of those files against the hash recorded for the last successful run, and skips the task when nothing changed. Hashing, rather than mtime comparison, is on deliberately (`task.source_freshness_hash_contents`): a fresh clone, a branch switch, and a copied binary all rewrite timestamps without changing content.

What each task declares, and why:

- Cargo tasks list the manifests, lockfile, every crate's `src`, the `.proto` and `.ftl` sources, and the generated resources that `arut-dev check` validates. Their `outputs` are `{ auto = true }`, a state marker outside the repository. `target/` is never declared as an output; cargo owns freshness inside it. `build:rust` is the exception and names the two binaries it produces.
- `ffi:wasm`, `ffi:apple`, `ffi:android` and `ffi:csharp` list the BoltFFI configuration and every core crate they read, and declare `bindings/generated/<target>` as their output. Debug and Release C# packages are separate directories, so `ffi:csharp` tracks them separately.
- `generate` lists the Fluent sources, the handle descriptors, and the generator's own code, and declares every resource file it writes.
- `install` and the TypeScript tasks list the workspace manifests, lockfile, and sources; `install` declares `node_modules/.modules.yaml`, so deleting `node_modules` re-installs even when the lockfile has not moved.
- `check:proto` and `check:deps` declare no sources and always run. `buf breaking` compares against the main branch and `cargo deny` against an advisory database; neither moves when a working-tree file does.

A skip is only as trustworthy as the declared sources. When a gate starts reading a kind of file that no input group mentions, add it to the group in `mise.toml`; `mise run --force <task>` ignores freshness in the meantime. Deleting a declared output also re-runs its task.

Below mise, each build system keeps its own cache. mr-boxington wraps cargo and caches rustc invocations in a content-addressed store shared by every checkout and worktree, leaving incremental compilation local to each `target/`; `mbx stats` reports what it saved and `mbx gc` reclaims space. XcodeGen uses its native `--use-cache`. Xcode, Gradle, MSBuild, Vite, and pnpm's store keep their own outputs between runs. Use `mise run clean` only when intentionally discarding generated bindings, Cargo outputs, and JavaScript installs and builds.

Windows Debug and Release packages use separate output directories selected by MSBuild's `Configuration`. `mise run ffi:csharp` generates Debug and `mise run ffi:csharp:release` generates Release. Separate tasks give each configuration explicit outputs and independent freshness checks. Both regenerate localization and handle descriptors before packaging bindings. The corresponding build and publish tasks select them automatically. Run the platform mise task before using a native IDE on a fresh checkout or after changing Rust APIs.

TypeScript 7.0.2 is pinned both for generated bindings and workspace checks, in `mise.toml`'s `typescript_version` variable and the pnpm catalog, which must match; `boltffi_version` pins the generator and must match the `@boltffi/runtime` catalog pin. On Windows, the wasm task explicitly selects that package for BoltFFI's `npx tsc` subprocess. Elsewhere BoltFFI finds `tsc` on PATH, and `bindings/ffi/wrappers/tsc` is first there: BoltFFI 0.30 names the generated file on the command line, which TypeScript 7 refuses while the workspace's tsconfig.json is discoverable, and it passes no library list for the ES2021 and DOM globals the generated code uses, so the wrapper adds both until BoltFFI does. The task also stages `@boltffi/runtime` beside the output for that check, because the workspace install that links the real one also links the package the task generates, so it cannot run first.

## Tool changes

Change tool versions in `mise.toml` and run `mise lock`. Keep pnpm's `packageManager` field and TypeScript's workspace catalog aligned with the build-tool versions. Package dependencies remain in Cargo, NuGet, pnpm, Gradle, and Swift manifests, with their native lockfiles where supported.

See [mise task configuration](https://mise.jdx.dev/tasks/task-configuration.html) for scoped tools, sources and outputs, [mise monorepo tasks](https://mise.jdx.dev/tasks/monorepo.html) for the `//path:task` syntax and the project graph, and [XcodeGen usage](https://github.com/yonaskolb/XcodeGen/blob/2.46.0/README.md#usage) for project caching.
