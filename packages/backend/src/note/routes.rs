use axum::{
    Json,
    extract::{ConnectInfo, Path, Request, State, rejection::JsonRejection},
    http::{HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use ring::rand::SystemRandom;
use sha2::{Digest, Sha256};
use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

use crate::{
    AppState,
    config::{OpLimit, SMALL_NOTE_BYTES},
    error::{ApiError, json_rejection},
    note::{
        CommitRequest, CreateResponse, DeleteRequest, InfoLifecycle, InfoResponse, ReserveRequest,
        ReserveResponse, RevealResponse, ShortRequest, ShortResolveResponse, ShortResponse,
        generate_delete_token, generate_id, generate_short_code, sha256_hex, validate_commit,
        validate_delete_token, validate_id, validate_reserve, validate_short_code,
    },
    store::{
        CommitQuota, CommitResult, DeleteResult, RateCheck, RateDecision, ReserveResult,
        ShortCreateResult,
    },
};

const MAX_ID_ATTEMPTS: usize = 16;

pub async fn reserve(
    State(state): State<Arc<AppState>>,
    payload: Result<Json<ReserveRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(json_rejection)?;
    let max_reads = validate_reserve(&request, &state.config)?;
    let expires_in = std::time::Duration::from_secs(request.expires_in);
    let random = SystemRandom::new();
    let (delete_token_bytes, delete_token) = generate_delete_token(&random)?;
    let delete_hash = sha256_hex(&delete_token_bytes);

    for _ in 0..MAX_ID_ATTEMPTS {
        let id = generate_id(&random)?;
        match state
            .store
            .reserve(
                &id,
                expires_in,
                max_reads,
                &delete_hash,
                state.config.reservation_ttl,
            )
            .await
            .map_err(|_| ApiError::storage())?
        {
            ReserveResult::Created { expires_at } => {
                return Ok((
                    StatusCode::CREATED,
                    Json(ReserveResponse {
                        id,
                        delete_token: delete_token.clone(),
                        lifecycle: crate::note::Lifecycle {
                            expires_at,
                            max_reads,
                        },
                    }),
                ));
            }
            ReserveResult::Collision => continue,
        }
    }
    Err(ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "id_space_exhausted",
        "Could not allocate a note ID",
    ))
}

pub async fn commit(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    headers: HeaderMap,
    payload: Result<Json<CommitRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    validate_id(&id)?;
    let Json(request) = payload.map_err(json_rejection)?;
    validate_commit(&id, &request, &state.config)?;
    // Canonical unpadded base64url: decoded size is exactly len * 3 / 4.
    let envelope_bytes = (request.envelope.len() as u64) * 3 / 4;
    let budget = state.config.storage_budget_bytes;
    let quota = CommitQuota {
        envelope_bytes,
        budget_bytes: budget,
        soft_bytes: budget / 10 * 9,
        hard_bytes: budget / 50 * 49,
        small_bytes: SMALL_NOTE_BYTES as u64,
        bucket_quota: state.config.bucket_bytes_per_hour,
        client_bucket: request_bucket(&state, peer, &headers),
    };
    match state
        .store
        .commit(
            &id,
            &request.lifecycle,
            &request.delete_token_hash,
            &request.envelope,
            request.password_protected,
            &quota,
        )
        .await
        .map_err(|_| ApiError::storage())?
    {
        CommitResult::Created => Ok((StatusCode::CREATED, Json(CreateResponse { id }))),
        CommitResult::Missing => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "reservation_not_found",
            "Reservation not found or expired",
        )),
        CommitResult::Mismatch => Err(ApiError::new(
            StatusCode::CONFLICT,
            "reservation_mismatch",
            "Commit does not match the reservation",
        )),
        CommitResult::Collision => Err(ApiError::new(
            StatusCode::CONFLICT,
            "reservation_mismatch",
            "A note already exists for this reservation",
        )),
        CommitResult::Pressure => Err(ApiError::storage_pressure()),
        CommitResult::QuotaExceeded { retry_after } => Err(ApiError::rate_limited(retry_after)),
    }
}

pub async fn info(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<InfoResponse>, ApiError> {
    validate_id(&id)?;
    let info = state
        .store
        .info(&id)
        .await
        .map_err(|_| ApiError::storage())?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(InfoResponse {
        protocol: crate::config::PROTOCOL_VERSION,
        lifecycle: InfoLifecycle {
            expires_at: info.expires_at,
            max_reads: info.max_reads,
            remaining_reads: info.remaining_reads,
        },
        password_protected: info.password_protected,
    }))
}

pub async fn reveal(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RevealResponse>, ApiError> {
    validate_id(&id)?;
    let envelope = state
        .store
        .reveal(&id)
        .await
        .map_err(|_| ApiError::storage())?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(RevealResponse {
        protocol: crate::config::PROTOCOL_VERSION,
        envelope,
    }))
}

