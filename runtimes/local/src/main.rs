//! Daemon composition root: choose native ports, list features, serve their routers.
use std::{env, path::PathBuf};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = PathBuf::from(env::var("ARUT_DATA").unwrap_or_else(|_| "arut-chat.pb".into()));
    let runtime = std::sync::Arc::new(arut_runtime_local::LocalRuntime::open(data, "chat")?);
    let chat = arut_feature_chat::compose(runtime.clone())?;
    let app = arut_runtime_local::Node::serve(runtime, chat.routers())?;
    #[cfg(unix)]
    if let Ok(socket) = env::var("ARUT_SOCKET") {
        use std::os::unix::fs::PermissionsExt;
        let listener = tokio::net::UnixListener::bind(&socket)?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
        println!("READY");
        axum::serve(listener, app).await?;
        return Ok(());
    }
    let address = env::var("ARUT_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("READY");
    axum::serve(listener, app).await?;
    Ok(())
}
