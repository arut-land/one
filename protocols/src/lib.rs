mod generated {
    include!(concat!(env!("OUT_DIR"), "/arut.protocols.rs"));
}

pub use generated::arut::*;

pub mod capability_manifest;

#[cfg(test)]
mod tests {
    use crate::chat::v1::{
        CHAT_SERVICE_DESCRIPTOR, ChatMessage, ChatRole, ChatService, ChatServiceClient,
        ChatServiceRouter, SendMessageRequest, SendMessageResponse, StartChatRequest,
        StartChatResponse,
    };
    use arut_rpc::{Request, Response, RpcFuture};
    use futures_executor::block_on;
    use std::sync::Arc;

    struct Responder;

    impl ChatService for Responder {
        fn list_conversations(
            &self,
            _: Request<crate::chat::v1::ListConversationsRequest>,
        ) -> RpcFuture<Response<crate::chat::v1::ListConversationsResponse>> {
            unreachable!()
        }

        fn send_message(
            &self,
            request: Request<SendMessageRequest>,
        ) -> RpcFuture<Response<SendMessageResponse>> {
            Box::pin(async move {
                Ok(Response::new(SendMessageResponse {
                    messages: vec![ChatMessage {
                        id: 1,
                        role: ChatRole::Assistant as i32,
                        text: format!("You said: {}", request.message.text),
                    }],
                }))
            })
        }

        fn start_chat(
            &self,
            _request: Request<StartChatRequest>,
        ) -> RpcFuture<Response<StartChatResponse>> {
            unreachable!()
        }
    }

    #[test]
    fn generated_direct_client_passes_typed_values() {
        let client = ChatServiceClient::direct(Arc::new(Responder));
        let response = block_on(client.send_message(Request::new(SendMessageRequest {
            command_id: "test".into(),
            chat_id: "chat".into(),
            text: "direct".into(),
        })))
        .unwrap();

        assert_eq!(response.message.messages[0].text, "You said: direct");
    }

    #[test]
    fn generated_remote_client_and_router_handle_wire_values() {
        let router = Arc::new(ChatServiceRouter::new(Arc::new(Responder)));
        let client = ChatServiceClient::remote(router);
        let response = block_on(client.send_message(Request::new(SendMessageRequest {
            command_id: "test".into(),
            chat_id: "chat".into(),
            text: "remote".into(),
        })))
        .unwrap();

        assert_eq!(response.message.messages[0].text, "You said: remote");
    }

    #[test]
    fn generates_canonical_service_descriptors() {
        assert_eq!(CHAT_SERVICE_DESCRIPTOR.package, "arut.chat.v1");
        assert_eq!(CHAT_SERVICE_DESCRIPTOR.version, "v1");
        assert_eq!(
            CHAT_SERVICE_DESCRIPTOR.methods[0].procedure,
            "/arut.chat.v1.ChatService/SendMessage"
        );
    }
}
