use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Request, State},
    http::{StatusCode, Uri, header::CONTENT_TYPE},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tower_http::{compression::CompressionLayer, services::ServeDir};

use nyanbin::{
    AppState, config::Config, cors, csp, error::ApiError, health, note, status, store::Store,
};

#[tokio::main]
async fn main() {
    if let Err(message) = run().await {
        eprintln!("nyanbin backend failed: {message}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let config = Config::from_env()?;
    let store = Store::connect(
        &config.redis_url,
        config.redis_prefix.clone(),
        config.redis_timeout,
    )
    .await?;
    store
        .ping()
        .await
        .map_err(|_| "Valkey readiness check failed".to_string())?;
    let body_limit = config.http_body_limit();
    let listen_addr = config.listen_addr;
    let state = Arc::new(AppState { config, store });
    spawn_meter_resync(state.clone());

    let notes = Router::new()
        .route(
            "/reserve",
            post(note::reserve).route_layer(middleware::from_fn_with_state(
                state.clone(),
                note::write_rate_limit,
            )),
        )
        .route(
            "/{id}",
            put(note::commit)
                .get(note::info)
                .delete(note::delete_note)
                .route_layer(middleware::from_fn_with_state(
                    state.clone(),
                    note::note_rate_limit,
                )),
        )
        .route(
            "/{id}/reveal",
            post(note::reveal).route_layer(middleware::from_fn_with_state(
                state.clone(),
                note::reveal_rate_limit,
            )),
        )
        .route(
            "/{id}/short",
            post(note::create_short).route_layer(middleware::from_fn_with_state(
                state.clone(),
                note::short_rate_limit,
            )),
        );
    let api = Router::new()
        .nest("/notes", notes)
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
        .route("/status", get(status::get_status))
        .route("/openapi.json", get(status::openapi))
        .route(
            "/short/{code}",
            get(note::resolve_short).route_layer(middleware::from_fn_with_state(
                state.clone(),
                note::short_rate_limit,
            )),
        )
        .fallback(api_not_found);

    let app = Router::new()
        .nest("/api", api)
        .fallback(serve_frontend)
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(
            CompressionLayer::new()
                .br(true)
                .deflate(true)
                .gzip(true)
                .zstd(true),
        )
        .layer(middleware::from_fn(normalize_api_errors))
        .layer(middleware::from_fn_with_state(state.clone(), cors::cors))
        .layer(middleware::from_fn(csp::security_headers))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .map_err(|error| format!("could not bind listener: {error}"))?;
    println!(
        "nyanbin listening on {}",
        listener
            .local_addr()
            .map_err(|error| format!("could not inspect listener: {error}"))?
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|error| format!("HTTP server failed: {error}"))
}
/// Static frontend resolution: exact file, then the SvelteKit-prerendered
/// `{path}.html` for extensionless routes, then the SPA shell at 200 so
/// client-side routes (`/note/{id}`) hydrate instead of surfacing a 404.
async fn serve_frontend(State(state): State<Arc<AppState>>, request: Request<Body>) -> Response {
    let root = state.config.frontend_path.clone();
    let method = request.method().clone();
    let path = request.uri().path().trim_end_matches('/').to_owned();
    if let Some(exact) = try_static(&root, request).await {
        return exact;
    }
    let last_segment = path.rsplit('/').next().unwrap_or("");
    if !path.is_empty() && !last_segment.is_empty() && !last_segment.contains('.') {
        // Delegate to ServeDir again so its path sanitization applies.
        if let Ok(uri) = Uri::try_from(format!("{path}.html")) {
            let rewritten = Request::builder()
                .method(method.clone())
                .uri(uri)
                .body(Body::empty())
                .expect("static request rebuild cannot fail");
            if let Some(page) = try_static(&root, rewritten).await {
                return page;
            }
        }
    }
    let shell = Request::builder()
        .method(method)
        .uri("/index.html")
        .body(Body::empty())
        .expect("static request rebuild cannot fail");
    try_static(&root, shell)
        .await
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

/// Runs one request through `ServeDir`, returning `None` on 404 so callers
/// can chain fallbacks. Filesystem failures surface as 500 rather than
/// falling through to a misleading page.
async fn try_static(root: &std::path::Path, request: Request<Body>) -> Option<Response> {
    let response = match ServeDir::new(root).try_call(request).await {
        Ok(response) => response.map(Body::new),
        Err(_) => return Some(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    };
    (response.status() != StatusCode::NOT_FOUND).then_some(response)
}

/// Periodically recomputes the storage meter from actual note sizes so
/// PEXPIREAT evictions cannot leave permanent drift. Skipped when the budget
/// is disabled.
fn spawn_meter_resync(state: Arc<AppState>) {
    if state.config.storage_budget_bytes == 0 {
        return;
    }
    let interval = state.config.meter_resync_interval;
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // The first tick fires immediately: correct the meter on startup.
        loop {
            ticker.tick().await;
            // Errors are tolerated; the next tick retries. Never log details.
            let _ =
                tokio::time::timeout(Duration::from_secs(300), state.store.resync_meter()).await;
        }
    });
}

async fn api_not_found() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "route_not_found",
        "API route not found",
    )
}

async fn normalize_api_errors(request: Request<Body>, next: Next) -> Response {
    let is_api = request.uri().path().starts_with("/api/") || request.uri().path() == "/api";
    let response = next.run(request).await;
    if !is_api || !response.status().is_client_error() && !response.status().is_server_error() {
        return response;
    }
    let is_json = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"));
    if is_json {
        return response;
    }
    match response.status() {
        StatusCode::METHOD_NOT_ALLOWED => ApiError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "Method not allowed",
        )
        .into_response(),
        StatusCode::PAYLOAD_TOO_LARGE => ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "Request body is too large",
        )
        .into_response(),
        StatusCode::NOT_FOUND => ApiError::new(
            StatusCode::NOT_FOUND,
            "route_not_found",
            "API route not found",
        )
        .into_response(),
        _ => ApiError::new(
            response.status(),
            "invalid_request",
            "Request could not be processed",
        )
        .into_response(),
    }
}
