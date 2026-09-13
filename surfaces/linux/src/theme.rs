use crate::observe::Tasks;
use ashpd::desktop::{
    Color,
    settings::{
        ACCENT_COLOR_SCHEME_KEY, APPEARANCE_NAMESPACE, REDUCED_MOTION_KEY, ReducedMotion,
        Settings as PortalSettings,
    },
};
use futures_util::StreamExt;
use gtk::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

thread_local! {
    static REDUCED: Cell<bool> = const { Cell::new(true) };
    static REVEAL_DURATION: u32 = gtk::Revealer::new().transition_duration();
}

/// The provider augments Arut classes only. GTK remains responsible for the theme.
pub struct Theme {
    provider: gtk::CssProvider,
    display: gdk::Display,
    settings: gtk::Settings,
    signals: Vec<glib::SignalHandlerId>,
    _tasks: Tasks,
}

struct Appearance {
    accent: Option<gdk::RGBA>,
    reduced_motion: bool,
}

impl Theme {
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
            reduced_motion: true,
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
        tasks.spawn(async move {
            let portal = match PortalSettings::new().await {
                Ok(portal) => portal,
                Err(error) => {
                    eprintln!("Appearance portal unavailable; using GTK theme: {error}");
                    return;
                }
            };
            // Subscribe before reading so an appearance change cannot fall between them.
            let changes = portal.receive_setting_changed().await;
            if let Ok(layout) = portal
                .read::<String>("org.gnome.desktop.wm.preferences", "button-layout")
                .await
            {
                portal_settings.set_gtk_decoration_layout(Some(&layout));
            }
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
                if change.namespace() == "org.gnome.desktop.wm.preferences"
                    && change.key() == "button-layout"
                {
                    if let Some(layout) = change
                        .value()
                        .try_clone()
                        .ok()
                        .and_then(|v| String::try_from(v).ok())
                    {
                        portal_settings.set_gtk_decoration_layout(Some(&layout));
                    }
                    continue;
                }
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
    REDUCED.set(appearance.reduced_motion);
    let reduced_motion = appearance.reduced_motion || !settings.is_gtk_enable_animations();
    update_reveals(widget, reduced_motion);
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
    // Every style color is a palette name; none is a hard-coded RGB value.
    // Themes may omit a name. In that case retain the native widget styling.
    css.push_str(".arut-message { padding: 10px; border-radius: 12px; margin: 4px 12px; } .arut-composer { border-radius: 18px; } .arut-composer textview, .arut-composer textview text { background-color: transparent; }\n");
    for (required, rule) in [
        (
            &["background", "foreground"][..],
            ".arut-window { background-color: @arut_background; color: @arut_foreground; }",
        ),
        (
            &["surface_variant", "muted_foreground"][..],
            ".arut-conversations { background-color: @arut_surface_variant; color: @arut_muted_foreground; }",
        ),
        (
            &["selection", "selection_foreground"][..],
            ".arut-outgoing { background-color: @arut_selection; color: @arut_selection_foreground; }",
        ),
        (
            &["surface", "foreground"][..],
            ".arut-message { background-color: @arut_surface; color: @arut_foreground; }",
        ),
        (
            &["border"][..],
            ".arut-message { border: 1px solid @arut_border; }",
        ),
        (
            &["accent"][..],
            ".arut-availability { border-bottom: 2px solid @arut_accent; padding: 4px; }",
        ),
    ] {
        if required.iter().all(|name| defined.contains(name)) {
            css.push_str(rule);
            css.push('\n');
        }
    }
    if high_contrast {
        css.push_str(".arut-message { border-width: 2px; }\n");
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

pub fn reveal_motion(revealer: &gtk::Revealer) {
    let settings = gtk::Settings::default().expect("GTK settings");
    let duration = REVEAL_DURATION.with(|duration| *duration);
    let weak = revealer.downgrade();
    let update = move |settings: &gtk::Settings| {
        if let Some(revealer) = weak.upgrade() {
            revealer.set_transition_duration(
                if settings.is_gtk_enable_animations() && !REDUCED.get() {
                    duration
                } else {
                    0
                },
            );
        }
    };
    update(&settings);
    // Object-bound signal disconnects when the revealer is destroyed.
    settings.connect_closure(
        "notify::gtk-enable-animations",
        false,
        glib::closure_local!(
            #[weak]
            revealer,
            move |settings: gtk::Settings, _: glib::ParamSpec| {
                revealer.set_transition_duration(
                    if settings.is_gtk_enable_animations() && !REDUCED.get() {
                        duration
                    } else {
                        0
                    },
                );
            }
        ),
    );
}

fn update_reveals(widget: &gtk::Widget, reduced: bool) {
    if let Some(revealer) = widget.downcast_ref::<gtk::Revealer>() {
        revealer.set_transition_duration(if reduced {
            0
        } else {
            REVEAL_DURATION.with(|duration| *duration)
        });
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        update_reveals(&current, reduced);
        child = current.next_sibling();
    }
}
