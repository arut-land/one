---
status: accepted
amends: 0013
---

# The Linux surface is written with relm4 over gtk4-rs

Plain gtk4-rs leaves every signal connection, widget update, and state flow to be wired by hand. relm4 is a mature Elm-style component layer over the same GTK4 widgets: a component declares its model, its messages, and its view, and the library owns the wiring. We decided to write the Linux surface with relm4, still without libadwaita, still reading projection types directly with no FFI. ADR 0013's choices (GTK4, desktop-adaptive theming, no libadwaita) stand; this only changes how the surface is written.

## Consequences

- The Linux surface stays Rust-owned and becomes the reference for how a surface composes scope handles.
- relm4's async components run on GLib's main context, so the surface still needs no executor of its own.
