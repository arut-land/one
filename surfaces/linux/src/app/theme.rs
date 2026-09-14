use crate::app::observe::Tasks;
use ashpd::desktop::{
    Color,
    settings::{
        ACCENT_COLOR_SCHEME_KEY, APPEARANCE_NAMESPACE, REDUCED_MOTION_KEY, ReducedMotion,
        Settings as PortalSettings,
    },
};
use futures_util::StreamExt;
use gtk::{gdk, glib, prelude::*};
use std::{cell::RefCell, rc::Rc};

/// The provider augments Arut classes only. GTK remains responsible for the theme.
pub struct Theme {
    provider: gtk::CssProvider,
    display: gdk::Display,
    settings: gtk::Settings,
    signals: Vec<glib::SignalHandlerId>,
    appearance: Rc<RefCell<Appearance>>,
    _tasks: Tasks,
}

struct Appearance {
    accent: Option<gdk::RGBA>,
    reduced_motion: bool,
    sidebar: glib::WeakRef<gtk::Revealer>,
}

impl Theme {
    pub fn bind_sidebar(&self, sidebar: &gtk::Revealer) {
        self.appearance.borrow().sidebar.set(Some(sidebar));
    }

    pub fn sidebar_motion(&self, sidebar: &gtk::Revealer, animate: bool) {
        let enabled = animate
            && self.settings.is_gtk_enable_animations()
            && !self.appearance.borrow().reduced_motion;
        if enabled {
            sidebar.set_transition_duration(200);
        } else {
            settle_sidebar(sidebar);
        }
    }

    pub fn install(widget: &impl IsA<gtk::Widget>) -> Self {
        let widget = widget.as_ref();
        let display = widget.display();
        let settings = gtk::Settings::default().expect("GTK settings");
        let provider = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let appearance = Rc::new(RefCell::new(Appearance {
            accent: None,
            reduced_motion: false,
            sidebar: glib::WeakRef::new(),
        }));
        refresh(widget, &settings, &provider, &appearance.borrow());
        let mut signals = Vec::new();
        for property in [
            "gtk-theme-name",
            "gtk-interface-color-scheme",
            "gtk-interface-contrast",
            "gtk-enable-animations",
        ] {
            let weak = widget.downgrade();
            let provider = provider.clone();
            let appearance = appearance.clone();
            signals.push(
                settings.connect_notify_local(Some(property), move |settings, _| {
                    if let Some(widget) = weak.upgrade() {
                        refresh(&widget, settings, &provider, &appearance.borrow());
                    }
                }),
            );
        }
        let mut tasks = Tasks::default();
        let weak = widget.downgrade();
        let portal_provider = provider.clone();
        let portal_settings = settings.clone();
        let portal_appearance = appearance.clone();
        tasks.spawn(async move {
            let appearance = portal_appearance;
            let portal = match PortalSettings::new().await {
                Ok(portal) => portal,
                Err(error) => {
                    eprintln!("Appearance portal unavailable; using GTK theme: {error}");
                    return;
                }
            };
            // Subscribe before reading so an appearance change cannot fall between them.
            let changes = portal.receive_setting_changed().await;
            let accent = portal
                .read::<(f64, f64, f64)>(APPEARANCE_NAMESPACE, ACCENT_COLOR_SCHEME_KEY)
                .await
                .ok()
                .and_then(|rgb| valid_accent(Color::from(rgb)));
            appearance.borrow_mut().accent = accent;
            let reduced = portal.reduced_motion().await.unwrap_or_default();
            appearance.borrow_mut().reduced_motion = reduced == ReducedMotion::ReducedMotion;
            if let Some(widget) = weak.upgrade() {
                refresh(
                    &widget,
                    &portal_settings,
                    &portal_provider,
                    &appearance.borrow(),
                );
            }
            let Ok(changes) = changes else {
                eprintln!("Appearance portal change stream unavailable");
                return;
            };
            futures_util::pin_mut!(changes);
            while let Some(change) = changes.next().await {
                if change.namespace() != APPEARANCE_NAMESPACE {
                    continue;
                }
                match change.key() {
                    ACCENT_COLOR_SCHEME_KEY => {
                        appearance.borrow_mut().accent = change
                            .value()
                            .try_clone()
                            .ok()
                            .and_then(|value| <(f64, f64, f64)>::try_from(value).ok())
                            .and_then(|rgb| valid_accent(Color::from(rgb)));
                    }
                    REDUCED_MOTION_KEY => {
                        if let Some(reduced) = change
                            .value()
                            .try_clone()
                            .ok()
                            .and_then(|value| ReducedMotion::try_from(value).ok())
                        {
                            appearance.borrow_mut().reduced_motion =
                                reduced == ReducedMotion::ReducedMotion;
                        }
                    }
                    _ => continue,
                }
                let Some(widget) = weak.upgrade() else {
                    break;
                };
                refresh(
                    &widget,
                    &portal_settings,
                    &portal_provider,
                    &appearance.borrow(),
                );
            }
        });
        Self {
            provider,
            display,
            settings,
            signals,
            appearance,
            _tasks: tasks,
        }
    }
}

