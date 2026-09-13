---
name: apple
description: Select Swift, SwiftUI, and Apple platform guidance for native macOS or iOS work, including state, composition, motion, accessibility, performance, and packaging. Other Apple OS guides apply only when that target is requested.
---

# Apple

Read the relevant child skill and its task-specific references. Do not load
every implementation guide or OS guide together.

| Task | Skill |
| --- | --- |
| SwiftUI implementation and state | [SwiftUI expert](swiftui-expert-skill/SKILL.md) |
| SwiftUI code review | [SwiftUI pro](swiftui-pro/SKILL.md) |
| View/component APIs | [UI patterns](swiftui-ui-patterns/SKILL.md) |
| Existing view decomposition | [View refactor](swiftui-view-refactor/SKILL.md) |
| Runtime performance and Instruments | [Performance audit](swiftui-performance-audit/SKILL.md) |
| Supported Liquid Glass work | [Liquid Glass](swiftui-liquid-glass/SKILL.md) |
| Swift modeling and concurrency | [Write Swift](write-swift/SKILL.md) |
| Mac conventions | [macOS design](macos-design-guidelines/SKILL.md) |
| iPhone conventions | [iOS design](ios-design-guidelines/SKILL.md) |
| iPad adaptation | [iPadOS design](ipados-design-guidelines/SKILL.md) |
| SwiftPM-only app packaging | [SPM packaging](macos-spm-app-packaging/SKILL.md) |
| Maintaining API references | [Update APIs](update-swiftui-apis/SKILL.md) |

For an explicitly requested additional target, select [tvOS](tvos-design-guidelines/SKILL.md),
[watchOS](watchos-design-guidelines/SKILL.md), or [visionOS](visionos-design-guidelines/SKILL.md).

Read actual deployment targets and Swift compiler settings before applying new
APIs, Observation, or concurrency examples. Preserve compatible observable and
command-backed bindings to the shared Rust core. Choose ownership from the
product's data flow; sample MV/MVVM preferences do not require a migration.
Arut's existing Xcode project workflow takes precedence over SPM-only scaffolding.