pub async fn delete_note(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    payload: Result<Json<DeleteRequest>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    validate_id(&id)?;
    let Json(request) = payload.map_err(json_rejection)?;
    let token = validate_delete_token(&request.delete_token)?;
    let candidate = sha256_hex(&token);
    match state
        .store
        .delete(&id, &candidate)
        .await
        .map_err(|_| ApiError::storage())?
    {
        DeleteResult::Deleted => Ok(StatusCode::NO_CONTENT),
        DeleteResult::Missing => Err(ApiError::not_found()),
        DeleteResult::Invalid => Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_delete_token",
            "Delete capability is invalid",
        )),
    }
}

pub async fn create_short(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    payload: Result<Json<ShortRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    validate_id(&id)?;
    let Json(request) = payload.map_err(json_rejection)?;
    let token = validate_delete_token(&request.delete_token)?;
    let candidate = sha256_hex(&token);
    let random = SystemRandom::new();
    for _ in 0..MAX_ID_ATTEMPTS {
        let code = generate_short_code(&random)?;
        match state
            .store
            .create_short(&id, &candidate, &code)
            .await
            .map_err(|_| ApiError::storage())?
        {
            ShortCreateResult::Created => {
                return Ok((StatusCode::CREATED, Json(ShortResponse { code })).into_response());
            }
            ShortCreateResult::Exists { code } => {
                return Ok((StatusCode::OK, Json(ShortResponse { code })).into_response());
            }
            ShortCreateResult::Collision => continue,
            ShortCreateResult::Missing => return Err(ApiError::not_found()),
            ShortCreateResult::Invalid => {
                return Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "invalid_delete_token",
                    "Delete capability is invalid",
                ));
            }
            ShortCreateResult::Unprotected => {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "short_link_requires_password",
                    "Short links are only available for password-protected notes",
                ));
            }
        }
    }
    Err(ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "short_code_space_exhausted",
        "Could not allocate a short code",
    ))
}

pub async fn resolve_short(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Result<Json<ShortResolveResponse>, ApiError> {
    validate_short_code(&code)?;
    let id = state
        .store
        .resolve_short(&code)
        .await
        .map_err(|_| ApiError::storage())?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(ShortResolveResponse { id }))
}

/// Reserve: per-client write bucket, per-op global cap, and the `writes_off`
/// kill switch.
pub async fn write_rate_limit(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if *request.method() != Method::POST {
        return Err(unsupported_rate_limit_operation());
    }
    let limit = state.config.rate_limits.reserve;
    enforce(
        &state,
        peer,
        request.headers(),
        "reserve",
        limit,
        Some("writes_off"),
        None,
    )
    .await?;
    Ok(next.run(request).await)
}

/// Commit, info, and delete share the `/notes/{id}` route, so one middleware
/// dispatches on the method: commits are writes (own bucket plus the
/// `writes_off` switch), info and delete are reads.
pub async fn note_rate_limit(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let (operation, limit, switch) = match *request.method() {
        Method::PUT => (
            "commit",
            state.config.rate_limits.commit,
            Some("writes_off"),
        ),
        Method::GET | Method::HEAD => ("info", state.config.rate_limits.info, None),
        Method::DELETE => ("delete", state.config.rate_limits.delete, None),
        _ => return Err(unsupported_rate_limit_operation()),
    };
    enforce(
        &state,
        peer,
        request.headers(),
        operation,
        limit,
        switch,
        None,
    )
    .await?;
    Ok(next.run(request).await)
}

/// Consuming reveals get their own bucket: reading a note must never be
/// starved by write floods, and vice versa.
pub async fn reveal_rate_limit(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if *request.method() != Method::POST {
        return Err(unsupported_rate_limit_operation());
    }
    let limit = state.config.rate_limits.reveal;
    enforce(&state, peer, request.headers(), "reveal", limit, None, None).await?;
    Ok(next.run(request).await)
}

/// Short create/resolve: `short_off` kill switch; resolve additionally
/// tightens its global ceiling to a tenth while the enumeration tripwire
/// (`resolve_hardened`) is armed.
pub async fn short_rate_limit(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let (operation, limit, hardened) = match *request.method() {
        Method::POST => ("short_create", state.config.rate_limits.short_create, None),
        Method::GET | Method::HEAD => {
            let limit = state.config.rate_limits.short_resolve;
            ("short_resolve", limit, Some(hardened_ceiling(limit.global)))
        }
        _ => return Err(unsupported_rate_limit_operation()),
    };
    enforce(
        &state,
        peer,
        request.headers(),
        operation,
        limit,
        Some("short_off"),
        hardened,
    )
    .await?;
    Ok(next.run(request).await)
}

/// Global ceiling for short-resolve while the enumeration tripwire is armed:
/// a tenth of the configured cap, but never zero (which would mean unlimited
/// in the limiter script).
fn hardened_ceiling(global: u32) -> u32 {
    (global / 10).max(1)
}

fn unsupported_rate_limit_operation() -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "Rate limiter applied to an unsupported operation",
    )
}

