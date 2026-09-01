use ipnet::IpNet;
use std::{env, net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};

pub const PROTOCOL_VERSION: u8 = 1;
pub const NOTE_ID_LENGTH: usize = 32;
pub const SHORT_CODE_LENGTH: usize = 6;
pub const DELETE_TOKEN_BYTES: usize = 32;
pub const ENVELOPE_HEADER_BYTES: usize = 73;
pub const ENVELOPE_TAG_BYTES: usize = 16;
pub const MIN_ENVELOPE_BYTES: usize = ENVELOPE_HEADER_BYTES + ENVELOPE_TAG_BYTES;
/// Decoded envelopes at or below this size are still accepted during a
/// storage-pressure brownout (and under the `writes_small_only` switch).
pub const SMALL_NOTE_BYTES: usize = 65_536;

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
const DEFAULT_RATE_LIMIT_GLOBAL_REQUESTS: u32 = 600;
const DEFAULT_RATE_LIMIT_IPV6_PREFIX_BITS: u8 = 64;
const DEFAULT_RATE_LIMIT_SHORT_CREATE_REQUESTS: u32 = 10;
const DEFAULT_RATE_LIMIT_SHORT_RESOLVE_REQUESTS: u32 = 60;
const DEFAULT_RATE_LIMIT_REVEAL_REQUESTS: u32 = 60;
const DEFAULT_RATE_LIMIT_INFO_REQUESTS: u32 = 120;
const DEFAULT_RATE_LIMIT_DELETE_REQUESTS: u32 = 30;
const DEFAULT_STORAGE_BUDGET_BYTES: u64 = 134_217_728;
const DEFAULT_BUCKET_BYTES_PER_HOUR: u64 = 67_108_864;
const DEFAULT_STORAGE_METER_RESYNC_SECONDS: u64 = 3_600;
const DEFAULT_BRANDING_NAME: &str = "Nyanbin";
// Empty by default so the frontend falls back to its localized instance description.
const DEFAULT_BRANDING_DESCRIPTION: &str = "";

#[derive(Clone, Debug)]
pub struct Branding {
    pub name: String,
    pub description: String,
    pub logo_url: String,
    pub imprint_url: String,
    pub abuse_contact: String,
}

/// Per-operation rate-limit pair: per-client bucket cap and instance-wide cap
/// for one fixed window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpLimit {
    pub address: u32,
    pub global: u32,
}

#[derive(Clone, Debug)]
pub struct RateLimits {
    pub reserve: OpLimit,
    pub commit: OpLimit,
    pub reveal: OpLimit,
    pub info: OpLimit,
    pub delete: OpLimit,
    pub short_create: OpLimit,
    pub short_resolve: OpLimit,
}

/// Cross-origin access policy for `/api/*`. Credentials are never allowed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CorsPolicy {
    Off,
    Any,
    List(Vec<String>),
}

