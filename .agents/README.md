# Arut project skills

Skills in `skills/` are installed for this repository. `../skills-lock.json`
records upstream sources, paths, and content hashes. Keep upstream copies intact
until we deliberately consolidate them into Arut-specific skills.

`upstream/ehmo-platform-design-skills/` preserves the original platform skill
directories from the initial installation. Active copies use the CLI's canonical
names in `skills/`. The archive also preserves Ehmo's web guidance, whose skill
name collides with Vercel's `web-design-guidelines`.

Use the relevant skills for a task, rather than applying every overlapping rule.
Arut's requirements and accepted architecture decisions take precedence over
generic examples. Installing a skill does not authorize a framework migration,
dependency change, deployment, or external communication.

The collection covers:

- Windows: WinUI design, implementation, testing, packaging, and review.
- Apple: SwiftUI state, composition, performance, motion, and platform design.
- Android: Compose state, effects, components, focus, testing, and performance.
- Linux: GTK UI engineering and Relm4 components.
- Shared product work: state modeling, usability, microinteractions, and design review.
- Engineering: Rust, async behavior, architecture, code review, and simplification.
- Web: React composition, performance, modernization, accessibility, and visual design.
- Writing and cleanup: unslop, deslop, grounded writing, and UI review.

Add a skill from the repository root using project scope:

```powershell
bunx --bun skills add owner/repo --skill skill-name --agent codex --yes
```

Use `--full-depth` when a repository nests skills inside plugins. Inspect the
installed files and lockfile after installation; a successful command can still
select only part of a repository's skill collection.

The registry-listed `mhagrelius/dotfiles` skills `developing-gtk-apps` and
`designing-gnome-ui` could not be fetched during installation. Relm4 and GTK UI
engineering guidance are available from other installed sources.
