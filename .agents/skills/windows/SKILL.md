---
name: windows
description: Select task-specific guidance for Arut's native Windows WinUI 3 and C# surface, including design, state adapters, implementation, builds, testing, and packaging. Use for Windows desktop work; Fluent React belongs to web.
---

# Windows

Read the child skill that owns the current task, then only its relevant
references. Add another child when the task needs a separate concern covered.
Do not load the entire group.

| Task | Skill |
| --- | --- |
| Controls, layout, materials, titlebar, motion, accessibility | [WinUI design](winui-design/SKILL.md) |
| App structure, lifecycle, windowing, native implementation | [WinUI app](winui-app/SKILL.md) |
| C# and XAML review, bindings, observable adapters | [Code review](winui-code-review/SKILL.md) |
| Build, run, crash diagnosis | [Development workflow](winui-dev-workflow/SKILL.md) |
| Missing machine prerequisites | [Setup](winui-setup/SKILL.md) |
| UI Automation and real input verification | [UI testing](winui-ui-testing/SKILL.md) |
| MSIX, signing, distribution | [Packaging](winui-packaging/SKILL.md) |

Use Arut's mise tasks, package pins, and actual packaging model. Preserve shared
Rust product state and native UI ownership; do not introduce a second product
model to satisfy a sample architecture. Prefer platform controls and resources,
with intentional product variations where required. Verify version-specific
examples against the selected SDK before copying them.
