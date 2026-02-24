//! SSRF (Server-Side Request Forgery) protection for webhook URLs.
//!
//! Blocks requests to private/internal IP ranges and special TLDs,
//! mirroring the `JavaScript` `safeValidateUrl()` implementation.

use crate::error::{A2AError, A2AResult};

/// Validate that a URL is safe to send webhook requests to.
///
/// Blocks the following patterns:
/// - Non-HTTP(S) protocols
/// - `localhost`, `127.0.0.1`, `0.0.0.0`, `::1`
/// - Private IP ranges: `10.x.x.x`, `192.168.x.x`, `172.16-31.x.x`
/// - Internal TLDs: `.internal`, `.local`, `.localhost`
///
/// # Errors
///
/// Returns [`A2AError::SsrfBlocked`] if the URL is blocked.
/// Returns [`A2AError::Validation`] if the URL is malformed.
///
/// # Example
///
/// ```
/// use stateset_a2a::notifications::validate_url;
///
/// // Public URL is fine
/// assert!(validate_url("https://example.com/webhooks").is_ok());
///
/// // Private IPs are blocked
/// assert!(validate_url("http://localhost:8080/hook").is_err());
/// assert!(validate_url("http://10.0.0.1/hook").is_err());
/// assert!(validate_url("http://192.168.1.1/hook").is_err());
/// ```
pub fn validate_url(url: &str) -> A2AResult<()> {
    if url.is_empty() {
        return Err(A2AError::Validation("URL is required".into()));
    }

    // Check scheme first (before full parse) so that non-HTTP protocols
    // are rejected with SsrfBlocked even if the host portion is missing/empty.
    let scheme_end = url.find("://").ok_or_else(|| {
        A2AError::Validation(format!("invalid URL (no scheme): {url}"))
    })?;
    let scheme = url[..scheme_end].to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(A2AError::SsrfBlocked(format!(
            "unsupported protocol: {scheme}"
        )));
    }

    // Parse the URL to extract host
    let (_scheme, host) = parse_url_components(url)?;

    // Check for blocked hostnames
    check_hostname(&host)?;

    Ok(())
}

/// Extract scheme and hostname from a URL string.
///
/// This is a minimal parser that avoids pulling in a full URL library.
fn parse_url_components(url: &str) -> A2AResult<(String, String)> {
    // Find scheme
    let Some(scheme_end) = url.find("://") else {
        return Err(A2AError::Validation(format!(
            "invalid URL (no scheme): {url}"
        )));
    };

    let scheme = url[..scheme_end].to_lowercase();

    // Extract host portion (after "://", before "/" or ":" port or "?")
    let after_scheme = &url[scheme_end + 3..];
    let host_end = after_scheme
        .find('/')
        .or_else(|| after_scheme.find('?'))
        .unwrap_or(after_scheme.len());
    let host_with_port = &after_scheme[..host_end];

    // Disallow userinfo (`user:pass@host`) to prevent host confusion bypasses.
    if host_with_port.contains('@') {
        return Err(A2AError::SsrfBlocked(
            "URL userinfo is not allowed for webhook targets".to_string(),
        ));
    }

    // Strip port if present (handle IPv6 bracket notation)
    let host = if host_with_port.starts_with('[') {
        // IPv6: [::1]:8080
        let bracket_end = host_with_port.find(']').unwrap_or(host_with_port.len());
        host_with_port[1..bracket_end].to_lowercase()
    } else if let Some(colon_pos) = host_with_port.rfind(':') {
        // Only strip port if what comes after the colon is numeric
        let after_colon = &host_with_port[colon_pos + 1..];
        if after_colon.chars().all(|c| c.is_ascii_digit()) {
            host_with_port[..colon_pos].to_lowercase()
        } else {
            host_with_port.to_lowercase()
        }
    } else {
        host_with_port.to_lowercase()
    };

    if host.is_empty() {
        return Err(A2AError::Validation(format!(
            "invalid URL (empty host): {url}"
        )));
    }

    Ok((scheme, host))
}

