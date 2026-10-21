//! Start the preview server and tie its watcher to the server's lifetime.
mod http;
mod runtime;
mod watch;

use anyhow::Result;
use runtime::Preview;
use std::{net::Ipv4Addr, path::PathBuf};

pub async fn serve(root: PathBuf, port: u16) -> Result<()> {
    let preview = Preview::new(root, port).await?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
    let router = http::router(preview.clone());
    let watcher = tokio::spawn(watch::run(preview));
    eprintln!(
        "Preview: http://127.0.0.1:{port}/\nWatching articles, Typst modules, and assets; press Ctrl-C to stop."
    );
    // SSE connections are permanent; waiting for them to drain would keep
    // Ctrl-C alive indefinitely. Both the server and watcher stop together.
    let result = tokio::select! {
        result = async { axum::serve(listener, router).await } => result,
        _ = tokio::signal::ctrl_c() => Ok(()),
    };
    watcher.abort();
    Ok(result?)
}
