# Surfaces

Linux uses `linux-gtk`, a Rust-owned GTK4 surface without FFI or libadwaita.
Apple, Android, Windows, and web views compose separate conversation, composer,
and list observers. Navigation belongs to the views. Native SDK builds require
their platform toolchains. The VS Code prototype is under `vscode`; native
extension-host integration is deferred to its roadmap phase.