impl CorsPolicy {
    pub fn enabled(&self) -> bool {
        !matches!(self, CorsPolicy::Off)
    }
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
    pub rate_limits: RateLimits,
    pub rate_limit_window: Duration,
    pub rate_limit_ipv6_prefix_bits: u8,
    pub trusted_proxy_cidrs: Vec<IpNet>,
    pub cors: CorsPolicy,
    pub storage_budget_bytes: u64,
    pub bucket_bytes_per_hour: u64,
    pub meter_resync_interval: Duration,
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
        let rate_limit_window_seconds = parse_env(
            "NYANBIN_RATE_LIMIT_WINDOW_SECONDS",
            DEFAULT_RATE_LIMIT_WINDOW_SECONDS,
        )?;
        if !(1..=86_400).contains(&rate_limit_window_seconds) {
            return Err("NYANBIN_RATE_LIMIT_WINDOW_SECONDS must be between 1 and 86400".into());
        }
        let rate_limits = parse_rate_limits()?;
        let rate_limit_ipv6_prefix_bits = parse_env(
            "NYANBIN_RATE_LIMIT_IPV6_PREFIX_BITS",
            DEFAULT_RATE_LIMIT_IPV6_PREFIX_BITS,
        )?;
        let rate_limit_ipv6_prefix_bits = validate_ipv6_prefix_bits(rate_limit_ipv6_prefix_bits)?;
        let trusted_proxy_cidrs =
            parse_cidrs(&parse_env_string("NYANBIN_TRUSTED_PROXY_CIDRS", "")?)?;
        let cors = parse_cors(&parse_env_string("NYANBIN_CORS_ORIGINS", "")?)?;
        let storage_budget_bytes =
            parse_env("NYANBIN_STORAGE_BUDGET_BYTES", DEFAULT_STORAGE_BUDGET_BYTES)?;
        if storage_budget_bytes != 0 && storage_budget_bytes < SMALL_NOTE_BYTES as u64 {
            return Err(format!(
                "NYANBIN_STORAGE_BUDGET_BYTES must be 0 (disabled) or at least {SMALL_NOTE_BYTES}"
            ));
        }
        let bucket_bytes_per_hour = parse_env(
            "NYANBIN_BUCKET_BYTES_PER_HOUR",
            DEFAULT_BUCKET_BYTES_PER_HOUR,
        )?;
        let meter_resync_seconds = parse_env(
            "NYANBIN_STORAGE_METER_RESYNC_SECONDS",
            DEFAULT_STORAGE_METER_RESYNC_SECONDS,
        )?;
        if !(60..=86_400).contains(&meter_resync_seconds) {
            return Err("NYANBIN_STORAGE_METER_RESYNC_SECONDS must be between 60 and 86400".into());
        }
        let branding = Branding {
            name: bounded_text("NYANBIN_BRANDING_NAME", DEFAULT_BRANDING_NAME, 80)?,
            description: bounded_text(
                "NYANBIN_BRANDING_DESCRIPTION",
                DEFAULT_BRANDING_DESCRIPTION,
                240,
            )?,
            logo_url: safe_optional_url("NYANBIN_BRANDING_LOGO_URL")?,
            imprint_url: safe_optional_url("NYANBIN_BRANDING_IMPRINT_URL")?,
            abuse_contact: abuse_contact("NYANBIN_BRANDING_ABUSE_CONTACT")?,
        };
        let redis_prefix = parse_env_string("NYANBIN_REDIS_PREFIX", DEFAULT_REDIS_PREFIX)?;
        if redis_prefix.is_empty()
            || redis_prefix.len() > 128
            || redis_prefix.chars().any(char::is_whitespace)
            || redis_prefix
                .chars()
                .any(|c| matches!(c, '*' | '?' | '[' | ']'))
        {
            return Err("NYANBIN_REDIS_PREFIX must be 1-128 non-whitespace characters without Redis glob metacharacters (* ? [ ])".into());
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
            rate_limits,
            rate_limit_window: Duration::from_secs(rate_limit_window_seconds),
            rate_limit_ipv6_prefix_bits,
            trusted_proxy_cidrs,
            cors,
            storage_budget_bytes,
            bucket_bytes_per_hour,
            meter_resync_interval: Duration::from_secs(meter_resync_seconds),
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

fn parse_rate_limits() -> Result<RateLimits, String> {
    let write_address = bounded_limit("NYANBIN_RATE_LIMIT_REQUESTS", DEFAULT_RATE_LIMIT_REQUESTS)?;
    let reveal_address = bounded_limit(
        "NYANBIN_RATE_LIMIT_REVEAL_REQUESTS",
        DEFAULT_RATE_LIMIT_REVEAL_REQUESTS,
    )?;
    let info_address = bounded_limit(
        "NYANBIN_RATE_LIMIT_INFO_REQUESTS",
        DEFAULT_RATE_LIMIT_INFO_REQUESTS,
    )?;
    let delete_address = bounded_limit(
        "NYANBIN_RATE_LIMIT_DELETE_REQUESTS",
        DEFAULT_RATE_LIMIT_DELETE_REQUESTS,
    )?;
    let short_create_address = bounded_limit(
        "NYANBIN_RATE_LIMIT_SHORT_CREATE_REQUESTS",
        DEFAULT_RATE_LIMIT_SHORT_CREATE_REQUESTS,
    )?;
    let short_resolve_address = bounded_limit(
        "NYANBIN_RATE_LIMIT_SHORT_RESOLVE_REQUESTS",
        DEFAULT_RATE_LIMIT_SHORT_RESOLVE_REQUESTS,
    )?;
    let fallback_global = global_limit(
        "NYANBIN_RATE_LIMIT_GLOBAL_REQUESTS",
        DEFAULT_RATE_LIMIT_GLOBAL_REQUESTS,
    )?;
    let per_op = |name: &str| -> Result<u32, String> {
        match env::var(name) {
            Err(env::VarError::NotPresent) => Ok(fallback_global),
            Ok(value) if value.trim().is_empty() => Ok(fallback_global),
            _ => global_limit(name, fallback_global),
        }
    };
    Ok(RateLimits {
        reserve: OpLimit {
            address: write_address,
            global: per_op("NYANBIN_RATE_LIMIT_GLOBAL_RESERVE_REQUESTS")?,
        },
        commit: OpLimit {
            address: write_address,
            global: per_op("NYANBIN_RATE_LIMIT_GLOBAL_COMMIT_REQUESTS")?,
        },
        reveal: OpLimit {
            address: reveal_address,
            global: per_op("NYANBIN_RATE_LIMIT_GLOBAL_REVEAL_REQUESTS")?,
        },
        info: OpLimit {
            address: info_address,
            global: per_op("NYANBIN_RATE_LIMIT_GLOBAL_INFO_REQUESTS")?,
        },
        delete: OpLimit {
            address: delete_address,
            global: per_op("NYANBIN_RATE_LIMIT_GLOBAL_DELETE_REQUESTS")?,
        },
        short_create: OpLimit {
            address: short_create_address,
            global: per_op("NYANBIN_RATE_LIMIT_GLOBAL_SHORT_CREATE_REQUESTS")?,
        },
        short_resolve: OpLimit {
            address: short_resolve_address,
            global: per_op("NYANBIN_RATE_LIMIT_GLOBAL_SHORT_RESOLVE_REQUESTS")?,
        },
    })
}

fn bounded_limit(name: &str, default: u32) -> Result<u32, String> {
    let value = parse_env(name, default)?;
    if value == 0 || value > 100_000 {
        return Err(format!("{name} must be between 1 and 100000"));
    }
    Ok(value)
}

fn global_limit(name: &str, default: u32) -> Result<u32, String> {
    let value = parse_env(name, default)?;
    if value == 0 || value > 10_000_000 {
        return Err(format!("{name} must be between 1 and 10000000"));
    }
    Ok(value)
}

fn parse_cors(value: &str) -> Result<CorsPolicy, String> {
    let entries: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if entries.is_empty() {
        return Ok(CorsPolicy::Off);
    }
    if entries.contains(&"*") {
        if entries.len() != 1 {
            return Err(
                "NYANBIN_CORS_ORIGINS must be either '*' or a list of exact origins, not both"
                    .into(),
            );
        }
        return Ok(CorsPolicy::Any);
    }
    let mut origins = Vec::with_capacity(entries.len());
    for entry in entries {
        let parsed = url::Url::parse(entry)
            .map_err(|_| format!("invalid origin in NYANBIN_CORS_ORIGINS: {entry}"))?;
        let valid = matches!(parsed.scheme(), "http" | "https")
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.query().is_none()
            && parsed.fragment().is_none()
            && parsed.path() == "/"
            && !entry.ends_with('/');
        if !valid {
            return Err(format!(
                "NYANBIN_CORS_ORIGINS entries must be exact scheme://host[:port] origins; got: {entry}"
            ));
        }
        let canonical = parsed.origin().ascii_serialization();
        if !origins.contains(&canonical) {
            origins.push(canonical);
        }
    }
    Ok(CorsPolicy::List(origins))
}

fn abuse_contact(name: &str) -> Result<String, String> {
    let value = bounded_text(name, "", 254)?;
    if value.is_empty() {
        return Ok(value);
    }
    let valid = value.chars().filter(|c| *c == '@').count() == 1
        && !value.starts_with('@')
        && !value.ends_with('@')
        && !value.chars().any(char::is_whitespace);
    if !valid {
        return Err(format!("{name} must be blank or a plain email address"));
    }
    Ok(value)
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

    #[test]
    fn cors_default_is_off() {
        assert_eq!(parse_cors("").unwrap(), CorsPolicy::Off);
        assert_eq!(parse_cors("  ").unwrap(), CorsPolicy::Off);
    }

    #[test]
    fn cors_wildcard_is_any_and_exclusive() {
        assert_eq!(parse_cors("*").unwrap(), CorsPolicy::Any);
        assert!(parse_cors("*,https://a.example").is_err());
    }

    #[test]
    fn cors_accepts_exact_origins_only() {
        assert_eq!(
            parse_cors("https://a.example, http://b.example:8080").unwrap(),
            CorsPolicy::List(vec![
                "https://a.example".into(),
                "http://b.example:8080".into()
            ])
        );
        assert!(parse_cors("https://a.example/path").is_err());
        assert!(parse_cors("https://a.example/").is_err());
        assert!(parse_cors("ftp://a.example").is_err());
        assert!(parse_cors("https://user:pw@a.example").is_err());
    }

    #[test]
    fn cors_canonicalizes_valid_origin_spellings() {
        assert_eq!(
            parse_cors("HTTPS://EXAMPLE.COM:443").unwrap(),
            CorsPolicy::List(vec!["https://example.com".into()])
        );
    }

    #[test]
    fn abuse_contact_must_look_like_email() {
        unsafe { env::set_var("NYANBIN_TEST_ABUSE_OK", "abuse@example.com") };
        assert_eq!(
            abuse_contact("NYANBIN_TEST_ABUSE_OK").unwrap(),
            "abuse@example.com"
        );
        unsafe { env::set_var("NYANBIN_TEST_ABUSE_BAD", "not an email") };
        assert!(abuse_contact("NYANBIN_TEST_ABUSE_BAD").is_err());
        assert_eq!(abuse_contact("NYANBIN_TEST_ABUSE_UNSET").unwrap(), "");
    }
}
