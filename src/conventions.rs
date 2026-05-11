use std::borrow::Cow;

const KNOWN_METHODS: &[&str] = &[
    "CONNECT", "DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT", "QUERY", "TRACE",
];

const REDACTED_QUERY_KEYS: &[&str] = &["AWSAccessKeyId", "Signature", "sig", "X-Goog-Signature"];

/// Normalize an HTTP method to the set required by the OpenTelemetry HTTP semantic conventions.
///
/// Returns the canonical method name and, when the input was not in the known set, the original
/// method to be reported as `http.request.method_original`.
pub(crate) fn normalize_method(method: &'static str) -> (&'static str, Option<&'static str>) {
    if KNOWN_METHODS.contains(&method) {
        (method, None)
    } else {
        ("_OTHER", Some(method))
    }
}

/// Replace the values of well-known sensitive query string keys with `REDACTED`, per the
/// OpenTelemetry `url.query` semantic convention.
pub(crate) fn redact_query(query: &str) -> Cow<'_, str> {
    let is_sensitive = |pair: &str| {
        pair.split_once('=')
            .is_some_and(|(k, _)| REDACTED_QUERY_KEYS.contains(&k))
    };

    if !query.split('&').any(is_sensitive) {
        return Cow::Borrowed(query);
    }

    let mut result = String::with_capacity(query.len());
    for (i, pair) in query.split('&').enumerate() {
        if i > 0 {
            result.push('&');
        }
        match pair.split_once('=') {
            Some((k, _)) if REDACTED_QUERY_KEYS.contains(&k) => {
                result.push_str(k);
                result.push_str("=REDACTED");
            }
            _ => result.push_str(pair),
        }
    }
    Cow::Owned(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_known_method() {
        assert_eq!(normalize_method("GET"), ("GET", None));
        assert_eq!(normalize_method("QUERY"), ("QUERY", None));
    }

    #[test]
    fn normalize_unknown_method() {
        assert_eq!(normalize_method("PROPFIND"), ("_OTHER", Some("PROPFIND")));
    }

    #[test]
    fn redact_passthrough() {
        assert_eq!(redact_query("foo=bar&baz=qux"), "foo=bar&baz=qux");
        assert_eq!(redact_query(""), "");
    }

    #[test]
    fn redact_sensitive() {
        assert_eq!(
            redact_query("q=OpenTelemetry&sig=abc123"),
            "q=OpenTelemetry&sig=REDACTED"
        );
        assert_eq!(
            redact_query("AWSAccessKeyId=AKIA&Signature=xyz&other=ok"),
            "AWSAccessKeyId=REDACTED&Signature=REDACTED&other=ok"
        );
        assert_eq!(
            redact_query("X-Goog-Signature=zzz"),
            "X-Goog-Signature=REDACTED"
        );
    }

    #[test]
    fn redact_preserves_valueless_and_orphan_keys() {
        assert_eq!(redact_query("foo&bar=baz"), "foo&bar=baz");
    }
}
