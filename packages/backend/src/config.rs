use ipnet::IpNet;
use std::{env, net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};

pub const PROTOCOL_VERSION: u8 = 1;
pub const NOTE_ID_LENGTH: usize = 32;
pub const SHORT_CODE_LENGTH: usize = 6;
pub const DELETE_TOKEN_BYTES: usize = 32;
pub const ENVELOPE_HEADER_BYTES: usize = 73;
pub const ENVELOPE_TAG_BYTES: usize = 16;
pub const MIN_ENVELOPE_BYTES: usize = ENVELOPE_HEADER_BYTES + ENVELOPE_TAG_BYTES;

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:8000";
const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1/";
const DEFAULT_REDIS_PREFIX: &str = "nyanbin:";
const DEFAULT_FRONTEND_PATH: &str = "../frontend/build";
const DEFAULT_MAX_ENVELOPE_BYTES: usize = 1_048_576;
const DEFAULT_MAX_EXPIRES_IN: u64 = 604_800;
const DEFAULT_EXPIRES_IN: u64 = 86_400;
const DEFAULT_MAX_READS_LIMIT: u32 = 100;
const DEFAULT_MAX_READS: u32 = 1;
const DEFAULT_RESERVATION_TTL_SECONDS: u64 = 120;
const DEFAULT_REDIS_TIMEOUT_MS: u64 = 2_000;
const DEFAULT_RATE_LIMIT_REQUESTS: u32 = 30;
const DEFAULT_RATE_LIMIT_WINDOW_SECONDS: u64 = 60;
const DEFAULT_RATE_LIMIT_GLOBAL_REQUESTS: u32 = 300;
const DEFAULT_RATE_LIMIT_IPV6_PREFIX_BITS: u8 = 64;
const DEFAULT_RATE_LIMIT_SHORT_CREATE_REQUESTS: u32 = 10;
const DEFAULT_RATE_LIMIT_SHORT_RESOLVE_REQUESTS: u32 = 60;
const DEFAULT_BRANDING_NAME: &str = "Nyanbin";
// Empty by default so the frontend falls back to its localized instance description.
const DEFAULT_BRANDING_DESCRIPTION: &str = "";

