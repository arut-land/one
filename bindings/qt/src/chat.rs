use crate::observable::ObservableState;
use arut_feature_chat::composer::product::ComposerState;
use arut_feature_chat::product::{
    ChatClient, ChatRole, ChatState, ChatStatus as ProductChatStatus,
};
use arut_product_session::ProductSession;
use core::pin::Pin;
use cxx::UniquePtr;
use cxx_qt::{CxxQtType, QObject, Threading, casting::Upcast};
use cxx_qt_lib::{QObjectMutPtr, QString, QStringList, QVariant};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    #[qenum(ChatModel)]
    pub enum ChatStatus {
        Idle,
        Sending,
        Failed,
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_uncreatable]
        #[qproperty(QString, transcript, READ, NOTIFY)]
        #[qproperty(QStringList, message_texts, cxx_name = "messageTexts", READ, NOTIFY)]
        #[qproperty(QStringList, message_roles, cxx_name = "messageRoles", READ, NOTIFY)]
        #[qproperty(QString, draft, READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        #[qproperty(QStringList, history_titles, cxx_name = "historyTitles", READ, NOTIFY)]
        #[qproperty(QString, selected_chat_id, cxx_name = "selectedChatId", READ, NOTIFY)]
        #[qproperty(
            i32,
            selected_history_index,
            cxx_name = "selectedHistoryIndex",
            READ,
            NOTIFY
        )]
        #[qproperty(ChatStatus, status, READ, NOTIFY)]
        type ChatModel = super::ChatModelRust;

        #[qinvokable]
        fn send(self: Pin<&mut Self>, text: &QString);
        #[qinvokable]
        #[cxx_name = "newChat"]
        fn new_chat(self: Pin<&mut Self>);
        #[qinvokable]
        #[cxx_name = "replaceDraft"]
        fn replace_draft(self: Pin<&mut Self>, text: &QString);
        #[qinvokable]
        #[cxx_name = "selectChat"]
        fn select_chat(self: Pin<&mut Self>, index: i32);
    }

    impl cxx_qt::Threading for ChatModel {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[rust_name = "new_chat_model"]
        fn make_unique() -> UniquePtr<ChatModel>;
    }
}

pub struct ChatModelRust {
    transcript: QString,
    message_texts: QStringList,
    message_roles: QStringList,
    draft: QString,
    error: QString,
    history_titles: QStringList,
    selected_chat_id: QString,
    selected_history_index: i32,
    history_ids: Vec<String>,
    generation: u64,
    status: qobject::ChatStatus,
    session: Option<Arc<ProductSession>>,
    client: Option<Arc<ChatClient>>,
    observable: Option<ObservableState>,
    composer_observer: Option<ComposerObserver>,
}

impl Default for ChatModelRust {
    fn default() -> Self {
        Self {
            transcript: QString::default(),
            message_texts: QStringList::default(),
            message_roles: QStringList::default(),
            draft: QString::default(),
            error: QString::default(),
            history_titles: QStringList::default(),
            selected_chat_id: QString::default(),
            selected_history_index: -1,
            history_ids: Vec::new(),
            generation: 0,
            status: qobject::ChatStatus::Idle,
            session: None,
            client: None,
            observable: None,
            composer_observer: None,
        }
    }
}

struct ComposerObserver {
    _observable: ObservableState,
    commands: Option<Sender<ChatCommand>>,
    worker: Option<JoinHandle<()>>,
}

enum ChatCommand {
    Replace(String),
    Send(String),
}

impl Drop for ComposerObserver {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn create_chat_model(session: Arc<ProductSession>) -> UniquePtr<qobject::ChatModel> {
    let mut model = qobject::new_chat_model();
    model.pin_mut().attach(session);
    model
}

/// Creates the QObject-valued property used to inject this model into QML.
///
/// # Safety
///
/// `model` must outlive the QML engine and every copy of the returned value.
pub unsafe fn model_property(mut model: Pin<&mut qobject::ChatModel>) -> QVariant {
    let object: Pin<&mut QObject> = model.as_mut().upcast_pin();
    let object = unsafe { QObjectMutPtr::from_raw(Pin::get_unchecked_mut(object)) };
    QVariant::from(&object)
}

impl qobject::ChatModel {
    fn attach(mut self: Pin<&mut Self>, session: Arc<ProductSession>) {
        let client = Arc::new(session.chat());
        self.as_mut().rust_mut().session = Some(session);
        self.bind(client);
    }

