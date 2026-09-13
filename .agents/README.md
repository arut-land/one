# Arut project skills

Platform skills live under `skills/windows`, `skills/apple`, `skills/web`,
`skills/linux`, and `skills/android`. Each group has a small discovery entrypoint
that routes to task-specific child skills. Load only the children needed for
the task; the CLI lists groups rather than every nested child. Shared guidance
is grouped under `rust`, `engineering`, `desktop`, `ui`, `ux`, `motion`, and
`writing`, using the same selective loading pattern.

The original folders were moved with their references, helpers,
examples, and licenses. [skill-sources.json](skill-sources.json) records
upstream sources, original hashes, current paths, and the baseline commit.
These are locally maintained copies; compare upstream changes before applying
them. They are excluded from the installer's root-level update lock so an update
does not recreate the old flat collection. `../skills-lock.json` is empty until
another upstream skill is installed. Restore this maintained collection through Git.

`upstream/ehmo-platform-design-skills/` preserves the original platform skill
directories from the initial installation. Active copies use the CLI's canonical
names in `skills/`. The archive also preserves Ehmo's web guidance, whose skill
name collides with Vercel's `web-design-guidelines`. Consult that web reference
when useful; it is not a second active entrypoint.

`upstream/retired-workflows/` preserves WPF/MSBuild migration, upstream PR/session
tooling, and older React migration material outside active discovery.
`upstream/swiftui-pro-nested-copy/` preserves the duplicate SwiftUI entrypoint.

Use the relevant skills for a task, rather than applying every overlapping rule.
Arut's requirements and accepted architecture decisions take precedence over
generic examples. Installing a skill does not authorize a framework migration,
dependency change, deployment, or external communication.

Select skills by the decision the task needs:

- Windows: `winui-design` for controls/materials/motion, `winui-app` for
  implementation/lifecycle, `winui-code-review` for review, and
  `winui-dev-workflow` for builds. Load setup, testing, or packaging for those tasks.
- Apple: `swiftui-expert-skill` for implementation/state, `swiftui-pro` for review,
  view patterns/refactoring for composition, and performance audit for measured
  problems. Select the relevant OS guide; gate Liquid Glass by deployment target.
- Android: Compose is the default UI stack. Choose state/effects, components,
  focus, motion, testing, or performance as needed. Kotlin and Gradle have their
  own skills. Manifest and resource XML remain valid infrastructure.
- Linux: Relm4 for components/messages/lifecycle; GTK UI engineering for layout
  and accessibility. Check examples against the actual GTK/Relm4 versions;
  libadwaita is not an implicit dependency.
- Rust: idioms, ownership, async behavior, traits, refactoring, and tests.
- Engineering: state modeling, architecture, code review, and simplification.
  Planning, agent orchestration, and remote project workflows retain their
  explicit-invocation scope.
- Desktop: shared keyboard, pointer, window, and accessibility guidance.
- UI: visual hierarchy, composition, design taste, and targeted redesigns.
- UX: usability heuristics, affordances, and product experiments.
- Motion: interaction feedback, animation implementation, vocabulary, and audits.
  Web and Expo examples apply only to those stacks; native motion uses platform skills.
- Web: React composition and performance, web guidelines, browser testing, and
  supported view transitions. Fluent React belongs here, separate from WinUI.
- Writing: unslop and grounded writing.

Load references on demand. A Compose state fix usually needs the state skill,
not animation, benchmarks, release, and testing skills automatically. Use the
repository's mise tasks, package pins, and deployment targets. Keep shared
product behavior in Rust and platform interaction state on its surface where
appropriate.

Add a skill from the repository root using project scope:

```powershell
bunx --bun skills add owner/repo --skill skill-name --agent codex --yes
bunx --bun skills list --agent codex --json
```

Use `--full-depth` when a repository nests skills inside plugins. Inspect the
installed files and lockfile after installation; a successful command can still
select only part of a repository's skill collection.

The registry-listed `mhagrelius/dotfiles` skills `developing-gtk-apps` and
`designing-gnome-ui` could not be fetched during installation. Relm4 and GTK UI
engineering guidance are available from other installed sources.
