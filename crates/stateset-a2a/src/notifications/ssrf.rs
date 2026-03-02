//! SSRF (Server-Side Request Forgery) protection for webhook URLs.
//!
//! Blocks requests to private, loopback, link-local, and reserved IP ranges,
//! plus internal-use hostnames/TLDs. For DNS hostnames, resolved addresses are
//! also checked to prevent DNS rebinding style bypasses.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

use crate::error::{A2AError, A2AResult};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedUrl {
    scheme: String,
    host: String,
    port: Option<u16>,
}

/// Optional validation controls for outbound webhook URLs.
///
/// Defaults remain strict and unchanged when no allowlist entries are configured.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UrlValidationOptions {
    outbound_allowlist: Vec<String>,
}

impl UrlValidationOptions {
    /// Build default URL validation options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the outbound allowlist with normalized host rules.
    ///
    /// Supported entry forms:
    /// - `example.com` (exact host match)
    /// - `*.example.com` (subdomain match only; apex excluded)
    ///
    /// Empty/blank entries are ignored.
    pub fn with_outbound_allowlist<I, S>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.outbound_allowlist = normalize_allowlist(entries);
        self
    }

    /// Return normalized allowlist entries.
    pub fn outbound_allowlist(&self) -> &[String] {
        &self.outbound_allowlist
    }
}

/// Validate that a URL is safe to send webhook requests to.
///
/// Blocks the following patterns:
/// - Non-HTTP(S) protocols
/// - Hostname bypass patterns (`userinfo@host`, malformed host/port)
/// - Local/internal hostnames (`localhost`, `.internal`, `.local`, etc.)
/// - Loopback/private/link-local/reserved IP ranges (`IPv4` + `IPv6`)
/// - Hostnames that DNS-resolve to blocked IP ranges
///
/// # Errors
///
/// Returns [`A2AError::SsrfBlocked`] if the URL is blocked.
/// Returns [`A2AError::Validation`] if the URL is malformed.
pub fn validate_url(url: &str) -> A2AResult<()> {
    validate_url_with_options(url, &UrlValidationOptions::default())
}

/// Validate URL safety with optional outbound allowlist constraints.
pub fn validate_url_with_options(url: &str, options: &UrlValidationOptions) -> A2AResult<()> {
    validate_url_with_resolver_and_options(url, resolve_host_ips, options)
}

#[cfg(test)]
fn validate_url_with_resolver<F>(url: &str, resolver: F) -> A2AResult<()>
where
    F: Fn(&str, u16) -> Vec<IpAddr>,
{
    validate_url_with_resolver_and_options(url, resolver, &UrlValidationOptions::default())
}

fn validate_url_with_resolver_and_options<F>(
    url: &str,
    resolver: F,
    options: &UrlValidationOptions,
) -> A2AResult<()>
where
    F: Fn(&str, u16) -> Vec<IpAddr>,
{
    if url.trim().is_empty() {
        return Err(A2AError::Validation("URL is required".into()));
    }

    let scheme_end = url
        .find("://")
        .ok_or_else(|| A2AError::Validation(format!("invalid URL (no scheme): {url}")))?;
    let scheme = url[..scheme_end].to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(A2AError::SsrfBlocked(format!("unsupported protocol: {scheme}")));
    }

    let parsed = parse_url_components(url)?;

    check_hostname_rules(&parsed.host)?;
    check_outbound_allowlist(&parsed.host, options)?;

    if let Ok(ip) = parsed.host.parse::<IpAddr>() {
        return check_ip(ip, &parsed.host);
    }

    // Handle ambiguous single-integer IPv4 literals (e.g., 2130706433).
    if let Some(ip) = parse_decimal_ipv4_literal(&parsed.host) {
        return check_ip(IpAddr::V4(ip), &parsed.host);
    }
    if is_ambiguous_ipv4_encoding_host(&parsed.host) {
        return Err(A2AError::SsrfBlocked(format!(
            "ambiguous IPv4 host encoding is not allowed: {}",
            parsed.host
        )));
    }

    validate_hostname_syntax(&parsed.host)?;

    let default_port = if parsed.scheme == "https" { 443 } else { 80 };
    let lookup_port = parsed.port.unwrap_or(default_port);
    let resolved_ips = resolver(&parsed.host, lookup_port);
    if resolved_ips.is_empty() {
        return Err(A2AError::SsrfBlocked(format!(
            "host could not be resolved safely: {}",
            parsed.host
        )));
    }
    for resolved_ip in resolved_ips {
        if let Some(reason) = blocked_ip_reason(resolved_ip) {
            return Err(A2AError::SsrfBlocked(format!(
                "cannot fetch {reason}: {} resolved to {resolved_ip}",
                parsed.host
            )));
        }
    }

    Ok(())
}