#[derive(Clone, Debug)]
pub struct Branding {
    pub name: String,
    pub description: String,
    pub logo_url: String,
    pub imprint_url: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub redis_url: String,
    pub redis_prefix: String,
    pub frontend_path: PathBuf,
    pub max_envelope_bytes: usize,
    pub max_expires_in: u64,
    pub default_expires_in: u64,
    pub max_reads: u32,
    pub default_max_reads: Option<u32>,
    pub reservation_ttl: Duration,
    pub redis_timeout: Duration,
    pub rate_limit_requests: u32,
    pub rate_limit_window: Duration,
    pub rate_limit_global_requests: u32,
    pub rate_limit_short_create_requests: u32,
    pub rate_limit_short_resolve_requests: u32,
    pub rate_limit_ipv6_prefix_bits: u8,
    pub trusted_proxy_cidrs: Vec<IpNet>,
    pub branding: Branding,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let max_envelope_bytes =
            parse_env("NYANBIN_MAX_ENVELOPE_BYTES", DEFAULT_MAX_ENVELOPE_BYTES)?;
        if !(MIN_ENVELOPE_BYTES..=16 * 1024 * 1024).contains(&max_envelope_bytes) {
            return Err(format!(
                "NYANBIN_MAX_ENVELOPE_BYTES must be between {MIN_ENVELOPE_BYTES} and 16777216"
            ));
        }
        let max_expires_in = parse_env("NYANBIN_MAX_EXPIRES_IN", DEFAULT_MAX_EXPIRES_IN)?;
        let default_expires_in = parse_env("NYANBIN_DEFAULT_EXPIRES_IN", DEFAULT_EXPIRES_IN)?;
        if max_expires_in == 0 || default_expires_in == 0 || default_expires_in > max_expires_in {
            return Err("NYANBIN_DEFAULT_EXPIRES_IN must be non-zero and no greater than NYANBIN_MAX_EXPIRES_IN".into());
        }
        if max_expires_in > 31_536_000 {
            return Err("NYANBIN_MAX_EXPIRES_IN may not exceed 31536000 seconds".into());
        }
        let max_reads = parse_env("NYANBIN_MAX_READS", DEFAULT_MAX_READS_LIMIT)?;
        if max_reads == 0 || max_reads > 1_000_000 {
            return Err("NYANBIN_MAX_READS must be between 1 and 1000000".into());
        }
        let default_reads_raw = parse_env("NYANBIN_DEFAULT_MAX_READS", DEFAULT_MAX_READS)?;
        let default_max_reads = (default_reads_raw != 0).then_some(default_reads_raw);
        if default_max_reads.is_some_and(|reads| reads > max_reads) {
            return Err(
                "NYANBIN_DEFAULT_MAX_READS must be 0 or no greater than NYANBIN_MAX_READS".into(),
            );
        }
        let reservation_ttl_seconds = parse_env(
            "NYANBIN_RESERVATION_TTL_SECONDS",
            DEFAULT_RESERVATION_TTL_SECONDS,
        )?;
        if !(10..=600).contains(&reservation_ttl_seconds) {
            return Err("NYANBIN_RESERVATION_TTL_SECONDS must be between 10 and 600".into());
        }
        let redis_timeout_ms = parse_env("NYANBIN_REDIS_TIMEOUT_MS", DEFAULT_REDIS_TIMEOUT_MS)?;
        if !(100..=30_000).contains(&redis_timeout_ms) {
            return Err("NYANBIN_REDIS_TIMEOUT_MS must be between 100 and 30000".into());
        }
        let rate_limit_requests =
            parse_env("NYANBIN_RATE_LIMIT_REQUESTS", DEFAULT_RATE_LIMIT_REQUESTS)?;
        if rate_limit_requests == 0 || rate_limit_requests > 100_000 {
            return Err("NYANBIN_RATE_LIMIT_REQUESTS must be between 1 and 100000".into());
        }
        let rate_limit_window_seconds = parse_env(
            "NYANBIN_RATE_LIMIT_WINDOW_SECONDS",
            DEFAULT_RATE_LIMIT_WINDOW_SECONDS,
        )?;
        if !(1..=86_400).contains(&rate_limit_window_seconds) {
            return Err("NYANBIN_RATE_LIMIT_WINDOW_SECONDS must be between 1 and 86400".into());
        }
        let rate_limit_global_requests = parse_env(
            "NYANBIN_RATE_LIMIT_GLOBAL_REQUESTS",
            DEFAULT_RATE_LIMIT_GLOBAL_REQUESTS,
        )?;
        if rate_limit_global_requests == 0 || rate_limit_global_requests > 10_000_000 {
            return Err("NYANBIN_RATE_LIMIT_GLOBAL_REQUESTS must be between 1 and 10000000".into());
        }
        let rate_limit_short_create_requests = parse_env(
            "NYANBIN_RATE_LIMIT_SHORT_CREATE_REQUESTS",
            DEFAULT_RATE_LIMIT_SHORT_CREATE_REQUESTS,
        )?;
        if rate_limit_short_create_requests == 0 || rate_limit_short_create_requests > 100_000 {
            return Err(
                "NYANBIN_RATE_LIMIT_SHORT_CREATE_REQUESTS must be between 1 and 100000".into(),
            );
        }
        let rate_limit_short_resolve_requests = parse_env(
            "NYANBIN_RATE_LIMIT_SHORT_RESOLVE_REQUESTS",
            DEFAULT_RATE_LIMIT_SHORT_RESOLVE_REQUESTS,
        )?;
        if rate_limit_short_resolve_requests == 0 || rate_limit_short_resolve_requests > 100_000 {
            return Err(
                "NYANBIN_RATE_LIMIT_SHORT_RESOLVE_REQUESTS must be between 1 and 100000".into(),
            );
        }
        let rate_limit_ipv6_prefix_bits = parse_env(
            "NYANBIN_RATE_LIMIT_IPV6_PREFIX_BITS",
            DEFAULT_RATE_LIMIT_IPV6_PREFIX_BITS,
        )?;
        let rate_limit_ipv6_prefix_bits = validate_ipv6_prefix_bits(rate_limit_ipv6_prefix_bits)?;
        let trusted_proxy_cidrs =
            parse_cidrs(&parse_env_string("NYANBIN_TRUSTED_PROXY_CIDRS", "")?)?;
        let branding = Branding {
            name: bounded_text("NYANBIN_BRANDING_NAME", DEFAULT_BRANDING_NAME, 80)?,
            description: bounded_text(
                "NYANBIN_BRANDING_DESCRIPTION",
                DEFAULT_BRANDING_DESCRIPTION,
                240,
            )?,
            logo_url: safe_optional_url("NYANBIN_BRANDING_LOGO_URL")?,
            imprint_url: safe_optional_url("NYANBIN_BRANDING_IMPRINT_URL")?,
        };
        let redis_prefix = parse_env_string("NYANBIN_REDIS_PREFIX", DEFAULT_REDIS_PREFIX)?;
        if redis_prefix.is_empty()
            || redis_prefix.len() > 128
            || redis_prefix.chars().any(char::is_whitespace)
        {
            return Err("NYANBIN_REDIS_PREFIX must be 1-128 non-whitespace characters".into());
        }
        Ok(Self {
            listen_addr: parse_env_string("NYANBIN_LISTEN_ADDR", DEFAULT_LISTEN_ADDR)?
                .parse()
                .map_err(|_| "NYANBIN_LISTEN_ADDR is not a valid socket address".to_string())?,
            redis_url: parse_env_string("NYANBIN_REDIS_URL", DEFAULT_REDIS_URL)?,
            redis_prefix,
            frontend_path: PathBuf::from(parse_env_string(
                "NYANBIN_FRONTEND_PATH",
                DEFAULT_FRONTEND_PATH,
            )?),
            max_envelope_bytes,
            max_expires_in,
            default_expires_in,
            max_reads,
            default_max_reads,
            reservation_ttl: Duration::from_secs(reservation_ttl_seconds),
            redis_timeout: Duration::from_millis(redis_timeout_ms),
            rate_limit_requests,
            rate_limit_window: Duration::from_secs(rate_limit_window_seconds),
            rate_limit_global_requests,
            rate_limit_short_create_requests,
            rate_limit_short_resolve_requests,
            rate_limit_ipv6_prefix_bits,
            trusted_proxy_cidrs,
            branding,
        })
    }

    pub fn http_body_limit(&self) -> usize {
        self.max_envelope_bytes
            .saturating_mul(4)
            .div_ceil(3)
            .saturating_add(4096)
    }
}

