use axum::{
    body::Body,
    extract::{Request, State},
    http::{
        HeaderValue, Method, StatusCode,
        header::{
            ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
            ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, ACCESS_CONTROL_MAX_AGE,
            ORIGIN, VARY,
        },
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::{AppState, config::CorsPolicy};

const ALLOW_METHODS: &str = "GET, POST, PUT, DELETE, OPTIONS";
const ALLOW_HEADERS: &str = "content-type";
const EXPOSE_HEADERS: &str = "retry-after";
// Ten minutes: long enough to collapse preflights, short enough that
// tightening the allowlist takes effect quickly.
const MAX_AGE: &str = "600";

/// Cross-origin access for `/api/*` per `NYANBIN_CORS_ORIGINS`. Credentials
/// are never allowed; the only request header the API needs is content-type.
pub async fn cors(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let policy = &state.config.cors;
    let is_api = request.uri().path().starts_with("/api/") || request.uri().path() == "/api";
    if !is_api || !policy.enabled() {
        return next.run(request).await;
    }
    let allowed = allowed_origin(policy, request.headers().get(ORIGIN));
    if request.method() == Method::OPTIONS {
        let mut response = StatusCode::NO_CONTENT.into_response();
        // CORP describes the resource, not the requester: on a CORS-enabled
        // instance every API response is a cross-origin-fetchable resource.
        response.headers_mut().insert(
            "cross-origin-resource-policy",
            HeaderValue::from_static("cross-origin"),
        );
        if let Some(origin) = allowed {
            let headers = response.headers_mut();
            headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            headers.insert(
                ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static(ALLOW_METHODS),
            );
            headers.insert(
                ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static(ALLOW_HEADERS),
            );
            headers.insert(ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static(MAX_AGE));
        }
        add_vary(&mut response, policy);
        return response;
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("cross-origin"),
    );
    if let Some(origin) = allowed {
        let headers = response.headers_mut();
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        headers.insert(
            ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static(EXPOSE_HEADERS),
        );
    }
    add_vary(&mut response, policy);
    response
}

fn allowed_origin(policy: &CorsPolicy, origin: Option<&HeaderValue>) -> Option<HeaderValue> {
    match policy {
        CorsPolicy::Off => None,
        CorsPolicy::Any => Some(HeaderValue::from_static("*")),
        CorsPolicy::List(origins) => {
            let value = origin?.to_str().ok()?;
            origins
                .iter()
                .any(|allowed| allowed == value)
                .then(|| origin.cloned())?
        }
    }
}

fn add_vary(response: &mut Response, policy: &CorsPolicy) {
    // Origin-dependent responses must not be cached across origins.
    if matches!(policy, CorsPolicy::List(_)) {
        response
            .headers_mut()
            .append(VARY, HeaderValue::from_static("origin"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(value: &str) -> HeaderValue {
        HeaderValue::from_str(value).unwrap()
    }

    #[test]
    fn off_policy_never_allows() {
        assert!(allowed_origin(&CorsPolicy::Off, Some(&origin("https://a.example"))).is_none());
    }

    #[test]
    fn any_policy_uses_wildcard_without_echo() {
        let value = allowed_origin(&CorsPolicy::Any, Some(&origin("https://evil.example")));
        assert_eq!(value.unwrap(), HeaderValue::from_static("*"));
        // Wildcard applies even without an Origin header (e.g. curl).
        assert!(allowed_origin(&CorsPolicy::Any, None).is_some());
    }

    #[test]
    fn list_policy_echoes_exact_match_only() {
        let policy = CorsPolicy::List(vec!["https://a.example".into()]);
        assert_eq!(
            allowed_origin(&policy, Some(&origin("https://a.example"))).unwrap(),
            origin("https://a.example")
        );
        assert!(allowed_origin(&policy, Some(&origin("https://a.example.evil"))).is_none());
        assert!(allowed_origin(&policy, Some(&origin("http://a.example"))).is_none());
        assert!(allowed_origin(&policy, None).is_none());
    }
}