/// Extract scheme, hostname, and optional port from a URL string.
///
/// This is an intentionally strict parser to avoid host confusion bypasses.
fn parse_url_components(url: &str) -> A2AResult<ParsedUrl> {
    let Some(scheme_end) = url.find("://") else {
        return Err(A2AError::Validation(format!("invalid URL (no scheme): {url}")));
    };

    let scheme = url[..scheme_end].to_ascii_lowercase();
    let after_scheme = &url[scheme_end + 3..];

    let authority_end = after_scheme.find(['/', '?', '#']).unwrap_or(after_scheme.len());
    let authority = &after_scheme[..authority_end];
    if authority.is_empty() {
        return Err(A2AError::Validation(format!("invalid URL (empty host): {url}")));
    }

    if authority.contains('@') {
        return Err(A2AError::SsrfBlocked(
            "URL userinfo is not allowed for webhook targets".to_string(),
        ));
    }

    let (raw_host, port) = parse_authority(authority, url)?;
    let host = raw_host.trim_end_matches('.').to_ascii_lowercase();

    if host.is_empty() {
        return Err(A2AError::Validation(format!("invalid URL (empty host): {url}")));
    }

    if host.bytes().any(|b| b.is_ascii_control() || b == b' ' || b == b'\t') {
        return Err(A2AError::Validation(format!("invalid URL host characters: {url}")));
    }

    Ok(ParsedUrl { scheme, host, port })
}

fn parse_authority(authority: &str, original_url: &str) -> A2AResult<(String, Option<u16>)> {
    if authority.starts_with('[') {
        let Some(bracket_end) = authority.find(']') else {
            return Err(A2AError::Validation(format!(
                "invalid URL (unterminated IPv6 host): {original_url}"
            )));
        };

        let host = &authority[1..bracket_end];
        if host.is_empty() {
            return Err(A2AError::Validation(format!("invalid URL (empty host): {original_url}")));
        }
        if host.contains('%') {
            return Err(A2AError::SsrfBlocked(
                "IPv6 zone identifiers are not allowed for webhook targets".to_string(),
            ));
        }

        let remainder = &authority[bracket_end + 1..];
        let port = parse_port_suffix(remainder, original_url)?;
        return Ok((host.to_string(), port));
    }

    if authority.contains('[') || authority.contains(']') {
        return Err(A2AError::Validation(format!("invalid URL host syntax: {original_url}")));
    }

    if let Some((host_part, port_part)) = authority.rsplit_once(':') {
        if authority.matches(':').count() > 1 {
            return Err(A2AError::Validation(format!(
                "invalid URL host (IPv6 must be bracketed): {original_url}"
            )));
        }

        if port_part.is_empty() {
            return Err(A2AError::Validation(format!("invalid URL port: {original_url}")));
        }
        if !port_part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(A2AError::Validation(format!("invalid URL port: {original_url}")));
        }
        let port = parse_port(port_part, original_url)?;
        return Ok((host_part.to_string(), Some(port)));
    }

    Ok((authority.to_string(), None))
}