/// Check a hostname against the SSRF blocklist.
fn check_hostname(host: &str) -> A2AResult<()> {
    // Exact hostname matches
    if host == "localhost" || host == "127.0.0.1" || host == "0.0.0.0" || host == "::1" {
        return Err(A2AError::SsrfBlocked(format!(
            "cannot fetch internal URL: {host}"
        )));
    }

    // Private IP ranges
    if host.starts_with("10.") {
        return Err(A2AError::SsrfBlocked(format!(
            "cannot fetch private IP: {host}"
        )));
    }

    if host.starts_with("192.168.") {
        return Err(A2AError::SsrfBlocked(format!(
            "cannot fetch private IP: {host}"
        )));
    }

    // 172.16.0.0 - 172.31.255.255
    if host.starts_with("172.") {
        if let Some(second_octet_str) = host.strip_prefix("172.").and_then(|s| s.split('.').next()) {
            if let Ok(second_octet) = second_octet_str.parse::<u8>() {
                if (16..=31).contains(&second_octet) {
                    return Err(A2AError::SsrfBlocked(format!(
                        "cannot fetch private IP: {host}"
                    )));
                }
            }
        }
    }

    // Internal TLDs
    if host.ends_with(".internal") || host.ends_with(".local") || host.ends_with(".localhost") {
        return Err(A2AError::SsrfBlocked(format!(
            "cannot fetch internal domain: {host}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Valid URLs =====

    #[test]
    fn allows_public_https() {
        assert!(validate_url("https://example.com/webhooks").is_ok());
    }

    #[test]
    fn allows_public_http() {
        assert!(validate_url("http://api.example.com/hooks").is_ok());
    }

    #[test]
    fn allows_public_with_port() {
        assert!(validate_url("https://example.com:8443/webhooks").is_ok());
    }

    #[test]
    fn allows_subdomain() {
        assert!(validate_url("https://hooks.seller-bot.example.com/a2a").is_ok());
    }

    #[test]
    fn allows_public_ip() {
        assert!(validate_url("https://203.0.113.1/hook").is_ok());
    }

    // ===== Blocked: localhost =====

    #[test]
    fn blocks_localhost() {
        let err = validate_url("http://localhost/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_localhost_with_port() {
        let err = validate_url("http://localhost:8080/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_127_0_0_1() {
        let err = validate_url("http://127.0.0.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_0_0_0_0() {
        let err = validate_url("http://0.0.0.0/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_ipv6_loopback() {
        let err = validate_url("http://[::1]/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    // ===== Blocked: 10.x.x.x =====

    #[test]
    fn blocks_10_0_0_1() {
        let err = validate_url("http://10.0.0.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_10_255_255_255() {
        let err = validate_url("http://10.255.255.255/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    // ===== Blocked: 192.168.x.x =====

    #[test]
    fn blocks_192_168_0_1() {
        let err = validate_url("http://192.168.0.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_192_168_1_100() {
        let err = validate_url("http://192.168.1.100/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    // ===== Blocked: 172.16-31.x.x =====

    #[test]
    fn blocks_172_16() {
        let err = validate_url("http://172.16.0.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_172_31() {
        let err = validate_url("http://172.31.255.255/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_172_24() {
        let err = validate_url("http://172.24.1.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn allows_172_15() {
        // 172.15.x.x is NOT a private range
        assert!(validate_url("http://172.15.0.1/hook").is_ok());
    }

    #[test]
    fn allows_172_32() {
        // 172.32.x.x is NOT a private range
        assert!(validate_url("http://172.32.0.1/hook").is_ok());
    }

    // ===== Blocked: internal TLDs =====

    #[test]
    fn blocks_dot_internal() {
        let err = validate_url("http://service.internal/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_dot_local() {
        let err = validate_url("http://myhost.local/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_dot_localhost() {
        let err = validate_url("http://app.localhost/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    // ===== Blocked: bad protocol =====

    #[test]
    fn blocks_ftp() {
        let err = validate_url("ftp://example.com/file").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_file() {
        let err = validate_url("file:///etc/passwd").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    // ===== Invalid URLs =====

    #[test]
    fn rejects_empty_url() {
        let err = validate_url("").unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn rejects_no_scheme() {
        let err = validate_url("example.com/hook").unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    // ===== Parse tests =====

    #[test]
    fn parse_components_basic() {
        let (scheme, host) = parse_url_components("https://example.com/path").unwrap();
        assert_eq!(scheme, "https");
        assert_eq!(host, "example.com");
    }

    #[test]
    fn parse_components_with_port() {
        let (scheme, host) = parse_url_components("http://api.test.com:9090/path").unwrap();
        assert_eq!(scheme, "http");
        assert_eq!(host, "api.test.com");
    }

    #[test]
    fn parse_components_ipv6() {
        let (scheme, host) = parse_url_components("http://[::1]:8080/path").unwrap();
        assert_eq!(scheme, "http");
        assert_eq!(host, "::1");
    }

    #[test]
    fn parse_components_uppercase_scheme() {
        let (scheme, _host) = parse_url_components("HTTPS://example.com/path").unwrap();
        assert_eq!(scheme, "https");
    }

    #[test]
    fn parse_components_query_string() {
        let (_, host) = parse_url_components("https://example.com?foo=bar").unwrap();
        assert_eq!(host, "example.com");
    }

    #[test]
    fn rejects_userinfo_host_confusion() {
        let err = validate_url("http://attacker@localhost:8080/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn parse_rejects_userinfo() {
        let err = parse_url_components("https://user:pass@example.com/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }
}
