---
status: accepted
amended-by: 0020 (the surface is written with relm4 over the same gtk4-rs widgets)
---

# Linux uses GTK4 in Rust without libadwaita, adapting to the running desktop

The Linux surface must feel native on whatever desktop is running rather than forcing one desktop's style onto the others. We decided to move from CXX-Qt to gtk4-rs and to not use libadwaita, because libadwaita ignores the system GTK theme by design. Theme, color scheme, and accent are read from the desktop portal; widgets are plain GTK4. The surface stays Rust-owned and reads projection types directly, with no FFI.

## Considered options

- **Keep CXX-Qt.** Rejected: a C++ build step, QML tooling, and a QObject threading model the rest of the stack does not need.
- **libadwaita.** Rejected for the default; a GNOME-specific variant may be added later if GNOME fidelity is judged worth a second Linux surface.
