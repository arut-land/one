//! One CSS provider for Arut's own classes. GTK owns the theme: `style.css`
//! references the theme's named colors and its own media queries, so a theme or
//! contrast change re-resolves without anything being rebuilt here. The portal
//! supplies the two settings GTK 4.20 does not: accent and reduced motion.
use crate::app::observe::Tasks;
use ashpd::desktop::{
    Color,
    settings::{
        ACCENT_COLOR_SCHEME_KEY, APPEARANCE_NAMESPACE, REDUCED_MOTION_KEY, ReducedMotion,
        Settings as PortalSettings,
    },
};
use futures_lite::StreamExt;
use gtk::{gdk, glib, prelude::*};
use std::{cell::Cell, rc::Rc};

pub struct Theme {
    provider: gtk::CssProvider,
    display: gdk::Display,
    settings: gtk::Settings,
    signals: Vec<glib::SignalHandlerId>,
    _tasks: Tasks,
}

#[derive(Clone, Copy, Default)]
struct Appearance {
    accent: Option<gdk::RGBA>,
    reduced_motion: bool,
}

impl Theme {
    pub fn install(widget: &impl IsA<gtk::Widget>) -> Self {
        let display = widget.as_ref().display();
        let settings = gtk::Settings::default().expect("GTK settings");
        let provider = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let appearance = Rc::new(Cell::new(Appearance::default()));
        refresh(&settings, &provider, appearance.get());
        let mut signals = Vec::new();
        for property in [
            "gtk-interface-color-scheme",
            "gtk-interface-contrast",
            "gtk-enable-animations",
        ] {
            let (provider, appearance) = (provider.clone(), appearance.clone());
            signals.push(
                settings.connect_notify_local(Some(property), move |settings, _| {
                    refresh(settings, &provider, appearance.get());
                }),
            );
        }
        let mut tasks = Tasks::default();
        tasks.spawn(portal(settings.clone(), provider.clone(), appearance));
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

/// Reads accent and reduced motion from the appearance portal and follows their
/// changes. GTK 4.20 exposes neither; 4.22 adds reduced motion, 4.24 the accent.
async fn portal(
    settings: gtk::Settings,
    provider: gtk::CssProvider,
    appearance: Rc<Cell<Appearance>>,
) {
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
    let reduced_motion =
        portal.reduced_motion().await.unwrap_or_default() == ReducedMotion::ReducedMotion;
    appearance.set(Appearance {
        accent,
        reduced_motion,
    });
    refresh(&settings, &provider, appearance.get());
    let Ok(changes) = changes else {
        eprintln!("Appearance portal change stream unavailable");
        return;
    };
    let mut changes = std::pin::pin!(changes);
    while let Some(change) = changes.next().await {
        if change.namespace() != APPEARANCE_NAMESPACE {
            continue;
        }
        let mut next = appearance.get();
        match change.key() {
            ACCENT_COLOR_SCHEME_KEY => {
                next.accent = change
                    .value()
                    .try_clone()
                    .ok()
                    .and_then(|value| <(f64, f64, f64)>::try_from(value).ok())
                    .and_then(|rgb| valid_accent(Color::from(rgb)));
            }
            REDUCED_MOTION_KEY => {
                let Some(reduced) = change
                    .value()
                    .try_clone()
                    .ok()
                    .and_then(|value| ReducedMotion::try_from(value).ok())
                else {
                    continue;
                };
                next.reduced_motion = reduced == ReducedMotion::ReducedMotion;
            }
            _ => continue,
        }
        appearance.set(next);
        refresh(&settings, &provider, next);
    }
}

fn valid_accent(color: Color) -> Option<gdk::RGBA> {
    [color.red(), color.green(), color.blue()]
        .into_iter()
        .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
        .then(|| color.into())
}

fn refresh(settings: &gtk::Settings, provider: &gtk::CssProvider, appearance: Appearance) {
    // GTK owns portal discovery and desktop defaults, including unsupported values.
    provider.set_prefers_color_scheme(settings.gtk_interface_color_scheme());
    provider.set_prefers_contrast(settings.gtk_interface_contrast());
    let accent = appearance.accent.map_or_else(
        || "@theme_selected_bg_color".to_owned(),
        |accent| accent.to_string(),
    );
    let mut css = format!("@define-color arut_accent {accent};\n");
    css.push_str(include_str!("style.css"));
    if appearance.reduced_motion || !settings.is_gtk_enable_animations() {
        css.push_str(".arut-window, .arut-window * { transition: none; animation: none; }\n");
    }
    provider.load_from_string(&css);
}
