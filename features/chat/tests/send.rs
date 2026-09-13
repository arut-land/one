mod support;
use arut_feature_chat::composer::ComposerAuthority;
use arut_feature_chat::composer::ComposerServiceImpl;
use arut_feature_chat::{ChatClient, ChatStatus};
use arut_protocol::chat::composer::v1::*;
use arut_protocol::chat::v1::*;
use arut_rpc::{Cancellation, Code, Request, Response, RpcFuture, RpcStream, Status};
use futures_executor::block_on;
use std::sync::{Arc, Mutex};
use support::NativeIds;

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

struct FailedClear(ComposerServiceImpl);
impl ComposerService for FailedClear {
    fn get_composer(
        &self,
        request: Request<GetComposerRequest>,
    ) -> RpcFuture<Response<GetComposerResponse>> {
        self.0.get_composer(request)
    }
    fn replace_composer(
        &self,
        request: Request<ReplaceComposerRequest>,
    ) -> RpcFuture<Response<ReplaceComposerResponse>> {
        if request.message.text.is_empty() {
            Box::pin(async { Err(Status::new(Code::Internal, "draft disk unavailable")) })
        } else {
            self.0.replace_composer(request)
        }
    }
    fn watch_composer(
        &self,
        request: Request<WatchComposerRequest>,
    ) -> RpcFuture<Response<RpcStream<WatchComposerResponse>>> {
        self.0.watch_composer(request)
    }
}

#[test]
fn retry_keeps_command_identity_and_cleanup_failure_keeps_accepted_messages() {
    let service = Arc::new(AcceptThenLoseResponse::default());
    let composer = Arc::new(FailedClear(ComposerServiceImpl::new(Arc::new(
        ComposerAuthority::default(),
    ))));
    let chat = ChatClient::established(
        ChatServiceClient::direct(service.clone()),
        ComposerServiceClient::direct(composer),
        "chat".into(),
        vec![],
        Arc::new(NativeIds),
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
