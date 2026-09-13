use crate::test_support::TestIds;
use crate::{
    ChatServiceImpl,
    composer::{ComposerAuthority, ComposerScope, ReplaceComposer},
};
use arut_protocol::chat::v1::{ChatService, SendMessageRequest, StartChatRequest};
use arut_rpc::Request;
use arut_storage::Redb;
use futures_executor::block_on;
use std::sync::Arc;
fn transcript(service: &ChatServiceImpl, chat_id: &str) -> usize {
    service
        .projection()
        .conversation(chat_id)
        .expect("conversation was started")
        .messages
        .len()
}

#[test]
fn restart_recovers_transcript_operations_drafts_and_send_deduplication() {
    let path = std::env::temp_dir().join(format!("arut-recovery-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    let directory = Arc::new(Redb::open(path.join("node.redb")).unwrap());
    let composer = Arc::new(ComposerAuthority::with_store(directory.clone()));
    composer
        .replace(ReplaceComposer {
            scope: ComposerScope::pending("owner"),
            command_id: "draft".into(),
            authority_epoch: 1,
            base_revision: 0,
            text: "first".into(),
        })
        .unwrap();
    let service = ChatServiceImpl::new(
        composer.clone(),
        Arc::new(directory.log()),
        Arc::new(TestIds),
    )
    .unwrap();
    let first = block_on(service.start_chat(Request::new(StartChatRequest {
        pending_scope_id: "owner".into(),
        command_id: "01900000-0000-7000-8000-000000000001".into(),
        expected_revision: 1,
        text: "first".into(),
    })))
    .unwrap()
    .message;
    let send = SendMessageRequest {
        chat_id: first.chat_id.clone(),
        command_id: uuid::Uuid::now_v7().to_string(),
        text: "second".into(),
    };
    let sent = block_on(service.send_message(Request::new(send.clone())))
        .unwrap()
        .message;
    composer
        .replace(ReplaceComposer {
            scope: ComposerScope::chat(&first.chat_id),
            command_id: "next-draft".into(),
            authority_epoch: 1,
            base_revision: 0,
            text: "unfinished".into(),
        })
        .unwrap();
    drop(service);
    drop(composer);
    drop(directory);
    let directory = Arc::new(Redb::open(path.join("node.redb")).unwrap());
    let recovered_composer = Arc::new(ComposerAuthority::with_store(directory.clone()));
    let recovered = ChatServiceImpl::new(
        recovered_composer.clone(),
        Arc::new(directory.log()),
        Arc::new(TestIds),
    )
    .unwrap();
    assert!(
        recovered.projection().conversations[0]
            .messages
            .iter()
            .all(|message| message.accepted_at_ms == 123)
    );
    assert_eq!(transcript(&recovered, &first.chat_id), 4);
    assert_eq!(recovered.projection().completed_operations.len(), 2);
    assert_eq!(
        recovered_composer
            .snapshot(&ComposerScope::chat(&first.chat_id))
            .unwrap()
            .text,
        "unfinished"
    );
    assert_eq!(
        block_on(recovered.send_message(Request::new(send)))
            .unwrap()
            .message,
        sent
    );
    assert_eq!(transcript(&recovered, &first.chat_id), 4);
    assert!(
        recovered_composer
            .snapshot(&ComposerScope::pending("owner"))
            .unwrap()
            .text
            .is_empty()
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn committed_promotion_recovers_after_cleanup_failure_without_erasing_later_edits() {
    use arut_storage::{KeyValue, MemoryLog, MemoryStore, StorageError};
    use std::sync::atomic::{AtomicBool, Ordering};
    #[derive(Default)]
    struct Store {
        data: MemoryStore,
        fail: AtomicBool,
    }
    impl KeyValue for Store {
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
            self.data.get(key)
        }
        fn put(&self, key: &str, value: &[u8]) -> Result<(), StorageError> {
            if self.fail.load(Ordering::Relaxed) {
                return Err(StorageError::Io(std::io::ErrorKind::StorageFull));
            }
            self.data.put(key, value)
        }
        fn remove(&self, key: &str) -> Result<(), StorageError> {
            self.data.remove(key)
        }
    }
    let store = Arc::new(Store::default());
    let log = Arc::new(MemoryLog::default());
    let composer = Arc::new(ComposerAuthority::with_store(store.clone()));
    let scope = ComposerScope::pending("owner");
    composer
        .replace(ReplaceComposer {
            scope: scope.clone(),
            command_id: "draft".into(),
            authority_epoch: 1,
            base_revision: 0,
            text: "accepted".into(),
        })
        .unwrap();
    let request = StartChatRequest {
        pending_scope_id: "owner".into(),
        command_id: uuid::Uuid::now_v7().to_string(),
        expected_revision: 1,
        text: "accepted".into(),
    };
    let service = ChatServiceImpl::new(composer.clone(), log.clone(), Arc::new(TestIds)).unwrap();
    store.fail.store(true, Ordering::Relaxed);
    assert!(block_on(service.start_chat(Request::new(request.clone()))).is_err());
    assert_eq!(service.projection().conversations.len(), 1);
    drop(service);
    drop(composer);

    store.fail.store(false, Ordering::Relaxed);
    let composer = Arc::new(ComposerAuthority::with_store(store.clone()));
    let service = ChatServiceImpl::new(composer.clone(), log.clone(), Arc::new(TestIds)).unwrap();
    let cleared = composer.snapshot(&scope).unwrap();
    assert_eq!(cleared.revision, 2);
    assert!(cleared.text.is_empty());
    let accepted = block_on(service.start_chat(Request::new(request.clone())))
        .unwrap()
        .message;
    assert_eq!(transcript(&service, &accepted.chat_id), 2);
    composer
        .replace(ReplaceComposer {
            scope: scope.clone(),
            command_id: "later".into(),
            authority_epoch: 1,
            base_revision: 2,
            text: "keep this".into(),
        })
        .unwrap();
    block_on(service.start_chat(Request::new(request))).unwrap();
    assert_eq!(composer.snapshot(&scope).unwrap().text, "keep this");
    drop(service);
    drop(composer);
    let composer = Arc::new(ComposerAuthority::with_store(store));
    let _service = ChatServiceImpl::new(composer.clone(), log, Arc::new(TestIds)).unwrap();
    assert_eq!(composer.snapshot(&scope).unwrap().text, "keep this");
}
