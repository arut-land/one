---
name: winui-code-review
description: "Code quality review for WinUI 3 apps — state ownership, x:Bind correctness, accessibility, theming, security, and performance. Use for a focused WinUI review to catch issues that the compiler and UI tests won't find."
---

### When to Use

Review the changed behavior and its affected call sites; use build evidence when relevant. This catches quality issues that aren't build errors and aren't visible in UI tests — patterns that compile and run but are wrong, fragile, or slow.

### How to Review

Read through the project's XAML and C# files and check each section below. The `Microsoft.WindowsAppSDK.Analyzers` Roslyn analyzer ships with the `winui-dev-workflow` skill and is injected when `BuildAndRun.ps1` calls project-mode `winapp run`. The wrapper supplies a temporary file through the environment-backed MSBuild `CustomAfterDirectoryBuildProps` hook, preserving SDK composition and each project's normal `Directory.Build.props` discovery (including referenced projects), then restores the environment and removes the temporary file. Plain `winapp run`, `dotnet build`, and Visual Studio do **not** load the analyzer automatically; to enable it outside the wrapper, add the `<Analyzer Include="..." />` and `<Import Project="..." />` entries to the project's own `Directory.Build.props` (or wait for the planned NuGet package).

The analyzer catches a curated set of WinUI 3 / Windows App SDK issues with categorized 4-digit IDs:

* **WUI0xxx** — UWP → WinUI 3 API compatibility (`UwpXamlNamespace`, `Window.Current`, `CoreDispatcher`, `GetForCurrentView`)
* **WUI1xxx** — Migration-table data-driven hints (UWP API has WinAppSDK equivalent, no equivalent, feature-area hint)
* **WUI2xxx** — Runtime / layout / XAML pitfalls (raw `TabView` content, nested `x:Bind` without fallback, `x:Bind` without `Mode`, null `Converter`, missing `AutomationId`, attached-property syntax)
* **WUI3xxx** — MVVM patterns (old `[ObservableProperty]` field syntax)
* **WUI4xxx** — Interop (`WebView2` not initialized, removed ONNX Runtime GenAI APIs `WUI4101`-`WUI4103`)

Every diagnostic ships at `Warning` severity (no rule is `Error`) and includes a `helpLinkUri`. Suppress noise with `#pragma warning disable WUIxxxx` or `<NoWarn>` as usual — the analyzer's `SuppressionTests` verify that pragma suppression round-trips correctly.

### State and commands

- [ ] Use existing observable adapters; prefer CommunityToolkit.Mvvm generators where they remove boilerplate and the compiler supports them
- [ ] Use toolkit commands where suitable; retain intentional command adapters at shared-core boundaries
- [ ] No UI types in ViewModels (`SolidColorBrush`, `Visibility`, `BitmapImage`) — these belong in converters or XAML
- [ ] No business logic in code-behind — only navigation, dialog coordination, and event wiring
- [ ] `async Task` for async methods, `async void` only for event handlers
- [ ] Preserve item identity and apply targeted inserts, removes, moves, and updates; avoid clear-and-repopulate churn that resets selection and scroll

### x:Bind and Data Binding

- [ ] Prefer `{x:Bind}` for known types; use `{Binding}` for dynamic DataContext and reusable styles/templates
- [ ] `Mode=OneWay` or `TwoWay` set explicitly — `OneTime` default causes blank UI for dynamic data
- [ ] `x:DataType` set on every `DataTemplate` — required for compiled x:Bind
- [ ] No nested nullable paths (e.g., `ViewModel.Selected.Name`) without `FallbackValue`
- [ ] Command bindings can use OneTime (commands don't change) — don't add `Mode=OneWay` to `Command="{x:Bind}"`

### Accessibility

- [ ] Stable automation identifiers for test targets where names or semantics are insufficient; repeated items retain distinct identities
- [ ] `AutomationProperties.Name` on icon-only buttons and controls without visible text
- [ ] Semantic controls (`Button`, `HyperlinkButton`) — not clickable `Border`/`TextBlock`
- [ ] No information conveyed by color alone

### Theming

- [ ] All colors use `{ThemeResource}` brushes — no hardcoded `#FF0000` or `Color="Blue"`
- [ ] Typography uses built-in styles (`TitleTextBlockStyle`, `SubtitleTextBlockStyle`, `BodyTextBlockStyle`, `CaptionTextBlockStyle`) ; introduce a named product token only when a deliberate design needs it
- [ ] Spacing uses 4px grid multiples (4, 8, 12, 16, 24, 32, 48)
- [ ] Corner radius uses `ControlCornerRadius` / `OverlayCornerRadius` ; intentional product shapes, such as the composer pill, can use a named local resource
- [ ] Styles referenced with `{StaticResource}` not `{ThemeResource}` (except for brush usage sites)

### Security

- [ ] No secrets, API keys, or tokens in source code
- [ ] No `Process.Start` with unsanitized user input
- [ ] External input validated and sanitized before use
- [ ] File paths from user input not used directly in `File.Delete` / `File.WriteAllText` without validation

### Performance

- [ ] Long or dynamic lists use `ListView`/`GridView` (virtualized), not `StackPanel` with `foreach`
- [ ] `x:Load` for content that's not always visible (e.g., dialogs, secondary panels)
- [ ] Use asynchronous I/O and move CPU work off the UI thread; `async` alone does not move execution to a worker
- [ ] No `.Result` / `.Wait()` / `.GetAwaiter().GetResult()` — these deadlock the UI thread
- [ ] `using` statements on all disposable objects (`Model`, `Tokenizer`, `InferenceSession`, `Generator`)

### Globalization

- [ ] User-facing strings use `x:Uid` in XAML and `ResourceLoader` in C# — not hardcoded
- [ ] String resources in `Strings/en-us/Resources.resw` (not `.resx`)
- [ ] Date/number formatting uses `CultureInfo.CurrentCulture` — not hardcoded formats
- [ ] Layout supports RTL (`FlowDirection` inherited from root, no absolute positioning that breaks in RTL)
- [ ] No string concatenation for user-facing messages — use `string.Format` or interpolation with resource strings

### Review Report

After reviewing, summarize:
1. **Issues found:** List each with file, line, and what's wrong
2. **Severity:** Error (must fix), Warning (should fix), or Note (could improve)
3. **Suggested fixes:** Specific code changes for each issue

### References

For detailed rules with code examples, see `references/quality-rules.md` — covers performance deep dives (x:Phase, layout optimization), security (PasswordVault, DPAPI, WebView2 hardening), accessibility (keyboard nav, screen readers), code quality (.editorconfig, naming), and globalization (x:Uid patterns, RTL, pluralization).
