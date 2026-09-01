use axum::{
    body::Body,
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

const HEADERS: &[(&str, &str)] = &[
    (
        "content-security-policy",
        "base-uri 'none'; form-action 'self'; frame-ancestors 'none'; object-src 'none'",
    ),
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-resource-policy", "same-origin"),
    (
        "permissions-policy",
        "camera=(), geolocation=(), microphone=(), payment=(), usb=()",
    ),
    ("referrer-policy", "no-referrer"),
    (
        "strict-transport-security",
        "max-age=31536000; includeSubDomains",
    ),
    ("x-content-type-options", "nosniff"),
    ("x-frame-options", "DENY"),
];

pub async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let is_api = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    for (name, value) in HEADERS {
        let name = HeaderName::from_static(name);
        // Inner layers (CORS) may have set a deliberate value — e.g. CORP
        // cross-origin on allowlisted API responses. Defaults never override.
        if !response.headers().contains_key(&name) {
            response
                .headers_mut()
                .insert(name, HeaderValue::from_static(value));
        }
    }
    if is_api {
        response
            .headers_mut()
            .insert("cache-control", HeaderValue::from_static("no-store"));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csp_complements_the_prerendered_resource_policy() {
        let csp = HEADERS
            .iter()
            .find(|(name, _)| *name == "content-security-policy")
            .unwrap()
            .1;
        assert!(csp.contains("base-uri 'none'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("object-src 'none'"));
        assert!(!csp.contains("unsafe-inline"));
        assert!(!csp.contains("https:"));
    }
}
