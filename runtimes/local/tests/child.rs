#![cfg(unix)]
use arut_product_session::{ProductSession, hosting::Host};
use arut_runtime_host_polled::NativeIds;
use arut_runtime_local::{child::ChildHost, hosting::TokioSpawner};
use std::sync::Arc;
#[tokio::test]
async fn child_process_hosts_a_chat_over_unix_socket() {
    let dir = std::env::temp_dir().join(format!("arut-child-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let host = ChildHost {
        executable: env!("CARGO_BIN_EXE_arutd").into(),
        socket: dir.join("node.sock"),
        data: dir.join("data.pb"),
        spawner: Arc::new(TokioSpawner(tokio::runtime::Handle::current())),
    };
    let channel = host.connect().await.unwrap();
    let session = ProductSession::new(
        arut_product_session::chat::ChatClients::remote(channel.clone()),
        arut_product_session::CapabilityServiceClient::remote(channel),
        arut_product_session::SessionScope {
            node_id: "local".into(),
            workspace_id: "default".into(),
            pending_scope_id: "test".into(),
        },
        Arc::new(NativeIds),
    );
    let chat = session.chat();
    let state = chat.send("through child IPC".into()).await;
    assert_eq!(chat.messages_after(0).len(), 2, "{:?}", state.error);
    assert_eq!(session.chat_summaries()[0].title, "through child IPC");
    drop(chat);
    drop(session);
    std::fs::remove_dir_all(dir).unwrap();
}