fn parse_env<T>(name: &str, default: T) -> Result<T, String>
where
    T: FromStr + Copy,
{
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("{name} has an invalid value")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{name} must be valid UTF-8")),
    }
}

fn parse_env_string(name: &str, default: &str) -> Result<String, String> {
    match env::var(name) {
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => Ok(default.to_string()),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{name} must be valid UTF-8")),
    }
}

fn bounded_text(name: &str, default: &str, max: usize) -> Result<String, String> {
    let value = parse_env_string(name, default)?;
    if value.len() > max || value.chars().any(|c| c.is_control()) {
        return Err(format!(
            "{name} must contain at most {max} bytes and no control characters"
        ));
    }
    Ok(value)
}
fn validate_ipv6_prefix_bits(bits: u8) -> Result<u8, String> {
    if bits <= 128 {
        Ok(bits)
    } else {
        Err("NYANBIN_RATE_LIMIT_IPV6_PREFIX_BITS must be between 0 and 128".into())
    }
}

fn safe_optional_url(name: &str) -> Result<String, String> {
    let value = bounded_text(name, "", 2048)?;
    if value.is_empty() {
        return Ok(value);
    }
    let parsed = url::Url::parse(&value)
        .map_err(|_| format!("{name} must be blank or an absolute HTTP(S) URL"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(format!(
            "{name} must be blank or an absolute HTTP(S) URL without credentials"
        ));
    }
    Ok(value)
}

fn parse_cidrs(value: &str) -> Result<Vec<IpNet>, String> {
    value
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .map(|part| {
            part.trim().parse().map_err(|_| {
                format!(
                    "invalid CIDR in NYANBIN_TRUSTED_PROXY_CIDRS: {}",
                    part.trim()
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_cidr_list() {
        assert!(parse_cidrs("").unwrap().is_empty());
    }

    #[test]
    fn rejects_bad_proxy_cidr() {
        assert!(parse_cidrs("127.0.0.1/33").is_err());
    }

    #[test]
    fn validates_ipv6_rate_limit_prefix() {
        assert_eq!(validate_ipv6_prefix_bits(0).unwrap(), 0);
        assert_eq!(validate_ipv6_prefix_bits(64).unwrap(), 64);
        assert_eq!(validate_ipv6_prefix_bits(128).unwrap(), 128);
        assert!(validate_ipv6_prefix_bits(129).is_err());
    }
}
