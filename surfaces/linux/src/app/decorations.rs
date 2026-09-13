//! Desktop decoration policy. GTK's native window negotiation wins; explicit
//! portal layouts follow, with environment signals only as a fallback.
use crate::app::observe::Tasks;
use ashpd::desktop::settings::Settings as Portal;
use futures_util::StreamExt;
use gtk::{gdk, glib, prelude::*};
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

fn resolve(
    server: bool,
    button_layout: Option<&str>,
    portal_gtk: Option<&str>,
    gtk_layout: Option<&str>,
    desktop: &Desktop,
) -> Policy {
    if server {
        return Policy::Server;
    }
    let layout = button_layout.or(portal_gtk).or(gtk_layout);
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

struct State {
    window: glib::WeakRef<gtk::ApplicationWindow>,
    header: gtk::HeaderBar,
    toolbar: gtk::Box,
    content: gtk::Box,
    button_layout: Option<String>,
    portal_gtk: Option<String>,
    current: Option<Policy>,
}
impl State {
    fn refresh(&mut self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let display = WidgetExt::display(&window);
        // GDK's prefers-SSD function is private. An unmapped ordinary GtkWindow
        // exposes GTK's negotiated choice through its documented CSD CSS classes.
        // No custom titlebar is installed on this probe, so it cannot force CSD.
        let probe = gtk::Window::builder().display(&display).build();
        WidgetExt::realize(&probe);
        let server = !probe.has_css_class("csd") && !probe.has_css_class("solid-csd");
        probe.destroy();
        // Read the typed property, not DisplayExtManual::get_setting: gtk-rs
        // 0.11.4 passes an uninitialized GValue to GDK, whose Wayland string
        // setting reader requires G_TYPE_STRING (fatal g_value_set_string).
        // GtkSettings includes the backend and settings.ini overrides. Exclude its
        // compiled default: only the desktop fallback may invent button positions.
        let settings = gtk::Settings::for_display(&display);
        let gtk_layout = settings.gtk_decoration_layout().filter(|layout| {
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
        });
        let next = resolve(
            server,
            self.button_layout.as_deref(),
            self.portal_gtk.as_deref(),
            gtk_layout.as_deref(),
            &Desktop::read(),
        );
        if self.current.as_ref() == Some(&next) {
            return;
        }
        let frame_changed = self.current.as_ref().is_none_or(|old| {
            matches!(old, Policy::Client(_)) != matches!(next, Policy::Client(_))
        });
        let visible = window.is_visible();
        if frame_changed && window.is_realized() {
            window.set_visible(false);
            WidgetExt::unrealize(&window);
        }
        if frame_changed {
            if matches!(self.current, Some(Policy::Client(_))) {
                self.header.remove(&self.toolbar);
            } else if self.toolbar.parent().is_some() {
                self.content.remove(&self.toolbar);
            }
            window.set_titlebar(None::<&gtk::Widget>);
        }
        match &next {
            Policy::Client(layout) => {
                self.header.set_decoration_layout(Some(layout));
                self.header.set_show_title_buttons(true);
                if frame_changed {
                    self.header.pack_start(&self.toolbar);
                    window.set_titlebar(Some(&self.header));
                }
                window.set_decorated(true);
            }
            Policy::Server | Policy::Toolbar => {
                self.header.set_show_title_buttons(false);
                if frame_changed {
                    self.content.prepend(&self.toolbar);
                }
                window.set_decorated(next == Policy::Server);
            }
        }
        eprintln!("arut-linux: decorations {next:?} (GTK server preference: {server})");
        self.current = Some(next);
        if frame_changed && visible {
            window.set_visible(true);
        }
    }
}

pub struct Decorations {
    display: gdk::Display,
    settings: gtk::Settings,
    display_signal: Option<glib::SignalHandlerId>,
    settings_signal: Option<glib::SignalHandlerId>,
    _tasks: Tasks,
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
        content.prepend(toolbar);
        toolbar.add_css_class("toolbar");
        toolbar.set_accessible_role(gtk::AccessibleRole::Toolbar);
        let state = Rc::new(RefCell::new(State {
            window: window.downgrade(),
            header: header.clone(),
            toolbar: toolbar.clone(),
            content,
            button_layout: None,
            portal_gtk: None,
            current: None,
        }));
        state.borrow_mut().refresh();
        let display = WidgetExt::display(window);
        let settings = gtk::Settings::for_display(&display);
        let display_signal = display.connect_setting_changed({
            let state = state.clone();
            move |_, _| {
                if let Ok(mut state) = state.try_borrow_mut() {
                    state.refresh();
                }
            }
        });
        let settings_signal = settings.connect_gtk_decoration_layout_notify({
            let state = state.clone();
            move |_| {
                if let Ok(mut state) = state.try_borrow_mut() {
                    state.refresh();
                }
            }
        });
        window.connect_realize({
            let state = Rc::downgrade(&state);
            move |window| {
                if let Some(toplevel) = window
                    .surface()
                    .and_then(|s| s.downcast::<gdk::Toplevel>().ok())
                {
                    let state = state.clone();
                    toplevel.connect_state_notify(move |_| {
                        if let Some(state) = state.upgrade()
                            && let Ok(mut state) = state.try_borrow_mut()
                        {
                            state.refresh();
                        }
                    });
                }
            }
        });
        let mut tasks = Tasks::default();
        tasks.spawn(async move {
            let Ok(portal) = Portal::new().await else {
                return;
            };
            let changes = portal.receive_setting_changed().await;
            let buttons = portal
                .read::<String>("org.gnome.desktop.wm.preferences", "button-layout")
                .await
                .ok();
            let layout = portal
                .read::<String>("org.gnome.desktop.interface", "gtk-decoration-layout")
                .await
                .ok();
            {
                let mut state = state.borrow_mut();
                state.button_layout = buttons;
                state.portal_gtk = layout;
                state.refresh();
            }
            let Ok(changes) = changes else {
                return;
            };
            futures_util::pin_mut!(changes);
            while let Some(change) = changes.next().await {
                let is_buttons = change.namespace() == "org.gnome.desktop.wm.preferences"
                    && change.key() == "button-layout";
                let is_layout = change.key() == "gtk-decoration-layout";
                if !is_buttons && !is_layout {
                    continue;
                }
                let value = change
                    .value()
                    .try_clone()
                    .ok()
                    .and_then(|v| String::try_from(v).ok());
                let mut state = state.borrow_mut();
                if is_buttons {
                    state.button_layout = value;
                } else {
                    state.portal_gtk = value;
                }
                state.refresh();
            }
        });
        Self {
            display,
            settings,
            display_signal: Some(display_signal),
            settings_signal: Some(settings_signal),
            _tasks: tasks,
        }
    }
}
impl Drop for Decorations {
    fn drop(&mut self) {
        self.display.disconnect(self.display_signal.take().unwrap());
        self.settings
            .disconnect(self.settings_signal.take().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::{Desktop, Policy, resolve};
    #[test]
    fn compositor_then_layout_then_desktop() {
        let desktop = |name: &str| Desktop {
            names: name.into(),
            ..Desktop::default()
        };
        assert_eq!(
            resolve(true, Some("close:"), None, None, &desktop("gnome")),
            Policy::Server
        );
        assert_eq!(
            resolve(
                false,
                Some("close:"),
                Some(":close"),
                None,
                &desktop("gnome")
            ),
            Policy::Client("close:".into())
        );
        assert_eq!(
            resolve(false, Some(":"), None, Some(":close"), &desktop("gnome")),
            Policy::Toolbar
        );
        for name in ["hyprland", "sway", "river", "niri"] {
            assert_eq!(
                resolve(false, Some(":close"), None, None, &desktop(name)),
                Policy::Toolbar
            );
        }
        assert_eq!(
            resolve(false, None, None, None, &desktop("kde")),
            Policy::Toolbar
        );
        assert_eq!(
            resolve(true, Some(":close"), None, None, &desktop("kde")),
            Policy::Server
        );
        assert_eq!(
            resolve(false, None, None, None, &desktop("gnome")),
            Policy::Client(":close".into())
        );
        assert_eq!(
            resolve(
                false,
                None,
                None,
                None,
                &Desktop {
                    hyprland: true,
                    ..Desktop::default()
                }
            ),
            Policy::Toolbar
        );
        assert_eq!(
            resolve(
                false,
                None,
                Some("close:minimize,maximize"),
                None,
                &desktop("deepin")
            ),
            Policy::Client("close:minimize,maximize".into())
        );
    }
}