fn parse_port_suffix(port_suffix: &str, original_url: &str) -> A2AResult<Option<u16>> {
    if port_suffix.is_empty() {
        return Ok(None);
    }

    let Some(port_str) = port_suffix.strip_prefix(':') else {
        return Err(A2AError::Validation(format!("invalid URL port: {original_url}")));
    };
    if port_str.is_empty() || !port_str.bytes().all(|b| b.is_ascii_digit()) {
        return Err(A2AError::Validation(format!("invalid URL port: {original_url}")));
    }
    let port = parse_port(port_str, original_url)?;
    Ok(Some(port))
}

fn parse_port(port: &str, original_url: &str) -> A2AResult<u16> {
    let parsed = port
        .parse::<u16>()
        .map_err(|_| A2AError::Validation(format!("invalid URL port: {original_url}")))?;

    if parsed == 0 {
        return Err(A2AError::Validation(format!("invalid URL port: {original_url}")));
    }
    Ok(parsed)
}

fn normalize_allowlist<I, S>(entries: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut normalized = Vec::new();
    for entry in entries {
        let candidate = normalize_host_match_value(entry.as_ref());
        if candidate.is_empty() {
            continue;
        }
        if !normalized.contains(&candidate) {
            normalized.push(candidate);
        }
    }
    normalized
}

fn normalize_host_match_value(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(suffix) = trimmed.strip_prefix("*.") {
        let suffix = suffix.trim_end_matches('.').to_ascii_lowercase();
        if suffix.is_empty() {
            return String::new();
        }
        return format!("*.{suffix}");
    }
    trimmed.trim_end_matches('.').to_ascii_lowercase()
}

fn check_outbound_allowlist(host: &str, options: &UrlValidationOptions) -> A2AResult<()> {
    if options.outbound_allowlist.is_empty() {
        return Ok(());
    }

    if host_matches_allowlist(host, &options.outbound_allowlist) {
        return Ok(());
    }

    Err(A2AError::SsrfBlocked(format!("host is not in outbound allowlist: {host}")))
}

fn host_matches_allowlist(host: &str, allowlist: &[String]) -> bool {
    let normalized_host = normalize_host_match_value(host);

    allowlist.iter().any(|entry| allowlist_entry_matches(normalized_host.as_str(), entry.as_str()))
}

fn allowlist_entry_matches(host: &str, entry: &str) -> bool {
    if let Some(suffix) = entry.strip_prefix("*.") {
        // Subdomain-only wildcard. `*.example.com` does not match `example.com`.
        return host.len() > suffix.len()
            && host.ends_with(suffix)
            && host
                .as_bytes()
                .get(host.len().saturating_sub(suffix.len() + 1))
                .is_some_and(|b| *b == b'.');
    }

    host == entry
}

fn check_hostname_rules(host: &str) -> A2AResult<()> {
    let blocked_exact = [
        "localhost",
        "localhost.localdomain",
        "local",
        "internal",
        "test",
        "invalid",
        "example",
        "home.arpa",
    ];
    if blocked_exact.contains(&host) {
        return Err(A2AError::SsrfBlocked(format!("cannot fetch internal URL: {host}")));
    }

    let blocked_suffixes = [
        ".localhost",
        ".localdomain",
        ".local",
        ".internal",
        ".home.arpa",
        ".test",
        ".invalid",
        ".example",
    ];
    if blocked_suffixes.iter().any(|suffix| host.ends_with(suffix)) {
        return Err(A2AError::SsrfBlocked(format!("cannot fetch internal domain: {host}")));
    }

    Ok(())
}

fn is_ambiguous_ipv4_encoding_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.is_empty() || labels.len() > 4 {
        return false;
    }

    let mut saw_hex = false;
    for label in &labels {
        if label.is_empty() {
            return false;
        }

        if let Some(hex_digits) = label.strip_prefix("0x") {
            if hex_digits.is_empty() || !hex_digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                return false;
            }
            saw_hex = true;
            continue;
        }

        if !label.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }

    // Single-integer decimal literals are already handled by parse_decimal_ipv4_literal.
    if labels.len() == 1 {
        return saw_hex;
    }

    saw_hex
        || labels.len() < 4
        || labels.iter().any(|label| label.len() > 1 && label.starts_with('0'))
}

