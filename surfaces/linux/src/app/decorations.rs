//! Desktop decoration policy. GTK's native window negotiation wins; the
//! desktop's own `gtk-decoration-layout` follows, with environment signals only
//! as a fallback. GTK already reads the portal's `button-layout` into that
//! setting, so nothing here talks to the portal.
use gtk::{glib, prelude::*};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Policy {
    Server,
    Toolbar,
    Client(String),
}

#[derive(Default)]
struct Desktop {
    names: String,
    hyprland: bool,
    sway: bool,
    kde: bool,
}
impl Desktop {
    fn read() -> Self {
        Self {
            names: std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_default()
                .to_lowercase(),
            hyprland: std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some(),
            sway: std::env::var_os("SWAYSOCK").is_some(),
            kde: std::env::var("KDE_FULL_SESSION").is_ok_and(|value| value == "true"),
        }
    }
    fn tiling(&self) -> bool {
        self.hyprland
            || self.sway
            || self
                .names
                .split([':', ';'])
                .any(|name| matches!(name, "hyprland" | "sway" | "river" | "niri"))
    }
}

fn resolve(server: bool, layout: Option<&str>, desktop: &Desktop) -> Policy {
    if server {
        return Policy::Server;
    }
    // Tilers can expose a compatibility GNOME settings portal. Its default
    // close button is not a request to add a title bar to tiled windows.
    if desktop.tiling() {
        return Policy::Toolbar;
    }
    if let Some(layout) = layout {
        if !layout
            .split([',', ':'])
            .any(|button| matches!(button.trim(), "close" | "minimize" | "maximize"))
        {
            return Policy::Toolbar;
        }
        return Policy::Client(layout.to_owned());
    }
    if desktop.kde || desktop.names.split(':').any(|name| name == "kde") {
        return Policy::Toolbar;
    }
    if desktop.names.split(':').any(|name| name == "gnome") {
        // Only GNOME receives GNOME's conventional fallback, never other desktops.
        return Policy::Client(":close".into());
    }
    Policy::Toolbar
}

/// GDK's prefers-SSD function is private. An unmapped ordinary `GtkWindow`
/// exposes GTK's negotiated choice through its documented CSD CSS classes.
/// No custom titlebar is installed on this probe, so it cannot force CSD.
fn prefers_server_frames(display: &gtk::gdk::Display) -> bool {
    let probe = gtk::Window::builder().display(display).build();
    WidgetExt::realize(&probe);
    let server = !probe.has_css_class("csd") && !probe.has_css_class("solid-csd");
    probe.destroy();
    server
}

/// The desktop's explicit layout, excluding GTK's compiled default: only the
/// desktop fallback in `resolve` may invent button positions.
///
/// Reads the typed property rather than `DisplayExtManual::get_setting`: gtk-rs
/// 0.11.4 passes an uninitialized `GValue` to GDK, whose Wayland string setting
/// reader requires `G_TYPE_STRING` (fatal `g_value_set_string`).
fn desktop_layout(settings: &gtk::Settings) -> Option<glib::GString> {
    settings.gtk_decoration_layout().filter(|layout| {
        settings
            .find_property("gtk-decoration-layout")
            .is_some_and(|spec| {
                spec.default_value()
                    .get::<Option<String>>()
                    .ok()
                    .flatten()
                    .as_deref()
                    != Some(layout.as_str())
            })
    })
}

