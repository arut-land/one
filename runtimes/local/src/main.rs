use std::{env, path::PathBuf};
#[tokio::main]
async fn main() {
    let data = PathBuf::from(env::var("ARUT_DATA").unwrap_or_else(|_| "arut-chat.pb".into()));
    let app = arut_runtime_local::app_with(data, arut_runtime_local::NodeStorage::from_env())
        .expect("initialize local node");
    #[cfg(unix)]
    if let Ok(socket) = env::var("ARUT_SOCKET") {
        use std::os::unix::fs::PermissionsExt;
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind IPC socket");
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
            .expect("protect IPC socket");
        println!("READY");
        axum::serve(listener, app).await.expect("serve IPC");
        return;
    }
    let address = env::var("ARUT_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("bind HTTP");
    println!("READY");
    axum::serve(listener, app).await.expect("serve HTTP");
}