fn validate_hostname_syntax(host: &str) -> A2AResult<()> {
    if host.len() > 253 {
        return Err(A2AError::Validation("invalid host: too long".to_string()));
    }

    for label in host.split('.') {
        if label.is_empty() {
            return Err(A2AError::Validation("invalid host: empty label".to_string()));
        }
        if label.len() > 63 {
            return Err(A2AError::Validation("invalid host: label too long".to_string()));
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(A2AError::Validation(
                "invalid host: label cannot start/end with '-'".to_string(),
            ));
        }
        if !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(A2AError::Validation("invalid host: illegal characters".to_string()));
        }
    }

    Ok(())
}

fn check_ip(ip: IpAddr, display_host: &str) -> A2AResult<()> {
    if let Some(reason) = blocked_ip_reason(ip) {
        return Err(A2AError::SsrfBlocked(format!("cannot fetch {reason}: {display_host}")));
    }
    Ok(())
}

fn blocked_ip_reason(ip: IpAddr) -> Option<&'static str> {
    match ip {
        IpAddr::V4(v4) => blocked_ipv4_reason(v4),
        IpAddr::V6(v6) => blocked_ipv6_reason(v6),
    }
}

fn blocked_ipv4_reason(ip: Ipv4Addr) -> Option<&'static str> {
    let [a, b, c, d] = ip.octets();

    if a == 127 {
        return Some("loopback IP");
    }
    if a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168) {
        return Some("private IP");
    }
    if a == 169 && b == 254 {
        return Some("link-local IP");
    }
    if a == 0 {
        return Some("unspecified IP");
    }
    if a == 100 && (64..=127).contains(&b) {
        return Some("shared carrier-grade NAT IP");
    }
    if (a == 192 && b == 0 && c == 2)
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
    {
        return Some("documentation/reserved IP");
    }
    if a == 198 && (b == 18 || b == 19) {
        return Some("benchmark/reserved IP");
    }
    if a == 192 && b == 88 && c == 99 {
        return Some("reserved IP");
    }
    if a == 255 && b == 255 && c == 255 && d == 255 {
        return Some("broadcast IP");
    }
    if a >= 224 {
        return Some("multicast/reserved IP");
    }

    None
}

fn blocked_ipv6_reason(ip: Ipv6Addr) -> Option<&'static str> {
    if ip.is_loopback() {
        return Some("loopback IP");
    }
    if ip.is_unspecified() {
        return Some("unspecified IP");
    }

    let [s0, s1, _, _, _, _, _, _] = ip.segments();

    // fc00::/7 (unique-local)
    if (s0 & 0xfe00) == 0xfc00 {
        return Some("private IP");
    }
    // fe80::/10 (link-local unicast)
    if (s0 & 0xffc0) == 0xfe80 {
        return Some("link-local IP");
    }
    // fec0::/10 (deprecated site-local)
    if (s0 & 0xffc0) == 0xfec0 {
        return Some("reserved IP");
    }
    // ff00::/8 (multicast)
    if (s0 & 0xff00) == 0xff00 {
        return Some("multicast/reserved IP");
    }
    // 2001:db8::/32 documentation range.
    if s0 == 0x2001 && s1 == 0x0db8 {
        return Some("documentation/reserved IP");
    }

    // IPv4-mapped IPv6 (::ffff:a.b.c.d): treat as blocked-by-default even
    // when mapped IPv4 is public to avoid parser/normalization confusion.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return blocked_ipv4_reason(v4).or(Some("IPv4-mapped IPv6 address"));
    }

    None
}

fn parse_decimal_ipv4_literal(host: &str) -> Option<Ipv4Addr> {
    if !host.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = host.parse::<u32>().ok()?;
    Some(Ipv4Addr::from(n))
}

