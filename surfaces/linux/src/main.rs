//! The Linux surface: relm4 over GTK4 without libadwaita, reading product
//! projections directly with no FFI (ADR 0013, ADR 0020, ADR 0021).
//! Non-Linux builds have no UI dependencies and run an empty main.

#[cfg(target_os = "linux")]
mod app;

fn main() {
    #[cfg(target_os = "linux")]
    app::run();
}
