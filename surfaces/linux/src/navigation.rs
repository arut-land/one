use std::path::PathBuf;

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
        let Some(text) = path().and_then(|path| std::fs::read_to_string(path).ok()) else {
            return Self::default();
        };
        let mut state = Self::default();
        for line in text.lines() {
            if let Some(id) = line.strip_prefix("selected=").filter(|id| !id.is_empty()) {
                state.selected = Some(id.to_owned());
            }
            if line == "collapsed=true" {
                state.collapsed = true;
            }
        }
        state
    }

    pub fn save(&self) {
        let Some(path) = path() else {
            return;
        };
        let result = (|| -> std::io::Result<()> {
            std::fs::create_dir_all(path.parent().expect("state directory"))?;
            let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
            let selected = self
                .selected
                .as_deref()
                .unwrap_or_default()
                .replace(['\n', '\r'], "");
            std::fs::write(
                &temporary,
                format!("selected={selected}\ncollapsed={}\n", self.collapsed),
            )?;
            std::fs::rename(temporary, path)
        })();
        if let Err(error) = result {
            eprintln!("Could not save navigation: {error}");
        }
    }
}
