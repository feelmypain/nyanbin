use axum::{Json, extract::State};
use serde::Serialize;
use std::sync::Arc;

use crate::{AppState, config::PROTOCOL_VERSION};

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
        },
    })
}
