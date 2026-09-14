//! One CSS provider for Arut's own classes. GTK owns the theme: `style.css`
//! references the theme's named colors and its own media queries, so a theme,
//! contrast or motion change re-resolves without anything being rebuilt here.
//! GTK 4.22 reports color scheme, contrast and reduced motion itself. The
//! portal supplies only the accent, which GTK does not expose before 4.24.
use crate::glib_observe::Tasks;
use ashpd::desktop::{Color, settings::Settings as PortalSettings};
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
        let accent: Rc<Cell<Option<gdk::RGBA>>> = Rc::new(Cell::new(None));
        refresh(&settings, &provider, accent.get());
        let mut signals = Vec::new();
        for property in [
            "gtk-interface-color-scheme",
            "gtk-interface-contrast",
            "gtk-interface-reduced-motion",
            "gtk-enable-animations",
        ] {
            let (provider, accent) = (provider.clone(), accent.clone());
            signals.push(
                settings.connect_notify_local(Some(property), move |settings, _| {
                    refresh(settings, &provider, accent.get());
                }),
            );
        }
        let mut tasks = Tasks::default();
        tasks.spawn(portal(settings.clone(), provider.clone(), accent));
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

/// Reads the accent color from the appearance portal and follows its changes.
/// It is the one appearance setting GTK does not report; 4.24 adds it, and this
/// task goes away when the crate can require that.
async fn portal(
    settings: gtk::Settings,
    provider: gtk::CssProvider,
    accent: Rc<Cell<Option<gdk::RGBA>>>,
) {
    let portal = match PortalSettings::new().await {
        Ok(portal) => portal,
        Err(error) => {
            eprintln!("Appearance portal unavailable; using the GTK theme accent: {error}");
            return;
        }
    };
    // Subscribe before reading so an accent change cannot fall between them.
    let changes = portal.receive_accent_color_changed().await;
    accent.set(portal.accent_color().await.ok().and_then(valid_accent));
    refresh(&settings, &provider, accent.get());
    let Ok(changes) = changes else {
        eprintln!("Appearance portal accent stream unavailable");
        return;
    };
    let mut changes = std::pin::pin!(changes);
    while let Some(color) = changes.next().await {
        accent.set(valid_accent(color));
        refresh(&settings, &provider, accent.get());
    }
}

fn valid_accent(color: Color) -> Option<gdk::RGBA> {
    [color.red(), color.green(), color.blue()]
        .into_iter()
        .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
        .then(|| color.into())
}

fn refresh(settings: &gtk::Settings, provider: &gtk::CssProvider, accent: Option<gdk::RGBA>) {
    // GTK owns portal discovery and desktop defaults, including unsupported values.
    provider.set_prefers_color_scheme(settings.gtk_interface_color_scheme());
    provider.set_prefers_contrast(settings.gtk_interface_contrast());
    // A desktop that turns animations off entirely is asking for reduced motion
    // even where it leaves the interface setting at its default.
    provider.set_prefers_reduced_motion(if settings.is_gtk_enable_animations() {
        settings.gtk_interface_reduced_motion()
    } else {
        gtk::ReducedMotion::Reduce
    });
    let accent = accent.map_or_else(
        || "@theme_selected_bg_color".to_owned(),
        |accent| accent.to_string(),
    );
    let mut css = format!("@define-color arut_accent {accent};\n");
    css.push_str(include_str!("style.css"));
    provider.load_from_string(&css);
}
