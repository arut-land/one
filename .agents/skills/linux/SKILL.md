---
name: linux
description: Select GTK4 and Relm4 guidance for Arut's native Linux desktop surface, including component state, messages, layout, accessibility, theme integration, and performance.
---

# Linux

Read [Relm4](relm4-app/SKILL.md) for component state, messages, lifecycle,
factories, and async work. Read [GTK UI engineering](gtk-ui-ux-engineer/SKILL.md)
for widget layout, desktop conventions, accessibility, and theme integration.
Load their supporting references only for the current task.

Use the repository's GTK4/Relm4 versions and Rust build workflow. The retained
libadwaita and C examples are conditional references, not instructions to add
libadwaita or replace Rust. Check GTK CSS capabilities against the selected
version; browser CSS examples are not automatically valid GTK CSS.

Keep widgets on their owning main context, preserve controller lifetimes and
message cancellation, and use actual virtualization for long transcripts.
Retain shared product behavior in Rust without duplicating it in presentation
models. Use system styling before introducing app-specific CSS.
