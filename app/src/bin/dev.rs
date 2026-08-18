//! Development server (decision D3).
//!
//! Production is a static directory; this exists only so the write-and-refresh
//! loop is fast locally. Regenerate with `cargo site`, then reload.
//!
//!   cargo dev [port]        default: 8080, or the PORT environment variable

use axum::Router;
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("app has a parent")
        .join("site");

    anyhow::ensure!(
        root.join("index.html").is_file(),
        "no site at {} — run `cargo site` first",
        root.display()
    );

    let port: u16 = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("PORT").ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let app = Router::new().fallback_service(ServeDir::new(&root));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("serving {} on http://{addr}", root.display());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
