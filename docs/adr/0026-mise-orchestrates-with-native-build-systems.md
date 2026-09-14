---
status: accepted
---

# mise orchestrates the task graph; every ecosystem keeps its own build system

One repository holds five build systems: cargo for 18 Rust crates, pnpm and Vite for the browser packages, Gradle for Android, xcodebuild and SwiftPM for Apple, MSBuild for WinUI, with BoltFFI generating the bindings between them. Something has to order those steps, pin the tools they run, and skip the ones whose inputs have not moved. mise already pinned the tools. It now also owns the task graph.

Tasks declare `sources` and `outputs`, and `task.source_freshness_hash_contents` makes freshness a blake3 comparison of file contents rather than of timestamps. Timestamps are the wrong signal here: a fresh clone, a branch switch, and a copied binary all rewrite mtimes without changing a byte, and CI restores the task state from a cache into a checkout whose files are all newer than it. Cargo tasks declare `outputs = { auto = true }`, so mise tracks a state marker and never archives `target/`; cargo keeps owning compilation freshness inside it, and `build:rust` names only the two binaries it produces. The generators declare their real outputs, because `bindings/generated/<target>`, the Fluent-derived resources, and the `dist` bundles are small, discrete, and expensive to reproduce. `check:proto` and `check:deps` declare no sources at all: `buf breaking` compares against the main branch and `cargo deny` compares against an advisory database, and neither moves when a file in the working tree does.

Tasks live next to the code they build. `surfaces/linux/mise.toml`, `surfaces/web/mise.toml`, `surfaces/android/mise.toml`, `surfaces/apple/mise.toml`, `surfaces/windows/mise.toml` and `bindings/ffi/mise.toml` are config roots listed under `[monorepo]`, so their tasks are addressed as `//surfaces/linux:run` and each task's `alias` keeps the repository-wide name (`surface:linux`) working from the root. Tool scoping survives the move: a JDK installs with the Android task and nowhere else, so `mise install` on a Linux or Windows machine still fetches Rust and nothing platform-specific.

mise infers a project graph from Cargo path dependencies and the pnpm workspace, which is what `mise run --affected` walks. Four directories belong to no package manager it reads, so `[monorepo.projects]` declares `bindings/kotlin`, `bindings/swift`, `bindings/dotnet` and the three native surfaces with their edges. The repository-wide gates stay unconditional in CI: they belong to the root project, which a crate-only change does not select, so an affected run would skip exactly the checks it exists to run.

`mr-boxington` wraps cargo through mise and caches rustc invocations in a content-addressed store shared by every checkout, keeping incremental compilation local. sccache was rejected for this repository earlier because it requires `CARGO_INCREMENTAL=0` and cannot cache crates that link, which is precisely `arut_ffi` (a cdylib) and `arut-i18n-macros` (a proc macro).

## Considered and rejected

**moon.** The closest alternative, and the one that was recommended before this decision. It hashes content by default and replays captured stdout for tasks with no file outputs, which mise's freshness check cannot do. It also means a second configuration system beside mise, a `moon.yml` per project, and a toolchain layer that has to be told to keep its hands off the versions mise already pins. It buys no faster Rust compile; cargo owns that either way.

**Nx and Turborepo.** Both draw their graph from `package.json`. Eighteen Rust crates are invisible to them, and their boundary rules are ESLint or TypeScript-only. Nx's Rust support lives in a community plugin outside the Nx organization.

**Bazel, Buck2, Pants.** One build language over five ecosystems means a build file per target, crate-universe repinning on every dependency change, and rules that must move in lockstep with the build tool. `rules_dotnet` cannot build WinUI, Pants has no C# or Swift backend, and Buck2's Swift and Kotlin rules are thin outside Meta. Native IDE integration, which is how the Apple and Windows surfaces are actually developed, breaks in all three. Months of rules work for one developer.

## Consequences

- Repeated `mise run check` and `mise run build` skip the tasks whose inputs are unchanged. Locally the second run of `check` costs seconds instead of the full gate; in CI the task state and the mr-boxington store are restored with `actions/cache`.
- A skip is only as honest as a task's declared `sources`. Adding a file kind that a gate reads means adding it to the matching input group in `mise.toml`; `mise run --force` ignores freshness when that is in doubt.
- Experimental mise features are load-bearing: input groups, the workspace project graph, and `--affected` all require `experimental = true`.
- `mise tasks ls --all` and `mise tasks deps` are the map of the repository's build steps. `mise tasks graph` shows the inferred project graph behind `--affected`.
- Android CI keeps its `paths:` filter. `--affected` decides after a runner has started and the NDK is fetched; the filter decides before.
