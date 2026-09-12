use arut_bindings_qt::chat::{create_chat_model, model_property};
use arut_product_session::ProductSession;
use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QString, QUrl,
};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() -> ExitCode {
    let mut app = QGuiApplication::new();
    let Some(app) = app.as_mut() else {
        return ExitCode::FAILURE;
    };
    let session = Arc::new(ProductSession::local());
    let mut chat = create_chat_model(Arc::clone(&session));
    let mut engine = QQmlApplicationEngine::new();

    let loaded = if let Some(mut engine) = engine.as_mut() {
        let failed = Arc::new(AtomicBool::new(false));
        let failed_signal = Arc::clone(&failed);
        let failure = engine.as_mut().on_object_creation_failed(move |_, url| {
            eprintln!("failed to load QML object: {url}");
            failed_signal.store(true, Ordering::Release);
        });
        let mut properties = QMap::<QMapPair_QString_QVariant>::default();
        // Locals drop in reverse order, so the engine is destroyed before the model.
        let chat_property = unsafe { model_property(chat.pin_mut()) };
        properties.insert(QString::from("chat"), chat_property);
        engine.as_mut().set_initial_properties(&properties);
        engine.load(&QUrl::from("qrc:/qt/qml/dev/arut/linux/qml/Main.qml"));
        drop(failure);
        !failed.load(Ordering::Acquire)
    } else {
        false
    };
    if !loaded {
        return ExitCode::FAILURE;
    }

    if app.exec() == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
