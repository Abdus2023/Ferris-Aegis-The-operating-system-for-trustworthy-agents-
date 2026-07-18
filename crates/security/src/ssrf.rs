//! SSRF (Server-Side Request Forgery) guard.
//!
//! Validates URLs and hostnames before an agent makes network requests.
//! Rejects connections to private/internal IP ranges, loopback, link-local,
//! and metadata endpoints that could leak cloud credentials.

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

/// The verdict returned by the SSRF guard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SsrfVerdict {
    /// The target is safe to connect to.
    Safe,
    /// The target is blocked due to SSRF risk.
    Blocked {
        /// Why the target was blocked.
        reason: String,
    },
}

impl SsrfVerdict {
    /// Whether the target is safe to connect to.
    pub fn is_safe(&self) -> bool {
        matches!(self, SsrfVerdict::Safe)
    }
}

/// SSRF guard — validates network targets before connection.
///
/// Blocks connections to:
/// - Loopback addresses (127.0.0.0/8, ::1)
/// - Private IPv4 ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
/// - Link-local addresses (169.254.0.0/16, fe80::/10)
/// - Cloud metadata endpoints (169.254.169.254)
/// - IPv6 unique local (fc00::/7)
/// - "localhost" hostname
#[derive(Debug, Clone)]
pub struct SsrfGuard {
    /// Whether the guard is enabled.
    enabled: bool,
}

impl SsrfGuard {
    /// Create a new SSRF guard (enabled by default).
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Create a disabled guard (all targets pass).
    pub fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Enable the guard.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the guard.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Check a hostname for SSRF risk.
    pub fn check_hostname(&self, hostname: &str) -> SsrfVerdict {
        if !self.enabled {
            return SsrfVerdict::Safe;
        }

        // Block "localhost" directly
        if hostname.eq_ignore_ascii_case("localhost") {
            return SsrfVerdict::Blocked {
                reason: "localhost hostname blocked".to_string(),
            };
        }

        // Try to parse as IP address
        if let Ok(ip) = IpAddr::from_str(hostname) {
            return self.check_ip(&ip);
        }

        // Hostname that isn't an IP literal — allow through
        // (DNS resolution is the caller's responsibility; the IP check
        // should be run again after resolution if strict mode is needed)
        SsrfVerdict::Safe
    }

    /// Check an IP address for SSRF risk.
    pub fn check_ip(&self, ip: &IpAddr) -> SsrfVerdict {
        if !self.enabled {
            return SsrfVerdict::Safe;
        }

        match ip {
            IpAddr::V4(v4) => self.check_ipv4(v4),
            IpAddr::V6(v6) => self.check_ipv6(v6),
        }
    }

    /// Check an IPv4 address.
    fn check_ipv4(&self, ip: &Ipv4Addr) -> SsrfVerdict {
        let octets = ip.octets();

        // Loopback: 127.0.0.0/8
        if octets[0] == 127 {
            return SsrfVerdict::Blocked {
                reason: format!("loopback address {} blocked", ip),
            };
        }

        // Private: 10.0.0.0/8
        if octets[0] == 10 {
            return SsrfVerdict::Blocked {
                reason: format!("private address {} blocked (10.0.0.0/8)", ip),
            };
        }

        // Private: 172.16.0.0/12
        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
            return SsrfVerdict::Blocked {
                reason: format!("private address {} blocked (172.16.0.0/12)", ip),
            };
        }

        // Private: 192.168.0.0/16
        if octets[0] == 192 && octets[1] == 168 {
            return SsrfVerdict::Blocked {
                reason: format!("private address {} blocked (192.168.0.0/16)", ip),
            };
        }

        // Link-local: 169.254.0.0/16 (includes cloud metadata at 169.254.169.254)
        if octets[0] == 169 && octets[1] == 254 {
            if octets[2] == 169 && octets[3] == 254 {
                return SsrfVerdict::Blocked {
                    reason: "cloud metadata endpoint 169.254.169.254 blocked".to_string(),
                };
            }
            return SsrfVerdict::Blocked {
                reason: format!("link-local address {} blocked (169.254.0.0/16)", ip),
            };
        }

        // Broadcast: 255.255.255.255
        if ip.is_broadcast() {
            return SsrfVerdict::Blocked {
                reason: "broadcast address blocked".to_string(),
            };
        }

        SsrfVerdict::Safe
    }

    /// Check an IPv6 address.
    fn check_ipv6(&self, ip: &Ipv6Addr) -> SsrfVerdict {
        // Loopback: ::1
        if ip.is_loopback() {
            return SsrfVerdict::Blocked {
                reason: format!("loopback address {} blocked", ip),
            };
        }

        // Unique local: fc00::/7
        let segments = ip.segments();
        if (segments[0] & 0xfe00) == 0xfc00 {
            return SsrfVerdict::Blocked {
                reason: format!("unique local address {} blocked (fc00::/7)", ip),
            };
        }

        // Link-local: fe80::/10
        if (segments[0] & 0xffc0) == 0xfe80 {
            return SsrfVerdict::Blocked {
                reason: format!("link-local address {} blocked (fe80::/10)", ip),
            };
        }

        SsrfVerdict::Safe
    }

    /// Check a full URL by extracting the host and checking it.
    pub fn check_url(&self, url: &str) -> SsrfVerdict {
        if !self.enabled {
            return SsrfVerdict::Safe;
        }

        // Extract host from URL — simple parser, no dependency on url crate
        let host = extract_host(url);
        match host {
            Some(h) => self.check_hostname(&h),
            None => SsrfVerdict::Blocked {
                reason: "could not parse host from URL".to_string(),
            },
        }
    }
}

