use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use std::sync::Arc;

use crate::{AppState, error::ApiError};

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct Health {
    status: &'static str,
}

pub async fn live() -> impl IntoResponse {
    (StatusCode::OK, Json(Health { status: "ok" }))
}

pub async fn ready(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    state.store.ping().await.map_err(|_| ApiError::storage())?;
    Ok((StatusCode::OK, Json(Health { status: "ok" })))
}
