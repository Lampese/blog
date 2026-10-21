//! HTTP transport for the loopback-only authoring API and preview artifacts.
use super::runtime::Preview;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{
        Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, time::Duration};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use tower_http::services::{ServeDir, ServeFile};

#[derive(Deserialize)]
struct CaptureRequest {
    article: String,
    revision: String,
    start: usize,
    end: usize,
}

#[derive(Serialize)]
struct CaptureResponse {
    code: String,
}

type ApiError = (StatusCode, Json<serde_json::Value>);

pub(super) fn router(preview: Preview) -> Router {
    Router::new()
        .route(
            "/_blog/status",
            get(|| async { Json(serde_json::json!({"development":true})) }),
        )
        .route("/_blog/references", post(capture))
        .route("/_blog/events", get(event_stream))
        .layer(DefaultBodyLimit::max(65536))
        .fallback_service(
            ServeDir::new(preview.output.clone())
                .append_index_html_on_directories(true)
                .not_found_service(ServeFile::new(preview.output.join("404.html"))),
        )
        .layer(middleware::from_fn(no_cache))
        .with_state(preview)
}

async fn no_cache(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn capture(
    State(preview): State<Preview>,
    headers: HeaderMap,
    Json(request): Json<CaptureRequest>,
) -> Result<Json<CaptureResponse>, ApiError> {
    validate_origin(&headers, preview.port)?;
    let include = preview
        .capture(
            request.article,
            request.revision,
            request.start..request.end,
        )
        .await
        .map_err(|error| api_error(StatusCode::CONFLICT, &format!("{error:#}")))?;
    Ok(Json(CaptureResponse {
        code: include.code(),
    }))
}

fn validate_origin(headers: &HeaderMap, port: u16) -> Result<(), ApiError> {
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "The preview authoring API only accepts loopback hosts",
        ));
    }
    if let Some(origin) = headers.get("origin").and_then(|h| h.to_str().ok())
        && origin != format!("http://{host}")
    {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Cross-origin writes are not allowed",
        ));
    }
    Ok(())
}

fn api_error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(serde_json::json!({"error":message})))
}

async fn event_stream(
    State(preview): State<Preview>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(preview.events.subscribe()).filter_map(|value| {
        value
            .ok()
            .and_then(|event| Event::default().json_data(event).ok())
            .map(Ok)
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoring_requires_a_loopback_host_and_a_matching_origin() {
        for (host, origin, allowed) in [
            (Some("127.0.0.1:7878"), None, true),
            (Some("localhost:7878"), Some("http://localhost:7878"), true),
            (Some("127.0.0.1:7878"), Some("http://127.0.0.1:7878"), true),
            (Some("example.com:7878"), None, false),
            (None, None, false),
            (Some("localhost:7879"), None, false),
            (Some("localhost:7878"), Some("https://example.com"), false),
            (Some("localhost:7878"), Some("http://127.0.0.1:7878"), false),
        ] {
            let mut headers = HeaderMap::new();
            if let Some(host) = host {
                headers.insert(header::HOST, host.parse().unwrap());
            }
            if let Some(origin) = origin {
                headers.insert(header::ORIGIN, origin.parse().unwrap());
            }
            assert_eq!(
                validate_origin(&headers, 7878).is_ok(),
                allowed,
                "host={host:?}, origin={origin:?}"
            );
        }
    }
}
