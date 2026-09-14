//! The bundled icon set, registered with GTK's icon theme so the surface asks
//! for `arut-*-symbolic` names and the app icon like any theme icon. The
//! resource itself is compiled by `build.rs` from `data/icons/lucide` and
//! `data/icons/hicolor`; the theme recolors the symbolic ones, so they follow
//! the foreground, contrast and accent like GTK's own.
use gtk::{gdk, gio};
use std::sync::Once;

/// The resource prefix `build.rs` compiles the icons under.
const RESOURCE_PATH: &str = "/dev/arut/Arut/icons";

pub fn install(display: &gdk::Display) {
    static REGISTERED: Once = Once::new();
    REGISTERED.call_once(|| {
        gio::resources_register_include!("arut.gresource").expect("bundled icons");
    });
    gtk::IconTheme::for_display(display).add_resource_path(RESOURCE_PATH);
}
