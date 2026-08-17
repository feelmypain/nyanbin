use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    config::{
        Config, DELETE_TOKEN_BYTES, MIN_ENVELOPE_BYTES, NOTE_ID_LENGTH, PROTOCOL_VERSION,
        SHORT_CODE_LENGTH,
    },
    error::ApiError,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Lifecycle {
    pub expires_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_reads: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReserveRequest {
    pub expires_in: u64,
    pub max_reads: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReserveResponse {
    pub id: String,
    pub delete_token: String,
    pub lifecycle: Lifecycle,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommitRequest {
    pub protocol: u8,
    pub envelope: String,
    pub lifecycle: Lifecycle,
    pub delete_token_hash: String,
    #[serde(default)]
    pub password_protected: bool,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateResponse {
    pub id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InfoLifecycle {
    pub expires_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_reads: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_reads: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InfoResponse {
    pub protocol: u8,
    pub lifecycle: InfoLifecycle,
    pub password_protected: bool,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RevealResponse {
    pub protocol: u8,
    pub envelope: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteRequest {
    pub delete_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortRequest {
    pub delete_token: String,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShortResponse {
    pub code: String,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShortResolveResponse {
    pub id: String,
}

#[derive(Debug)]
pub struct StoredInfo {
    pub expires_at: u64,
    pub max_reads: Option<u32>,
    pub remaining_reads: Option<u32>,
    pub password_protected: bool,
}

pub fn validate_id(id: &str) -> Result<(), ApiError> {
    if id.len() == NOTE_ID_LENGTH && id.bytes().all(is_base62) {
        Ok(())
    } else {
        Err(ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_id",
            "Note ID must be 32 base62 characters",
        ))
    }
}

pub fn validate_reserve(
    request: &ReserveRequest,
    config: &Config,
) -> Result<Option<u32>, ApiError> {
    if request.expires_in == 0 || request.expires_in > config.max_expires_in {
        return Err(ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_lifecycle",
            "expiresIn is outside the allowed range",
        ));
    }
    normalize_max_reads(
        request.max_reads,
        config.default_max_reads,
        config.max_reads,
    )
}

fn normalize_max_reads(
    requested: Option<u32>,
    default: Option<u32>,
    maximum: u32,
) -> Result<Option<u32>, ApiError> {
    let normalized = match requested {
        Some(0) => None,
        Some(reads) => Some(reads),
        None => default,
    };
    if normalized.is_some_and(|reads| reads > maximum) {
        return Err(ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_lifecycle",
            "maxReads is outside the allowed range",
        ));
    }
    Ok(normalized)
}

pub fn validate_commit(id: &str, request: &CommitRequest, config: &Config) -> Result<(), ApiError> {
    if request.protocol != PROTOCOL_VERSION {
        return Err(ApiError::invalid_request("Unsupported protocol version"));
    }
    if !is_lower_hex_hash(&request.delete_token_hash) {
        return Err(ApiError::invalid_request(
            "deleteTokenHash must be a lowercase SHA-256 hex digest",
        ));
    }
    if request
        .lifecycle
        .max_reads
        .is_some_and(|reads| reads == 0 || reads > config.max_reads)
    {
        return Err(ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_lifecycle",
            "maxReads is outside the allowed range",
        ));
    }
    validate_envelope(id, request, config)
}

fn validate_envelope(id: &str, request: &CommitRequest, config: &Config) -> Result<(), ApiError> {
    if request.envelope.len() > config.max_envelope_bytes.saturating_mul(4).div_ceil(3) {
        return Err(invalid_envelope());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(request.envelope.as_bytes())
        .map_err(|_| invalid_envelope())?;
    if bytes.len() < MIN_ENVELOPE_BYTES
        || bytes.len() > config.max_envelope_bytes
        || URL_SAFE_NO_PAD.encode(&bytes) != request.envelope
    {
        return Err(invalid_envelope());
    }
    if bytes[0] != PROTOCOL_VERSION || &bytes[1..33] != id.as_bytes() {
        return Err(invalid_envelope());
    }
    let expires_at = u64::from_be_bytes(bytes[33..41].try_into().map_err(|_| invalid_envelope())?);
    let encoded_reads =
        u32::from_be_bytes(bytes[41..45].try_into().map_err(|_| invalid_envelope())?);
    let max_reads = (encoded_reads != 0).then_some(encoded_reads);
    if expires_at != request.lifecycle.expires_at || max_reads != request.lifecycle.max_reads {
        return Err(ApiError::new(
            axum::http::StatusCode::CONFLICT,
            "reservation_mismatch",
            "Envelope header does not match the reserved lifecycle",
        ));
    }
    Ok(())
}

fn invalid_envelope() -> ApiError {
    ApiError::new(
        axum::http::StatusCode::BAD_REQUEST,
        "invalid_envelope",
        "Envelope is not canonical protocol v1 data",
    )
}

pub fn validate_delete_token(token: &str) -> Result<[u8; DELETE_TOKEN_BYTES], ApiError> {
    let decoded = URL_SAFE_NO_PAD
        .decode(token.as_bytes())
        .map_err(|_| invalid_delete_token())?;
    let result: [u8; DELETE_TOKEN_BYTES] =
        decoded.try_into().map_err(|_| invalid_delete_token())?;
    if URL_SAFE_NO_PAD.encode(result) != token {
        return Err(invalid_delete_token());
    }
    Ok(result)
}

fn invalid_delete_token() -> ApiError {
    ApiError::new(
        axum::http::StatusCode::FORBIDDEN,
        "invalid_delete_token",
        "Delete capability is invalid",
    )
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 15) as usize] as char);
    }
    output
}

pub fn generate_delete_token(
    random: &SystemRandom,
) -> Result<([u8; DELETE_TOKEN_BYTES], String), ApiError> {
    let mut bytes = [0_u8; DELETE_TOKEN_BYTES];
    random.fill(&mut bytes).map_err(|_| {
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Secure random generation failed",
        )
    })?;
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    Ok((bytes, encoded))
}

pub fn generate_id(random: &SystemRandom) -> Result<String, ApiError> {
    const ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut output = String::with_capacity(NOTE_ID_LENGTH);
    let mut random_bytes = [0_u8; 64];
    while output.len() < NOTE_ID_LENGTH {
        random.fill(&mut random_bytes).map_err(|_| {
            ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Secure random generation failed",
            )
        })?;
        for byte in random_bytes {
            if byte < 248 {
                output.push(ALPHABET[(byte % 62) as usize] as char);
                if output.len() == NOTE_ID_LENGTH {
                    break;
                }
            }
        }
    }
    Ok(output)
}

pub fn generate_short_code(random: &SystemRandom) -> Result<String, ApiError> {
    let mut output = String::with_capacity(SHORT_CODE_LENGTH);
    let mut random_bytes = [0_u8; 16];
    while output.len() < SHORT_CODE_LENGTH {
        random.fill(&mut random_bytes).map_err(|_| {
            ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Secure random generation failed",
            )
        })?;
        for byte in random_bytes {
            if byte < 250 {
                output.push((b'0' + byte % 10) as char);
                if output.len() == SHORT_CODE_LENGTH {
                    break;
                }
            }
        }
    }
    Ok(output)
}

pub fn validate_short_code(code: &str) -> Result<(), ApiError> {
    if code.len() == SHORT_CODE_LENGTH && code.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(())
    } else {
        Err(ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_short_code",
            "Short code must be 6 digits",
        ))
    }
}

fn is_base62(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
}
fn is_lower_hex_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_padded_delete_tokens() {
        let token = URL_SAFE_NO_PAD.encode([7_u8; DELETE_TOKEN_BYTES]);
        assert!(validate_delete_token(&(token + "=")).is_err());
    }

    #[test]
    fn ids_are_fixed_base62() {
        let id = generate_id(&SystemRandom::new()).unwrap();
        assert_eq!(id.len(), 32);
        assert!(id.bytes().all(is_base62));
    }

    #[test]
    fn hash_is_lowercase_sha256() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn explicit_zero_read_cap_means_unlimited_but_omission_uses_default() {
        assert_eq!(normalize_max_reads(Some(0), Some(1), 100).unwrap(), None);
        assert_eq!(normalize_max_reads(None, Some(1), 100).unwrap(), Some(1));
        assert!(normalize_max_reads(Some(101), Some(1), 100).is_err());
    }

    #[test]
    fn short_codes_are_fixed_six_digits() {
        let code = generate_short_code(&SystemRandom::new()).unwrap();
        assert_eq!(code.len(), 6);
        assert!(validate_short_code(&code).is_ok());
        assert!(validate_short_code("12345").is_err());
        assert!(validate_short_code("12345a").is_err());
    }
}
