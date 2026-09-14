use crate::composer::ComposerAuthority;
use crate::composer::ComposerServiceImpl;
use crate::test_support::{Intercept, MemoryPorts, TestIds};
use crate::{ChatClient, ChatStatus};
use arut_protocol::chat::composer::v1::*;
use arut_protocol::chat::v1::*;
use arut_rpc::{Cancellation, Code, Request, Response, RpcFuture, Status};
use futures_executor::block_on;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct AcceptThenLoseResponse(Mutex<Vec<SendMessageRequest>>);
impl ChatService for AcceptThenLoseResponse {
    fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> RpcFuture<Response<SendMessageResponse>> {
        let mut calls = self.0.lock().unwrap();
        calls.push(request.message.clone());
        let first = calls.len() == 1;
        Box::pin(async move {
            if first {
                return Err(Status::new(Code::Unavailable, "response lost after commit"));
            }
            Ok(Response::new(SendMessageResponse {
                messages: vec![ChatMessage {
                    id: 1,
                    accepted_at_ms: 123,
                    role: ChatRole::User as i32,
                    text: request.message.text,
                }],
            }))
        })
    }
    fn start_chat(&self, _: Request<StartChatRequest>) -> RpcFuture<Response<StartChatResponse>> {
        unreachable!()
    }
    fn list_conversations(
        &self,
        _: Request<ListConversationsRequest>,
    ) -> RpcFuture<Response<ListConversationsResponse>> {
        unreachable!()
    }
}

/// A composer whose disk refuses the empty write that clears a sent draft.
fn failing_clear() -> ComposerServiceClient {
    ComposerServiceClient::direct(Arc::new(
        Intercept::new(ComposerServiceImpl::new(Arc::new(
            ComposerAuthority::default(),
        )))
        .on_replace(|inner, request| {
            if request.message.text.is_empty() {
                Box::pin(async { Err(Status::new(Code::Internal, "draft disk unavailable")) })
            } else {
                inner.replace_composer(request)
            }
        }),
    ))
}

#[test]
fn retry_keeps_command_identity_and_cleanup_failure_keeps_accepted_messages() {
    let service = Arc::new(AcceptThenLoseResponse::default());
    let chat = ChatClient::established(
        ChatServiceClient::direct(service.clone()),
        failing_clear(),
        "chat".into(),
        vec![],
        Arc::new(TestIds),
        Arc::new(Cancellation::root()),
    );
    block_on(chat.composer().replace("hello".into()));
    assert_eq!(
        block_on(chat.send("hello".into())).status,
        ChatStatus::Failed
    );
    let accepted = block_on(chat.send("hello".into()));
    assert_eq!(accepted.status, ChatStatus::Idle);
    assert_eq!(accepted.last_message_id, 1);
    assert_eq!(chat.messages_after(0)[0].text, "hello");
    assert!(chat.composer().state().error.is_some());
    {
        let calls = service.0.lock().unwrap();
        assert_eq!(calls[0].command_id, calls[1].command_id);
    }
    block_on(chat.send("next".into()));
    let calls = service.0.lock().unwrap();
    assert_ne!(calls[1].command_id, calls[2].command_id);
}

/// A person types faster than the node answers, then sends. The send waits for
/// the draft it commits, so the node promotes the text they actually typed.
#[test]
fn a_send_after_rapid_edits_carries_the_last_text() {
    let feature = crate::compose(Arc::new(MemoryPorts::new(Arc::new(TestIds)))).unwrap();
    let clients = feature.clients();
    let chat = ChatClient::pending(
        clients.chat,
        clients.composer,
        "pending".into(),
        None,
        Arc::new(TestIds),
        Arc::new(Cancellation::root()),
    );

    // None of these edits is awaited: the last one is still waiting to be
    // written when the send starts.
    let composer = chat.composer();
    drop(
        (1..=50)
            .map(|n| composer.replace(format!("edit {n}")))
            .collect::<Vec<_>>(),
    );
    let state = block_on(chat.send("edit 50".into()));

    assert_eq!(state.error, None);
    assert_eq!(chat.messages_after(0)[0].text, "edit 50");
    assert_eq!(chat.composer().state().text, "");
}