/// Extract the hostname from a URL string.
///
/// Simple parser that handles `scheme://host:port/path` format.
fn extract_host(url: &str) -> Option<String> {
    // Strip scheme
    let after_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        url
    };

    // Strip path
    let authority = if let Some(idx) = after_scheme.find('/') {
        &after_scheme[..idx]
    } else {
        after_scheme
    };

    // Strip port
    let host = if let Some(idx) = authority.rfind(':') {
        &authority[..idx]
    } else {
        authority
    };

    // Strip brackets from IPv6 literals
    let host = host.trim_start_matches('[').trim_end_matches(']');

    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

impl Default for SsrfGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn rejects_loopback() {
        let guard = SsrfGuard::new();
        assert!(!guard.check_ip(&IpAddr::from([127, 0, 0, 1])).is_safe());
        assert!(!guard.check_ip(&IpAddr::from([127, 255, 255, 255])).is_safe());
    }

    #[test]
    fn rejects_private_10() {
        let guard = SsrfGuard::new();
        assert!(!guard.check_ip(&IpAddr::from([10, 0, 0, 1])).is_safe());
        assert!(!guard.check_ip(&IpAddr::from([10, 255, 255, 255])).is_safe());
    }

    #[test]
    fn rejects_private_172() {
        let guard = SsrfGuard::new();
        assert!(!guard.check_ip(&IpAddr::from([172, 16, 0, 1])).is_safe());
        assert!(!guard.check_ip(&IpAddr::from([172, 31, 255, 255])).is_safe());
        // 172.15.x.x is NOT private
        assert!(guard.check_ip(&IpAddr::from([172, 15, 0, 1])).is_safe());
        // 172.32.x.x is NOT private
        assert!(guard.check_ip(&IpAddr::from([172, 32, 0, 1])).is_safe());
    }

    #[test]
    fn rejects_private_192() {
        let guard = SsrfGuard::new();
        assert!(!guard.check_ip(&IpAddr::from([192, 168, 0, 1])).is_safe());
        assert!(!guard.check_ip(&IpAddr::from([192, 168, 255, 255])).is_safe());
    }

    #[test]
    fn rejects_cloud_metadata() {
        let guard = SsrfGuard::new();
        assert!(!guard.check_ip(&IpAddr::from([169, 254, 169, 254])).is_safe());
    }

    #[test]
    fn rejects_link_local() {
        let guard = SsrfGuard::new();
        assert!(!guard.check_ip(&IpAddr::from([169, 254, 0, 1])).is_safe());
    }

    #[test]
    fn allows_public_ips() {
        let guard = SsrfGuard::new();
        assert!(guard.check_ip(&IpAddr::from([8, 8, 8, 8])).is_safe());
        assert!(guard.check_ip(&IpAddr::from([1, 1, 1, 1])).is_safe());
        assert!(guard.check_ip(&IpAddr::from([203, 0, 113, 1])).is_safe());
    }

    #[test]
    fn rejects_localhost_hostname() {
        let guard = SsrfGuard::new();
        assert!(!guard.check_hostname("localhost").is_safe());
        assert!(!guard.check_hostname("LOCALHOST").is_safe());
    }

    #[test]
    fn allows_public_hostnames() {
        let guard = SsrfGuard::new();
        assert!(guard.check_hostname("api.openai.com").is_safe());
        assert!(guard.check_hostname("example.com").is_safe());
    }

    #[test]
    fn checks_url_host() {
        let guard = SsrfGuard::new();
        assert!(guard.check_url("https://api.openai.com/v1/chat").is_safe());
        assert!(!guard.check_url("http://localhost:3000/api").is_safe());
        assert!(!guard.check_url("http://10.0.0.1:8080/internal").is_safe());
        assert!(!guard.check_url("http://169.254.169.254/metadata").is_safe());
    }

    #[test]
    fn disabled_guard_allows_everything() {
        let guard = SsrfGuard::disabled();
        assert!(guard.check_ip(&IpAddr::from([127, 0, 0, 1])).is_safe());
        assert!(guard.check_hostname("localhost").is_safe());
        assert!(guard.check_url("http://169.254.169.254/metadata").is_safe());
    }

    #[test]
    fn rejects_ipv6_loopback() {
        let guard = SsrfGuard::new();
        assert!(!guard.check_ip(&IpAddr::from(Ipv6Addr::LOCALHOST)).is_safe());
    }

    #[test]
    fn rejects_ipv6_link_local() {
        let guard = SsrfGuard::new();
        let fe80 = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1);
        assert!(!guard.check_ip(&IpAddr::from(fe80)).is_safe());
    }
}