struct State {
    window: glib::WeakRef<gtk::ApplicationWindow>,
    header: gtk::HeaderBar,
    toolbar: gtk::Box,
    content: gtk::Box,
    current: Option<Policy>,
}
impl State {
    fn refresh(&mut self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let display = WidgetExt::display(&window);
        let settings = gtk::Settings::for_display(&display);
        let next = resolve(
            prefers_server_frames(&display),
            desktop_layout(&settings).as_deref(),
            &Desktop::read(),
        );
        if self.current.as_ref() == Some(&next) {
            return;
        }
        let client = matches!(next, Policy::Client(_));
        let was_client = matches!(self.current, Some(Policy::Client(_)));
        if self.current.is_none() || client != was_client {
            if was_client {
                self.header.remove(&self.toolbar);
            } else if self.current.is_some() {
                self.content.remove(&self.toolbar);
            }
            if client {
                self.header.pack_start(&self.toolbar);
                window.set_titlebar(Some(&self.header));
            } else {
                window.set_titlebar(None::<&gtk::Widget>);
                self.content.prepend(&self.toolbar);
            }
        }
        match &next {
            Policy::Client(layout) => {
                self.header.set_decoration_layout(Some(layout));
                self.header.set_show_title_buttons(true);
                window.set_decorated(true);
            }
            Policy::Server | Policy::Toolbar => {
                self.header.set_show_title_buttons(false);
                window.set_decorated(next == Policy::Server);
            }
        }
        eprintln!("arut-linux: decorations {next:?}");
        self.current = Some(next);
    }
}

pub struct Decorations {
    settings: gtk::Settings,
    signal: Option<glib::SignalHandlerId>,
}
impl Decorations {
    pub fn install(
        window: &gtk::ApplicationWindow,
        header: &gtk::HeaderBar,
        toolbar: &gtk::Box,
    ) -> Self {
        let content = header.parent().unwrap().downcast::<gtk::Box>().unwrap();
        content.remove(header);
        // HeaderBar internally wraps its children; remove explicitly before
        // promoting the content to a genuine toolbar without WindowHandle gestures.
        header.remove(toolbar);
        toolbar.add_css_class("toolbar");
        toolbar.set_accessible_role(gtk::AccessibleRole::Toolbar);
        let state = Rc::new(RefCell::new(State {
            window: window.downgrade(),
            header: header.clone(),
            toolbar: toolbar.clone(),
            content,
            current: None,
        }));
        state.borrow_mut().refresh();
        let settings = gtk::Settings::for_display(&WidgetExt::display(window));
        let signal = settings.connect_gtk_decoration_layout_notify(move |_| {
            if let Ok(mut state) = state.try_borrow_mut() {
                state.refresh();
            }
        });
        Self {
            settings,
            signal: Some(signal),
        }
    }
}
impl Drop for Decorations {
    fn drop(&mut self) {
        if let Some(signal) = self.signal.take() {
            self.settings.disconnect(signal);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Desktop, Policy, resolve};

    fn desktop(name: &str) -> Desktop {
        Desktop {
            names: name.into(),
            ..Desktop::default()
        }
    }

    #[test]
    fn server_frames_win_over_every_layout() {
        assert_eq!(
            resolve(true, Some("close:"), &desktop("gnome")),
            Policy::Server
        );
        assert_eq!(
            resolve(true, Some(":close"), &desktop("kde")),
            Policy::Server
        );
    }

    #[test]
    fn tilers_never_get_invented_window_buttons() {
        for name in ["hyprland", "sway", "river", "niri"] {
            assert_eq!(
                resolve(false, Some(":close"), &desktop(name)),
                Policy::Toolbar
            );
        }
        assert_eq!(
            resolve(
                false,
                None,
                &Desktop {
                    hyprland: true,
                    ..Desktop::default()
                }
            ),
            Policy::Toolbar
        );
    }

    #[test]
    fn an_explicit_layout_decides_before_the_desktop_name() {
        assert_eq!(
            resolve(false, Some("close:"), &desktop("gnome")),
            Policy::Client("close:".into())
        );
        assert_eq!(
            resolve(false, Some("close:minimize,maximize"), &desktop("deepin")),
            Policy::Client("close:minimize,maximize".into())
        );
        assert_eq!(
            resolve(false, Some(":"), &desktop("gnome")),
            Policy::Toolbar
        );
    }

    #[test]
    fn only_gnome_falls_back_to_gnome_buttons() {
        assert_eq!(
            resolve(false, None, &desktop("gnome")),
            Policy::Client(":close".into())
        );
        assert_eq!(resolve(false, None, &desktop("kde")), Policy::Toolbar);
        assert_eq!(resolve(false, None, &desktop("xfce")), Policy::Toolbar);
    }
}
