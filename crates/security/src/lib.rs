//! Boundary validation. Public server-side fetchers also need DNS/IP validation and
//! destination pinning in the network layer.

use std::net::{Ipv4Addr, Ipv6Addr};
use thiserror::Error;
use url::{Host, Url};

const MAX_URL_BYTES: usize = 8 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UrlPolicyError {
    #[error("URL is empty or exceeds the permitted length")] InvalidLength,
    #[error("URL could not be parsed")] InvalidUrl,
    #[error("only HTTP and HTTPS URLs are supported")] UnsupportedScheme,
    #[error("URL must include a host")] MissingHost,
    #[error("embedded URL credentials are not accepted")] EmbeddedCredentials,
    #[error("public fetch cannot target a non-public IP literal")] NonPublicAddress,
}

pub fn validate_http_url(raw: &str) -> Result<Url, UrlPolicyError> {
    if raw.is_empty() || raw.len() > MAX_URL_BYTES || raw.chars().any(char::is_control) {
        return Err(UrlPolicyError::InvalidLength);
    }
    let mut url = Url::parse(raw).map_err(|_| UrlPolicyError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") { return Err(UrlPolicyError::UnsupportedScheme); }
    if url.host_str().is_none() { return Err(UrlPolicyError::MissingHost); }
    if !url.username().is_empty() || url.password().is_some() { return Err(UrlPolicyError::EmbeddedCredentials); }
    // URL fragments are browser-side only; do not pass them to a server or persist them.
    url.set_fragment(None);
    Ok(url)
}

/// Rejects obvious private IP literals for server-side fetches. Domain names still
/// require checking every DNS A/AAAA result and pinning the validated address to the
/// socket connection, including after redirects, to prevent DNS rebinding.
pub fn validate_public_fetch_url(raw: &str) -> Result<Url, UrlPolicyError> {
    let url = validate_http_url(raw)?;
    match url.host() {
        Some(Host::Ipv4(ip)) if is_non_public_v4(ip) => Err(UrlPolicyError::NonPublicAddress),
        Some(Host::Ipv6(ip)) if is_non_public_v6(ip) => Err(UrlPolicyError::NonPublicAddress),
        _ => Ok(url),
    }
}

fn is_non_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, d] = ip.octets();
    ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified()
        || ip.is_multicast() || ip == Ipv4Addr::BROADCAST || a == 0
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && (c == 0 || c == 113))
        || (a == 192 && b == 88 && c == 99)
        || (a == 255 && b == 255 && c == 255 && d == 255)
}

fn is_non_public_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    ip.is_loopback() || ip.is_unspecified() || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || ip.to_ipv4().is_some_and(is_non_public_v4)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_http_and_https_are_accepted() {
        assert!(validate_http_url("https://example.test/media.mp4").is_ok());
        assert!(validate_http_url("http://example.test/media.mp4").is_ok());
        for input in ["file:///etc/passwd", "javascript:alert(1)", "data:text/plain,hi", "ftp://example.test/x"] {
            assert!(validate_http_url(input).is_err(), "accepted {input}");
        }
    }
    #[test]
    fn credentials_and_controls_are_rejected() {
        assert_eq!(validate_http_url("https://user:pass@example.test/media.mp4").unwrap_err(), UrlPolicyError::EmbeddedCredentials);
        assert!(validate_http_url("https://example.test/\nmedia").is_err());
    }
    #[test]
    fn strips_fragment() {
        let url = validate_http_url("https://example.test/a.mp4#player").unwrap();
        assert_eq!(url.fragment(), None);
    }
    #[test]
    fn public_fetch_blocks_private_ip_literals() {
        for input in ["http://127.0.0.1/admin", "http://10.1.2.3/", "http://192.168.1.10/", "http://169.254.169.254/latest/meta-data/", "http://[::1]/", "http://[fe80::1]/"] {
            assert_eq!(validate_public_fetch_url(input).unwrap_err(), UrlPolicyError::NonPublicAddress);
        }
        assert!(validate_public_fetch_url("https://93.184.216.34/").is_ok());
    }
}