impl Drop for Theme {
    fn drop(&mut self) {
        for signal in self.signals.drain(..) {
            self.settings.disconnect(signal);
        }
        gtk::style_context_remove_provider_for_display(&self.display, &self.provider);
    }
}

fn valid_accent(color: Color) -> Option<gdk::RGBA> {
    [color.red(), color.green(), color.blue()]
        .into_iter()
        .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
        .then(|| color.into())
}

fn refresh(
    widget: &gtk::Widget,
    settings: &gtk::Settings,
    provider: &gtk::CssProvider,
    appearance: &Appearance,
) {
    // GTK owns portal discovery and desktop defaults, including unsupported values.
    provider.set_prefers_color_scheme(settings.gtk_interface_color_scheme());
    let contrast = settings.gtk_interface_contrast();
    provider.set_prefers_contrast(contrast);
    let high_contrast = contrast == gtk::InterfaceContrast::More;
    let reduced_motion = appearance.reduced_motion || !settings.is_gtk_enable_animations();
    if reduced_motion && let Some(sidebar) = appearance.sidebar.upgrade() {
        settle_sidebar(&sidebar);
    }
    let mut css = String::new();
    let mut defined = std::collections::HashSet::new();
    // Palette lookup and theme notification pattern adapted from WaterUI (MIT):
    // https://github.com/water-rs/waterui/blob/main/backends/gtk/src/theme.rs
    // Copyright WaterUI contributors. No reactive framework is used here.
    for (alias, name) in [
        ("background", "theme_bg_color"),
        ("surface", "theme_base_color"),
        ("surface_variant", "theme_unfocused_bg_color"),
        ("foreground", "theme_fg_color"),
        ("muted_foreground", "theme_unfocused_fg_color"),
        ("selection", "theme_selected_bg_color"),
        ("selection_foreground", "theme_selected_fg_color"),
        ("border", "borders"),
    ] {
        if let Some(color) = lookup(widget, name) {
            defined.insert(alias);
            css.push_str(&format!("@define-color arut_{alias} {color};\n"));
        }
    }
    let accent = appearance
        .accent
        .or_else(|| lookup(widget, "theme_selected_bg_color"));
    if let Some(accent) = accent {
        defined.insert("accent");
        css.push_str(&format!("@define-color arut_accent {accent};\n"));
    }
    css.push_str(include_str!("style.css"));
    for (required, rule) in [
        (
            &["background", "foreground"][..],
            ".arut-window { background-color: @arut_background; color: @arut_foreground; }",
        ),
        (
            &["surface_variant"][..],
            ".arut-sidebar { background-color: @arut_surface_variant; }",
        ),
        (
            &["surface", "foreground"][..],
            ".arut-chat-surface { background-color: @arut_surface; color: @arut_foreground; }",
        ),
        (
            &["selection", "selection_foreground"][..],
            ".arut-conversations row:selected { background-color: @arut_selection; color: @arut_selection_foreground; } .arut-conversations row:selected .dim-label { opacity: 0.85; }",
        ),
        (
            &["selection", "selection_foreground"][..],
            ".arut-message.arut-outgoing { background-color: @arut_selection; color: @arut_selection_foreground; }",
        ),
        (
            &["surface", "foreground"][..],
            ".arut-composer { background-color: @arut_surface; color: @arut_foreground; }",
        ),
        (
            &["border"][..],
            ".arut-message.arut-outgoing, .arut-error { border: 1px solid @arut_border; } .arut-sidebar { border-right: 1px solid @arut_border; } .arut-composer { box-shadow: 0 2px 6px alpha(@arut_border, 0.35); }",
        ),
        (
            &["muted_foreground"][..],
            ".arut-availability { color: @arut_muted_foreground; font-size: smaller; padding: 4px; }",
        ),
        (
            &["accent"][..],
            ".arut-composer:focus-within { outline: 2px solid @arut_accent; outline-offset: -2px; }",
        ),
    ] {
        if required.iter().all(|name| defined.contains(name)) {
            css.push_str(rule);
            css.push('\n');
        }
    }
    if high_contrast {
        css.push_str(".arut-message, .arut-message.arut-outgoing, .arut-composer { border: 2px solid currentColor; }\n");
    }
    if reduced_motion {
        css.push_str(".arut-window, .arut-window * { transition: none; animation: none; }\n");
    }
    provider.load_from_string(&css);
}

#[expect(
    deprecated,
    reason = "GTK deprecated StyleContext without a replacement for named-color lookup"
)]
fn lookup(widget: &gtk::Widget, name: &str) -> Option<gdk::RGBA> {
    widget.style_context().lookup_color(name)
}

fn settle_sidebar(sidebar: &gtk::Revealer) {
    sidebar.set_transition_duration(0);
    if sidebar.is_child_revealed() != sidebar.reveals_child() && sidebar.is_mapped() {
        // Changing duration does not stop GTK's active progress tracker.
        // Unmapping settles it at the target before the next allocation.
        sidebar.set_visible(false);
        sidebar.set_visible(true);
    }
}
