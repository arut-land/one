use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("dev.arut.bindings"))
        .files(["src/chat.rs"])
        .build()
        .export();
}
