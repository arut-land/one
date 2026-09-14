//! Bundles this surface's icons into one GResource: the Lucide set its toolbar,
//! composer and transcript draw (`data/icons/lucide`, copied unchanged from the
//! 1.46.0 tag with its ISC license), converted to GTK's symbolic form so the
//! theme recolors them, and the hicolor app icon, so the window and the About
//! dialog have it without an installed icon theme. The conversion runs once
//! per change to those files and nothing here runs off Linux.
use std::{env, fs, path::PathBuf};

/// Lucide source name to the name this surface asks the icon theme for.
const LUCIDE: &[(&str, &str)] = &[
    ("panel-left-close", "sidebar-hide"),
    ("panel-left-open", "sidebar-show"),
    ("message-square-plus", "new-conversation"),
    ("menu", "menu"),
    ("arrow-up", "send"),
    ("arrow-down", "latest"),
    ("triangle-alert", "warning"),
    ("messages-square", "empty"),
    ("copy", "copy"),
    ("refresh-cw", "retry"),
];

/// Every SVG shape Lucide draws with. GTK recolors a symbolic icon by
/// injecting a stylesheet, and its `foreground-stroke` and `transparent-fill`
/// classes are how GTK's own stroke-drawn symbolic icons opt in.
const SHAPES: &[&str] = &[
    "<path",
    "<rect",
    "<circle",
    "<ellipse",
    "<line",
    "<polyline",
    "<polygon",
];

/// A filled dot for unread rows; Lucide has no filled shapes.
const UNREAD: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><circle cx="8" cy="8" r="4"/></svg>
"#;

fn symbolic(svg: &str) -> String {
    let mut symbolic = svg.to_owned();
    for shape in SHAPES {
        symbolic = symbolic.replace(
            shape,
            &format!("{shape} class=\"foreground-stroke transparent-fill\""),
        );
    }
    symbolic
}

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let lucide = manifest.join("data/icons/lucide");
    let hicolor = manifest.join("data/icons/hicolor");
    let icons = PathBuf::from(env::var("OUT_DIR").unwrap()).join("icons");
    println!("cargo:rerun-if-changed={}", lucide.display());
    println!("cargo:rerun-if-changed={}", hicolor.display());
    let actions = icons.join("scalable/actions");
    fs::create_dir_all(&actions).unwrap();
    let mut files = Vec::new();
    for (source, name) in LUCIDE {
        let svg = fs::read_to_string(lucide.join(format!("{source}.svg")))
            .unwrap_or_else(|error| panic!("{source}.svg: {error}"));
        let relative = format!("scalable/actions/arut-{name}-symbolic.svg");
        fs::write(icons.join(&relative), symbolic(&svg)).unwrap();
        files.push(relative);
    }
    let unread = "scalable/actions/arut-unread-symbolic.svg".to_owned();
    fs::write(icons.join(&unread), UNREAD).unwrap();
    files.push(unread);
    for entry in fs::read_dir(&hicolor).unwrap() {
        let size = entry.unwrap().file_name();
        let relative = format!("{}/apps/dev.arut.Arut.png", size.to_str().unwrap());
        fs::create_dir_all(icons.join(&size).join("apps")).unwrap();
        fs::copy(hicolor.join(&relative), icons.join(&relative)).unwrap();
        files.push(relative);
    }
    let listing: String = files
        .iter()
        .map(|file| format!("    <file>{file}</file>\n"))
        .collect();
    let manifest_xml = icons.join("arut.gresource.xml");
    fs::write(
        &manifest_xml,
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gresources>\n  <gresource prefix=\"/dev/arut/Arut/icons\">\n{listing}  </gresource>\n</gresources>\n"
        ),
    )
    .unwrap();
    glib_build_tools::compile_resources(
        &[icons.to_str().unwrap()],
        manifest_xml.to_str().unwrap(),
        "arut.gresource",
    );
}
