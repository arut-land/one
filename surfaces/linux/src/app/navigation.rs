//! Navigation state across launches. Only navigation lives here: the node owns
//! durable conversations and drafts.
use gtk::glib::KeyFile;
use std::path::PathBuf;

const GROUP: &str = "navigation";

#[derive(Default)]
pub struct Navigation {
    pub selected: Option<String>,
    pub collapsed: bool,
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
        Self {
            selected: file
                .string(GROUP, "selected")
                .ok()
                .map(|id| id.to_string())
                .filter(|id| !id.is_empty()),
            collapsed: file.boolean(GROUP, "collapsed").unwrap_or(false),
        }
    }

    pub fn save(&self) {
        let Some(path) = path() else {
            return;
        };
        let file = KeyFile::new();
        file.set_string(GROUP, "selected", self.selected.as_deref().unwrap_or(""));
        file.set_boolean(GROUP, "collapsed", self.collapsed);
        let saved = std::fs::create_dir_all(path.parent().expect("state directory"))
            .map_err(|error| error.to_string())
            .and_then(|()| file.save_to_file(path).map_err(|error| error.to_string()));
        if let Err(error) = saved {
            eprintln!("Could not save navigation: {error}");
        }
    }
}