    fn bind(mut self: Pin<&mut Self>, client: Arc<ChatClient>) {
        self.as_mut().rust_mut().generation += 1;
        self.as_mut().rust_mut().composer_observer = None;
        self.as_mut().rust_mut().observable = None;
        let chat_state = client.state();
        let composer = client.composer();

        let qt_thread = self.qt_thread();
        let mut observable = ObservableState::new(client.changes());
        observable.observe(qt_thread, |mut model: Pin<&mut qobject::ChatModel>| {
            let state = model
                .as_ref()
                .get_ref()
                .rust()
                .client
                .as_ref()
                .expect("ChatModel is not attached")
                .state();
            model.as_mut().apply_state(state);
        });

        let qt_thread = self.qt_thread();
        let mut composer_observable = ObservableState::new(composer.changes());
        composer_observable.observe(qt_thread, |mut model: Pin<&mut qobject::ChatModel>| {
            let state = model
                .as_ref()
                .get_ref()
                .rust()
                .client
                .as_ref()
                .expect("ChatModel is not attached")
                .composer()
                .state();
            model.as_mut().apply_composer(state);
        });
        let (commands, command_rx) = mpsc::channel();
        let worker_composer = composer.clone();
        let worker_client = Arc::clone(&client);
        let qt_thread = self.qt_thread();
        let generation = self.as_ref().get_ref().rust().generation;
        let worker = std::thread::spawn(move || {
            futures_executor::block_on(worker_composer.initialize());
            loop {
                match command_rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(ChatCommand::Replace(text)) => {
                        futures_executor::block_on(worker_composer.replace(text));
                    }
                    Ok(ChatCommand::Send(text)) => {
                        let state = futures_executor::block_on(worker_client.send(text));
                        let composer = worker_composer.state();
                        let _ = qt_thread.queue(move |mut model| {
                            if model.as_ref().get_ref().rust().generation != generation {
                                return;
                            }
                            model.as_mut().apply_state(state);
                            model.as_mut().apply_composer(composer);
                        });
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        futures_executor::block_on(worker_composer.sync_once());
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        self.as_mut().rust_mut().client = Some(client);
        self.as_mut().rust_mut().observable = Some(observable);
        self.as_mut().rust_mut().composer_observer = Some(ComposerObserver {
            _observable: composer_observable,
            commands: Some(commands),
            worker: Some(worker),
        });
        self.as_mut().apply_state(chat_state);
        self.as_mut().apply_composer(composer.state());
        self.apply_history();
    }

    fn send(self: Pin<&mut Self>, text: &QString) {
        let commands = self
            .as_ref()
            .get_ref()
            .rust()
            .composer_observer
            .as_ref()
            .expect("ChatModel is not attached")
            .commands
            .as_ref()
            .expect("ChatModel command lane is stopped");
        let _ = commands.send(ChatCommand::Send(String::from(text)));
    }

    fn new_chat(mut self: Pin<&mut Self>) {
        self.as_mut().stop_binding();
        let client = self
            .as_ref()
            .get_ref()
            .rust()
            .session
            .as_ref()
            .expect("ChatModel is not attached")
            .new_chat();
        self.as_mut().bind(Arc::new(client));
    }

    fn select_chat(mut self: Pin<&mut Self>, index: i32) {
        let Some(chat_id) = usize::try_from(index)
            .ok()
            .and_then(|index| self.as_ref().get_ref().rust().history_ids.get(index))
            .cloned()
        else {
            return;
        };
        self.as_mut().stop_binding();
        let client = self
            .as_ref()
            .get_ref()
            .rust()
            .session
            .as_ref()
            .expect("ChatModel is not attached")
            .select_chat(&chat_id);
        if let Some(client) = client {
            self.as_mut().bind(Arc::new(client));
        }
    }

    fn replace_draft(self: Pin<&mut Self>, text: &QString) {
        let commands = self
            .as_ref()
            .get_ref()
            .rust()
            .composer_observer
            .as_ref()
            .expect("ChatModel is not attached")
            .commands
            .as_ref()
            .expect("ChatModel command lane is stopped");
        let _ = commands.send(ChatCommand::Replace(String::from(text)));
    }

    fn stop_binding(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().generation += 1;
        self.as_mut().rust_mut().composer_observer = None;
        self.as_mut().rust_mut().observable = None;
    }

    fn apply_composer(mut self: Pin<&mut Self>, state: ComposerState) {
        let draft = QString::from(state.text);
        if self.draft() != &draft {
            self.as_mut().rust_mut().draft = draft;
            self.draft_changed();
        }
    }

    fn apply_state(mut self: Pin<&mut Self>, state: ChatState) {
        let selected_chat_id = QString::from(state.id.clone().unwrap_or_default());
        let message_texts = state
            .messages
            .iter()
            .map(|message| QString::from(message.text.as_str()))
            .collect::<QStringList>();
        let message_roles = state
            .messages
            .iter()
            .map(|message| {
                QString::from(if message.role == ChatRole::User {
                    "user"
                } else {
                    "assistant"
                })
            })
            .collect::<QStringList>();
        let transcript = QString::from(
            state
                .messages
                .iter()
                .map(|message| match message.role {
                    ChatRole::User => format!("You\n{}", message.text),
                    ChatRole::Assistant => format!("Arut\n{}", message.text),
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
        );
        let error = QString::from(state.error);
        let status = state.status.into();
        let transcript_changed = self.transcript() != &transcript;
        let messages_changed = self.message_texts() != &message_texts;
        let error_changed = self.error() != &error;
        let status_changed = *self.status() != status;
        let selected_chat_changed = self.selected_chat_id() != &selected_chat_id;
        {
            let mut current = self.as_mut().rust_mut();
            current.transcript = transcript;
            current.message_texts = message_texts;
            current.message_roles = message_roles;
            current.error = error;
            current.status = status;
            current.selected_chat_id = selected_chat_id;
        }
        if transcript_changed {
            self.as_mut().transcript_changed();
        }
        if messages_changed {
            self.as_mut().message_texts_changed();
            self.as_mut().message_roles_changed();
        }
        if error_changed {
            self.as_mut().error_changed();
        }
        if status_changed {
            self.as_mut().status_changed();
        }
        if selected_chat_changed {
            self.as_mut().selected_chat_id_changed();
        }
        self.apply_history();
    }

    fn apply_history(mut self: Pin<&mut Self>) {
        let summaries = self
            .as_ref()
            .get_ref()
            .rust()
            .session
            .as_ref()
            .expect("ChatModel is not attached")
            .chat_summaries();
        let titles = summaries
            .iter()
            .map(|summary| QString::from(summary.title.as_str()))
            .collect::<QStringList>();
        let selected_id = self.selected_chat_id().to_string();
        let selected_index = summaries
            .iter()
            .position(|summary| summary.id == selected_id)
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1);
        let ids = summaries.into_iter().map(|summary| summary.id).collect();
        let selected_index_changed = *self.selected_history_index() != selected_index;
        if self.history_titles() != &titles {
            {
                let mut current = self.as_mut().rust_mut();
                current.history_titles = titles;
                current.history_ids = ids;
            }
            self.as_mut().history_titles_changed();
        }
        if selected_index_changed {
            self.as_mut().rust_mut().selected_history_index = selected_index;
            self.selected_history_index_changed();
        }
    }
}

impl From<ProductChatStatus> for qobject::ChatStatus {
    fn from(status: ProductChatStatus) -> Self {
        match status {
            ProductChatStatus::Idle => Self::Idle,
            ProductChatStatus::Sending => Self::Sending,
            ProductChatStatus::Failed => Self::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cxx_qt_lib::QCoreApplication;
    use std::time::Instant;

    static QT_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn publishes_chat_and_composer_state_as_qt_types() {
        let _test = QT_TEST.lock().expect("Qt test lock poisoned");
        let app = QCoreApplication::new();
        let session = Arc::new(ProductSession::local());
        let mut model = create_chat_model(session);
        model.pin_mut().replace_draft(&QString::from("hello"));
        model.pin_mut().send(&QString::from("hello"));
        let deadline = Instant::now() + Duration::from_secs(1);
        while !model.transcript().to_string().contains("You said: hello")
            || !model.draft().is_empty()
        {
            app.as_ref().unwrap().process_events();
            assert!(Instant::now() < deadline, "Qt observer did not refresh");
            std::thread::yield_now();
        }
        assert_eq!(model.history_titles().len(), 1);
        model.pin_mut().new_chat();
        assert!(model.transcript().is_empty());
        model.pin_mut().replace_draft(&QString::from("second"));
        model.pin_mut().send(&QString::from("second"));
        let deadline = Instant::now() + Duration::from_secs(1);
        while !model.transcript().to_string().contains("You said: second") {
            app.as_ref().unwrap().process_events();
            assert!(Instant::now() < deadline, "Qt observer did not refresh");
            std::thread::yield_now();
        }
        assert_eq!(model.history_titles().len(), 2);
        model.pin_mut().select_chat(1);
        assert!(model.transcript().to_string().contains("You said: hello"));
    }

    #[test]
    fn new_chat_waits_for_queued_send_then_selects_clean_pending_state() {
        let _test = QT_TEST.lock().expect("Qt test lock poisoned");
        let app = QCoreApplication::new();
        let session = Arc::new(ProductSession::local());
        let mut model = create_chat_model(session);
        model.pin_mut().replace_draft(&QString::from("queued"));
        model.pin_mut().send(&QString::from("queued"));
        model.pin_mut().new_chat();

        app.as_ref().unwrap().process_events();
        assert!(model.transcript().is_empty());
        assert!(model.draft().is_empty());
        assert_eq!(model.history_titles().len(), 1);
        assert_eq!(*model.selected_history_index(), -1);
    }
}