fn resolve_host_ips(host: &str, port: u16) -> Vec<IpAddr> {
    let addr = format!("{host}:{port}");
    match addr.to_socket_addrs() {
        Ok(iter) => {
            let mut ips = Vec::new();
            for socket_addr in iter {
                let ip = socket_addr.ip();
                if !ips.contains(&ip) {
                    ips.push(ip);
                }
            }
            ips
        }
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Valid URLs =====

    #[test]
    fn allows_public_https() {
        let result = validate_url_with_resolver("https://example.com/webhooks", |_host, _port| {
            vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]
        });
        assert!(result.is_ok());
    }

    #[test]
    fn allows_public_http() {
        let result = validate_url_with_resolver("http://api.example.com/hooks", |_host, _port| {
            vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]
        });
        assert!(result.is_ok());
    }

    #[test]
    fn allows_public_with_port() {
        let result =
            validate_url_with_resolver("https://example.com:8443/webhooks", |_host, _port| {
                vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]
            });
        assert!(result.is_ok());
    }

    #[test]
    fn allows_subdomain() {
        let result = validate_url_with_resolver(
            "https://hooks.seller-bot.example.com/a2a",
            |_host, _port| vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn allows_public_ip() {
        assert!(validate_url("https://8.8.8.8/hook").is_ok());
    }

    // ===== Blocked: loopback/private/link-local =====

    #[test]
    fn blocks_localhost() {
        let err = validate_url("http://localhost/hook").unwrap_err();
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
    fn blocks_10_0_0_1() {
        let err = validate_url("http://10.0.0.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_172_16_to_31() {
        let err = validate_url("http://172.31.255.255/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_192_168() {
        let err = validate_url("http://192.168.1.100/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_169_254_link_local() {
        let err = validate_url("http://169.254.1.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_ipv6_loopback() {
        let err = validate_url("http://[::1]/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_ipv6_link_local() {
        let err = validate_url("http://[fe80::1]/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_ipv6_unique_local() {
        let err = validate_url("http://[fd00::1]/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_ipv4_mapped_ipv6_loopback() {
        let err = validate_url("http://[::ffff:127.0.0.1]/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    // ===== Blocked: reserved/documentation =====

    #[test]
    fn blocks_documentation_ipv4_range() {
        let err = validate_url("http://203.0.113.10/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_documentation_ipv6_range() {
        let err = validate_url("http://[2001:db8::1]/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_benchmark_ipv4_range() {
        let err = validate_url("http://198.18.0.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn blocks_carrier_grade_nat_range() {
        let err = validate_url("http://100.64.0.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
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

    #[test]
    fn blocks_dot_home_arpa() {
        let err = validate_url("http://router.home.arpa/hook").unwrap_err();
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

    #[test]
    fn rejects_userinfo_host_confusion() {
        let err = validate_url("http://attacker@localhost:8080/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn rejects_unbracketed_ipv6() {
        let err = validate_url("http://::1/hook").unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn rejects_invalid_port_non_numeric() {
        let err = validate_url("https://example.com:notaport/hook").unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn rejects_invalid_port_zero() {
        let err = validate_url("https://example.com:0/hook").unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn rejects_ipv6_zone_identifier() {
        let err = validate_url("http://[fe80::1%25eth0]/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    // ===== Parse tests =====

    #[test]
    fn parse_components_basic() {
        let parsed = parse_url_components("https://example.com/path").unwrap();
        assert_eq!(parsed.scheme, "https");
        assert_eq!(parsed.host, "example.com");
        assert_eq!(parsed.port, None);
    }

    #[test]
    fn parse_components_with_port() {
        let parsed = parse_url_components("http://api.test.com:9090/path").unwrap();
        assert_eq!(parsed.scheme, "http");
        assert_eq!(parsed.host, "api.test.com");
        assert_eq!(parsed.port, Some(9090));
    }

    #[test]
    fn parse_components_ipv6() {
        let parsed = parse_url_components("http://[::1]:8080/path").unwrap();
        assert_eq!(parsed.scheme, "http");
        assert_eq!(parsed.host, "::1");
        assert_eq!(parsed.port, Some(8080));
    }

    #[test]
    fn parse_components_query_and_fragment() {
        let parsed = parse_url_components("https://example.com/path?a=1#frag").unwrap();
        assert_eq!(parsed.host, "example.com");
    }

    // ===== DNS resolution checks =====

    #[test]
    fn dns_resolution_blocks_private_target() {
        let err = validate_url_with_resolver("https://public.example.com/hook", |_host, _port| {
            vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))]
        })
        .unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn dns_resolution_allows_public_target() {
        let result =
            validate_url_with_resolver("https://public.example.com/hook", |_host, _port| {
                vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))]
            });
        assert!(result.is_ok());
    }

    #[test]
    fn decimal_ipv4_literal_is_blocked() {
        // 2130706433 == 127.0.0.1
        let err = validate_url("http://2130706433/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn rejects_hex_ipv4_literal_encoding() {
        let err = validate_url("http://0x7f000001/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn rejects_octal_ipv4_literal_encoding() {
        let err = validate_url("http://0177.0.0.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn rejects_short_ipv4_literal_encoding() {
        let err = validate_url("http://127.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn rejects_ipv4_mapped_ipv6_even_when_mapped_ip_is_public() {
        let err = validate_url("http://[::ffff:8.8.8.8]/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn rejects_malformed_unterminated_ipv6_authority() {
        let err = validate_url("http://[::1/hook").unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn rejects_malformed_empty_ipv6_authority() {
        let err = validate_url("http://[]/hook").unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn rejects_malformed_empty_authority_host_with_port() {
        let err = validate_url("http://:8080/hook").unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn rejects_userinfo_confusion_variant() {
        let err = validate_url("https://safe.example.com@127.0.0.1/hook").unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn dns_resolution_blocks_mixed_public_and_private_targets() {
        let err = validate_url_with_resolver("https://public.example.com/hook", |_host, _port| {
            vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))]
        })
        .unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn allowlist_empty_preserves_default_behavior() {
        let options = UrlValidationOptions::new();
        let result = validate_url_with_resolver_and_options(
            "https://api.example.com/hook",
            |_host, _port| vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
            &options,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn allowlist_allows_exact_host() {
        let options =
            UrlValidationOptions::new().with_outbound_allowlist(["api.example.com".to_string()]);
        let result = validate_url_with_resolver_and_options(
            "https://api.example.com/hook",
            |_host, _port| vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
            &options,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn allowlist_blocks_non_listed_host() {
        let options =
            UrlValidationOptions::new().with_outbound_allowlist(["api.example.com".to_string()]);
        let err =
            validate_url_with_options("https://other.example.com/hook", &options).unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn allowlist_supports_wildcard_subdomains_only() {
        let options = UrlValidationOptions::new().with_outbound_allowlist(["*.example.com"]);
        let result = validate_url_with_resolver_and_options(
            "https://hooks.example.com/hook",
            |_host, _port| vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
            &options,
        );
        assert!(result.is_ok());
        let err = validate_url_with_options("https://example.com/hook", &options).unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn allowlist_cannot_override_private_ip_blocks() {
        let options = UrlValidationOptions::new().with_outbound_allowlist(["127.0.0.1"]);
        let err = validate_url_with_options("https://127.0.0.1/hook", &options).unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn allowlist_dns_rebinding_style_resolution_is_still_blocked() {
        let options = UrlValidationOptions::new().with_outbound_allowlist(["api.example.com"]);
        let err = validate_url_with_resolver_and_options(
            "https://api.example.com/hook",
            |_host, _port| vec![IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))],
            &options,
        )
        .unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }

    #[test]
    fn dns_resolution_failure_style_empty_result_is_blocked() {
        let err = validate_url_with_resolver("https://api.example.com/hook", |_host, _port| vec![])
            .unwrap_err();
        assert!(matches!(err, A2AError::SsrfBlocked(_)));
    }
}
