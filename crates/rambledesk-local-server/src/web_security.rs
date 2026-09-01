use axum::http::{HeaderMap, HeaderValue, header};
use subtle::ConstantTimeEq;

pub(crate) fn header_text(value: &HeaderValue) -> Option<&str> {
    value.to_str().ok()
}

pub(crate) fn has_exact_host(headers: &HeaderMap, allowed_host: &str) -> bool {
    headers.get(header::HOST).and_then(header_text) == Some(allowed_host)
}

pub(crate) fn has_exact_host_and_origin(
    headers: &HeaderMap,
    allowed_host: &str,
    allowed_origin: &str,
) -> bool {
    has_exact_host(headers, allowed_host)
        && headers.get(header::ORIGIN).and_then(header_text) == Some(allowed_origin)
}

pub(crate) fn bearer_credential(value: Option<&HeaderValue>) -> Option<&str> {
    value
        .and_then(header_text)
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
}

pub(crate) fn constant_time_bytes_eq(expected: &[u8], candidate: &[u8]) -> bool {
    expected.len() == candidate.len() && bool::from(expected.ct_eq(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_host_origin_and_bearer_primitives_reject_near_matches() {
        let cases = [
            ("127.0.0.1:37643", "http://127.0.0.1:37643", true),
            ("localhost:37643", "http://127.0.0.1:37643", false),
            ("127.0.0.1:37643", "http://localhost:37643", false),
            ("127.0.0.1:37643.evil", "http://127.0.0.1:37643", false),
        ];
        for (host, origin, expected) in cases {
            let mut headers = HeaderMap::new();
            headers.insert(header::HOST, HeaderValue::from_str(host).expect("host"));
            headers.insert(
                header::ORIGIN,
                HeaderValue::from_str(origin).expect("origin"),
            );
            assert_eq!(
                has_exact_host_and_origin(&headers, "127.0.0.1:37643", "http://127.0.0.1:37643",),
                expected,
            );
        }

        assert_eq!(
            bearer_credential(Some(&HeaderValue::from_static("Bearer session-token"))),
            Some("session-token"),
        );
        assert_eq!(
            bearer_credential(Some(&HeaderValue::from_static("bearer session-token"))),
            None,
        );
        assert_eq!(
            bearer_credential(Some(&HeaderValue::from_static("Bearer "))),
            None,
        );
        assert!(constant_time_bytes_eq(b"session-token", b"session-token"));
        assert!(!constant_time_bytes_eq(b"session-token", b"session-tokee"));
        assert!(!constant_time_bytes_eq(b"session-token", b"short"));
    }
}
