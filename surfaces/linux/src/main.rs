//! Native Linux UI built with relm4 over GTK4, without libadwaita.
//!
//! The root owns the desktop executor and ChildHost and binds feature clients to IPC.
//! The local observation module refreshes GObject properties on GLib and binds
//! them to widgets. Each watch owns one native GLib task, internally a task source
//! plus gtk-rs's child waker source, with no timer or per-invalidation task.
//! A keyed gio::ListModel fetches only transcript additions; GtkListView recycles
//! message and conversation widgets. The sidebar filters its model with SearchEntry.
//! Decoration policy lives in decorations.rs. An unmapped ordinary GTK window
//! reveals GTK's compositor preference without forcing client decorations.
//! Server frames win; portal button-layout then gtk-decoration-layout determine
//! client button positions. Tilers ignore compatibility GNOME portal defaults.
//! Hyprland/Sway/river/niri and empty layouts use a real toolbar with no window
//! controls. KDE uses compositor frames when offered and never gets an invented
//! GNOME or Breeze layout. Only GNOME has a GNOME fallback. Settings and toplevel
//! changes re-evaluate the policy. ShortcutController exposes gio actions.
//! The split collapses below 720 logical pixels; the reading column caps at 880.
//! Composer buffers and ordered command consumers survive conversation switches,
//! as do transcript reading positions. Only navigation is persisted here, under
//! XDG_STATE_HOME/arut/linux-ui; the node owns durable conversations and drafts.
//! Chat and composer errors are typed enums from the core (ADR 0016); strings.rs
//! maps every `ChatError`/`ComposerError`/`NodeFailure` variant, plus the typed
//! availability and connect-time RPC statuses, to a message id in the shared
//! Fluent source and asks `arut_i18n` for the sentence (ADR 0022). The language
//! list comes from GLib, so this surface follows the desktop's own setting.
//! GTK reads scheme and contrast from its portal settings. Theme colors come
//! from named GTK colors; ashpd supplies live accent and reduced-motion settings.
//! Only Arut classes receive additional CSS, and icons come from the desktop.
//! `mise run review:linux` uses a separate app identity, data directory, workspace,
//! and headless Hyprland output. ARUT_REVIEW=1 enables fixture actions only for
//! this run; real node messages and native TextBuffers exercise the normal UI.
//! The error fixture injects SessionError::Unavailable through Fluent accessors.
//! Review runs with G_DEBUG=fatal-criticals, exercises native typing/send/search
//! and draft switches, and runs the unmapped GTK component test. ARUT_REVIEW_GDB=1
//! records a native backtrace on failure. Group continuations are display-only
//! fixtures after the real echo-node transcript, never persisted product data.
//! Transcript timestamps sit outside bubbles, with one heading per speaker group,
//! four-pixel continuation spacing and naturally sized, opposite-aligned bubbles.
//! Non-Linux builds have no UI dependencies and run an empty main.

#[cfg(target_os = "linux")]
mod app;

fn main() {
    #[cfg(target_os = "linux")]
    app::run();
}
