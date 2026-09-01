use axum::{
    Json,
    extract::State,
    http::{HeaderValue, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::sync::Arc;

use crate::{AppState, config::PROTOCOL_VERSION};

/// The frozen v1 contract, embedded at compile time so the served spec can
/// never drift from the binary. Regenerate with `pnpm run openapi:generate`.
const OPENAPI_JSON: &str = include_str!("../../openapi.json");

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    max_envelope_bytes: usize,
    max_expires_in: u64,
    max_reads: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Defaults {
    expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_reads: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    files: bool,
    passwords: bool,
    formats: [&'static str; 3],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Branding {
    name: String,
    description: String,
    logo_url: String,
    imprint_url: String,
    #[serde(rename = "abuseContact")]
    reserved_empty: &'static str,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    protocol: u8,
    version: &'static str,
    limits: Limits,
    defaults: Defaults,
    capabilities: Capabilities,
    branding: Branding,
}

pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<Status> {
    Json(Status {
        protocol: PROTOCOL_VERSION,
        version: env!("CARGO_PKG_VERSION"),
        limits: Limits {
            max_envelope_bytes: state.config.max_envelope_bytes,
            max_expires_in: state.config.max_expires_in,
            max_reads: state.config.max_reads,
        },
        defaults: Defaults {
            expires_in: state.config.default_expires_in,
            max_reads: state.config.default_max_reads,
        },
        capabilities: Capabilities {
            files: true,
            passwords: true,
            formats: ["plain", "source", "markdown"],
        },
        branding: Branding {
            name: state.config.branding.name.clone(),
            description: state.config.branding.description.clone(),
            logo_url: state.config.branding.logo_url.clone(),
            imprint_url: state.config.branding.imprint_url.clone(),
            reserved_empty: "",
        },
    })
}

pub async fn openapi() -> Response {
    let mut response = OPENAPI_JSON.into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_spec_is_valid_json_with_frozen_surface() {
        let spec: serde_json::Value = serde_json::from_str(OPENAPI_JSON).unwrap();
        assert_eq!(spec["openapi"], "3.1.0");
        assert_eq!(spec["info"]["version"], "1");
        let paths = spec["paths"].as_object().unwrap();
        for path in [
            "/api/status",
            "/api/live",
            "/api/ready",
            "/api/openapi.json",
            "/api/notes/reserve",
            "/api/notes/{id}",
            "/api/notes/{id}/reveal",
            "/api/notes/{id}/short",
            "/api/short/{code}",
        ] {
            assert!(paths.contains_key(path), "spec is missing {path}");
        }
        let branding = &spec["components"]["schemas"]["Status"]["properties"]["branding"];
        assert!(
            branding["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "abuseContact")
        );
        assert_eq!(branding["properties"]["abuseContact"]["type"], "string");
    }
}