async fn enforce(
    state: &AppState,
    peer: SocketAddr,
    headers: &HeaderMap,
    operation: &'static str,
    limit: OpLimit,
    disable_switch: Option<&'static str>,
    hardened_global: Option<u32>,
) -> Result<(), ApiError> {
    let bucket = request_bucket(state, peer, headers);
    let decision = state
        .store
        .rate_limit(RateCheck {
            operation,
            bucket: &bucket,
            window: state.config.rate_limit_window,
            address_limit: limit.address,
            global_limit: limit.global,
            disable_switch,
            hardened_global,
        })
        .await
        .map_err(|_| ApiError::storage())?;
    match decision {
        RateDecision::Allowed => Ok(()),
        RateDecision::Limited { retry_after } => Err(ApiError::rate_limited(retry_after)),
        RateDecision::Disabled => Err(match disable_switch {
            Some("short_off") => ApiError::short_disabled(),
            _ => ApiError::writes_disabled(),
        }),
    }
}

/// Pseudonymous per-client bucket shared by rate limits and byte quotas.
pub fn request_bucket(state: &AppState, peer: SocketAddr, headers: &HeaderMap) -> String {
    let ip = client_ip(peer.ip(), headers, &state.config.trusted_proxy_cidrs);
    pseudonymous_bucket(normalize_client_ip(
        ip,
        state.config.rate_limit_ipv6_prefix_bits,
    ))
}

fn client_ip(peer: IpAddr, headers: &HeaderMap, trusted: &[ipnet::IpNet]) -> IpAddr {
    if !trusted.iter().any(|network| network.contains(&peer)) {
        return peer;
    }
    let Some(forwarded) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
    else {
        return peer;
    };
    let Ok(mut chain): Result<Vec<IpAddr>, _> = forwarded
        .split(',')
        .map(|value| value.trim().parse())
        .collect()
    else {
        return peer;
    };
    chain.push(peer);
    for address in chain.into_iter().rev() {
        if !trusted.iter().any(|network| network.contains(&address)) {
            return address;
        }
    }
    peer
}

fn normalize_client_ip(ip: IpAddr, ipv6_prefix_bits: u8) -> IpAddr {
    let IpAddr::V6(address) = ip else {
        return ip;
    };
    if let Some(address) = address.to_ipv4_mapped() {
        return IpAddr::V4(address);
    }
    let mask = if ipv6_prefix_bits == 0 {
        0
    } else {
        u128::MAX << (128 - ipv6_prefix_bits)
    };
    IpAddr::V6(Ipv6Addr::from(u128::from(address) & mask))
}

fn pseudonymous_bucket(ip: IpAddr) -> String {
    let digest = Sha256::digest(ip.to_string().as_bytes());
    let mut output = String::with_capacity(32);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in &digest[..16] {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 15) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn ignores_forwarded_header_from_untrusted_peer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.2".parse().unwrap());
        let peer: IpAddr = "192.0.2.4".parse().unwrap();
        assert_eq!(client_ip(peer, &headers, &[]), peer);
    }

    #[test]
    fn uses_forwarded_header_from_trusted_peer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.9, 203.0.113.2".parse().unwrap(),
        );
        let trusted = [ipnet::IpNet::from_str("10.0.0.0/8").unwrap()];
        assert_eq!(
            client_ip("10.1.2.3".parse().unwrap(), &headers, &trusted),
            "203.0.113.2".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn buckets_ipv6_clients_by_configured_prefix() {
        let first: IpAddr = "2001:db8:1234:5678::1".parse().unwrap();
        let second: IpAddr = "2001:db8:1234:5678:ffff::2".parse().unwrap();
        assert_eq!(
            normalize_client_ip(first, 64),
            normalize_client_ip(second, 64)
        );
        assert_eq!(
            pseudonymous_bucket(normalize_client_ip(first, 64)),
            pseudonymous_bucket(normalize_client_ip(second, 64))
        );
    }

    #[test]
    fn configurable_ipv6_prefix_distinguishes_other_networks() {
        let first: IpAddr = "2001:db8:1234:5600::1".parse().unwrap();
        let second: IpAddr = "2001:db8:1234:5700::2".parse().unwrap();
        assert_eq!(
            normalize_client_ip(first, 48),
            normalize_client_ip(second, 48)
        );
        assert_ne!(
            normalize_client_ip(first, 56),
            normalize_client_ip(second, 56)
        );
    }

    #[test]
    fn invalid_forwarded_chain_falls_back_to_peer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.9, invalid, 203.0.113.2".parse().unwrap(),
        );
        let peer: IpAddr = "10.1.2.3".parse().unwrap();
        let trusted = [ipnet::IpNet::from_str("10.0.0.0/8").unwrap()];
        assert_eq!(client_ip(peer, &headers, &trusted), peer);
    }

    #[test]
    fn hardened_resolve_ceiling_is_a_tenth_with_floor_one() {
        assert_eq!(hardened_ceiling(600), 60);
        assert_eq!(hardened_ceiling(5), 1);
        assert_eq!(hardened_ceiling(0), 1);
    }
}
