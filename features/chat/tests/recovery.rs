use arut_feature_chat::{
    composer::{ComposerScope, ReplaceComposer, authority::ComposerAuthority},
    ports::NativeIds,
    service::ChatServiceImpl,
};
use arut_protocol::chat::v1::{ChatService, SendMessageRequest, StartChatRequest};
use arut_rpc::Request;
use arut_storage::{Directory, KeyValue};
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
    let directory = Arc::new(Directory::open(&path).unwrap());
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
        Arc::new(directory.log("chat").unwrap()),
        Arc::new(NativeIds),
    )
    .unwrap();
    let first = block_on(service.start_chat(Request::new(StartChatRequest {
        pending_scope_id: "owner".into(),
        command_id: "start".into(),
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
    let recovered_composer = Arc::new(ComposerAuthority::with_store(directory.clone()));
    let recovered = ChatServiceImpl::new(
        recovered_composer.clone(),
        Arc::new(directory.log("chat").unwrap()),
        Arc::new(NativeIds),
    )
    .unwrap();
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
    assert!(directory.get("draft:pending:owner").unwrap().is_none());
    std::fs::remove_dir_all(path).unwrap();
}
