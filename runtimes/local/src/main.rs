use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let address = env::var("ARUT_BACKEND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let data_path =
        PathBuf::from(env::var("ARUT_BACKEND_DATA").unwrap_or_else(|_| "arut-chat.pb".into()));
    let app = arut_runtime_local::app(data_path)
        .unwrap_or_else(|error| panic!("failed to initialize chat backend: {error}"));

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| panic!("failed to bind chat backend at {address}: {error}"));
    println!("Chat backend listening on http://{address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("Chat backend failed");
}
