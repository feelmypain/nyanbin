use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Request},
    http::{StatusCode, header::CONTENT_TYPE},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use std::{net::SocketAddr, sync::Arc};
use tower_http::{
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
};

mod config;
mod csp;
mod error;
mod health;
mod note;
mod status;
mod store;

use config::Config;
use error::ApiError;
use store::Store;

#[derive(Clone)]
pub struct AppState {
    config: Config,
    store: Store,
}

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
    let frontend_path = config.frontend_path.clone();
    let state = Arc::new(AppState { config, store });

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
                .route_layer(middleware::from_fn_with_state(
                    state.clone(),
                    note::write_rate_limit,
                ))
                .get(note::info)
                .delete(note::delete_note),
        )
        .route("/{id}/reveal", post(note::reveal));
    let api = Router::new()
        .nest("/notes", notes)
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
        .route("/status", get(status::get_status))
        .fallback(api_not_found);

    let index = frontend_path.join("index.html");
    let static_files = ServeDir::new(frontend_path).not_found_service(ServeFile::new(index));
    let app = Router::new()
        .nest("/api", api)
        .fallback_service(static_files)
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(
            CompressionLayer::new()
                .br(true)
                .deflate(true)
                .gzip(true)
                .zstd(true),
        )
        .layer(middleware::from_fn(normalize_api_errors))
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
