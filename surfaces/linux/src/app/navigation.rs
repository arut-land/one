//! Navigation state across launches. Only navigation lives here: the node owns
//! durable conversations and drafts.
use gtk::{glib::KeyFile, prelude::*};
use std::path::PathBuf;

const GROUP: &str = "navigation";
const WINDOW: &str = "window";
const DEFAULT_SIZE: (i32, i32) = (900, 600);
const DEFAULT_SIDEBAR: i32 = 260;
/// Below this the sidebar and the chat column cannot both be read; a saved
/// value smaller than it came from a state file, not from a drag.
const MINIMUM: (i32, i32) = (360, 320);

#[derive(Debug, PartialEq, Eq)]
pub struct Navigation {
    pub selected: Option<String>,
    pub collapsed: bool,
    /// Width of the conversation sidebar pane.
    pub sidebar: i32,
    /// Last window size. Wayland owns placement, so only the size is ours to
    /// restore; it is clamped to the monitors present at the next launch.
    pub size: (i32, i32),
    pub maximized: bool,
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            selected: None,
            collapsed: false,
            sidebar: DEFAULT_SIDEBAR,
            size: DEFAULT_SIZE,
            maximized: false,
        }
    }
}

fn path() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .map(|base| base.join("arut/linux-ui"))
}

impl Navigation {
    pub fn load() -> Self {
        let file = KeyFile::new();
        let Some(path) = path() else {
            return Self::default();
        };
        if file
            .load_from_file(path, gtk::glib::KeyFileFlags::NONE)
            .is_err()
        {
            return Self::default();
        }
        let default = Self::default();
        Self {
            selected: file
                .string(GROUP, "selected")
                .ok()
                .map(|id| id.to_string())
                .filter(|id| !id.is_empty()),
            collapsed: file
                .boolean(GROUP, "collapsed")
                .unwrap_or(default.collapsed),
            sidebar: file
                .integer(WINDOW, "sidebar")
                .unwrap_or(default.sidebar)
                .max(0),
            size: (
                file.integer(WINDOW, "width").unwrap_or(default.size.0),
                file.integer(WINDOW, "height").unwrap_or(default.size.1),
            ),
            maximized: file
                .boolean(WINDOW, "maximized")
                .unwrap_or(default.maximized),
        }
    }

    /// The saved size, held inside the largest monitor this session can see so
    /// a window saved on an unplugged display still opens fully on screen.
    pub fn size_on(&self, display: Option<&gtk::gdk::Display>) -> (i32, i32) {
        let available = display
            .map(|display| {
                let monitors = display.monitors();
                (0..monitors.n_items())
                    .filter_map(|position| monitors.item(position))
                    .filter_map(|object| object.downcast::<gtk::gdk::Monitor>().ok())
                    .map(|monitor| {
                        let area = monitor.geometry();
                        (area.width(), area.height())
                    })
                    .fold((0, 0), |largest, area| {
                        (largest.0.max(area.0), largest.1.max(area.1))
                    })
            })
            .filter(|available| available.0 > 0 && available.1 > 0);
        let Some((width, height)) = available else {
            return self.size;
        };
        (
            self.size.0.clamp(MINIMUM.0.min(width), width),
            self.size.1.clamp(MINIMUM.1.min(height), height),
        )
    }

    pub fn save(&self) {
        let Some(path) = path() else {
            return;
        };
        let file = KeyFile::new();
        file.set_string(GROUP, "selected", self.selected.as_deref().unwrap_or(""));
        file.set_boolean(GROUP, "collapsed", self.collapsed);
        file.set_integer(WINDOW, "sidebar", self.sidebar);
        file.set_integer(WINDOW, "width", self.size.0);
        file.set_integer(WINDOW, "height", self.size.1);
        file.set_boolean(WINDOW, "maximized", self.maximized);
        let saved = std::fs::create_dir_all(path.parent().expect("state directory"))
            .map_err(|error| error.to_string())
            .and_then(|()| file.save_to_file(path).map_err(|error| error.to_string()));
        if let Err(error) = saved {
            eprintln!("Could not save navigation: {error}");
        }
    }
}
