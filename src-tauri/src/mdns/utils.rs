// Copyright 2026 ZenohX Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::net::IpAddr;

/// Sanitizes a raw hostname into a valid mDNS hostname ending with `.local.`.
pub fn sanitize_hostname(name: &str) -> String {
    let mut cleaned = name.trim().to_lowercase();
    if cleaned.is_empty() {
        cleaned = "zenohx".to_string();
    }
    // Strip trailing or leading dots
    cleaned = cleaned.trim_matches('.').to_string();
    // Strip .local if user included it
    if let Some(stripped) = cleaned.strip_suffix(".local") {
        cleaned = stripped.trim_matches('.').to_string();
    }
    // Remove invalid DNS characters (only keep a-z, 0-9, '-')
    cleaned = cleaned
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    cleaned = cleaned.trim_matches('-').to_string();
    if cleaned.is_empty() {
        cleaned = "zenohx".to_string();
    }
    format!("{cleaned}.local.")
}

/// Formats a sanitized mDNS FQDN for user-facing display (e.g. "zenohx.local").
pub fn display_hostname(fqdn: &str) -> String {
    fqdn.trim_end_matches('.').to_string()
}

/// Checks whether a network interface is a virtual bridge, container, VM, or VPN interface.
pub fn is_virtual_or_sub_interface(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("docker")
        || lower.starts_with("br-")
        || lower.starts_with("bridge")
        || lower.starts_with("br0")
        || lower.starts_with("cni")
        || lower.starts_with("flannel")
        || lower.starts_with("calico")
        || lower.starts_with("waydroid")
        || lower.starts_with("anbox")
        || lower.starts_with("veth")
        || lower.starts_with("virbr")
        || lower.starts_with("vnet")
        || lower.starts_with("vboxnet")
        || lower.starts_with("vmnet")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("tailscale")
        || lower.starts_with("cloudflare")
        || lower.starts_with("warp")
        || lower.starts_with("wg")
        || lower.starts_with("wireguard")
        || lower.starts_with("zt")
        || lower.starts_with("zerotier")
        || lower.starts_with("utun")
        || lower.starts_with("ppp")
        || lower.starts_with("dummy")
        || lower.contains(':')
        || lower.contains("vethernet")
        || lower.contains("virtualbox")
        || lower.contains("vmware")
        || lower.contains("hyper-v")
        || lower.contains("wsl")
}

/// Checks whether an IP address is a private Local Area Network (LAN) address.
///
/// Returns true for:
/// - IPv4 RFC 1918 private ranges:
///   - 10.0.0.0/8 (10.0.0.0 - 10.255.255.255)
///   - 172.16.0.0/12 (172.16.0.0 - 172.31.255.255)
///   - 192.168.0.0/16 (192.168.0.0 - 192.168.255.255)
/// - IPv6 RFC 4193 Unique Local Addresses (ULA): fc00::/7 (fd00::/8)
pub fn is_lan_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private(),
        IpAddr::V6(v6) => (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

/// Detects the primary local LAN IP address by querying the OS kernel routing table.
pub fn get_primary_local_ip() -> Option<IpAddr> {
    for target in &["8.8.8.8:80", "1.1.1.1:80", "223.5.5.5:80"] {
        if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(target).is_ok() {
                if let Ok(local_addr) = socket.local_addr() {
                    let ip = local_addr.ip();
                    if is_lan_ip(&ip) {
                        return Some(ip);
                    }
                }
            }
        }
    }
    None
}

/// Collects only local LAN network interface IPv4 and IPv6 addresses without duplicates.
/// Strictly filters out non-LAN IPs (WAN, CGNAT, loopback, link-local) and virtual
/// bridge/container interfaces (docker, waydroid, veth, etc.).
pub fn collect_local_ip_addresses() -> Vec<IpAddr> {
    let mut lan_ips = Vec::new();

    // 1. Primary LAN IP detected via kernel routing table
    if let Some(ip) = get_primary_local_ip() {
        if is_lan_ip(&ip) {
            lan_ips.push(ip);
        }
    }

    // 2. Iterate interfaces and collect ONLY non-virtual, physical LAN IPs
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            let ip = iface.ip();
            if !is_virtual_or_sub_interface(&iface.name) && is_lan_ip(&ip) {
                if !lan_ips.contains(&ip) {
                    lan_ips.push(ip);
                }
            }
        }
    }

    // 3. Fallback: If no LAN interfaces are connected (e.g. offline machine), fallback to loopback
    if lan_ips.is_empty() {
        lan_ips.push(IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
    } else if lan_ips.len() > 1 {
        // Sort: primary LAN IP first, then IPv4 LAN, then IPv6 ULA
        let first = lan_ips.remove(0);
        lan_ips.sort_by_key(|ip| match ip {
            IpAddr::V4(_) => 0,
            IpAddr::V6(_) => 1,
        });
        lan_ips.insert(0, first);
        lan_ips.dedup();
    }

    lan_ips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_hostname() {
        assert_eq!(sanitize_hostname("zenohx"), "zenohx.local.");
        assert_eq!(sanitize_hostname("zenohx.local"), "zenohx.local.");
        assert_eq!(sanitize_hostname("ZenohX.local."), "zenohx.local.");
        assert_eq!(sanitize_hostname("my robot!"), "my-robot.local.");
        assert_eq!(sanitize_hostname("---my robot!---"), "my-robot.local.");
        assert_eq!(sanitize_hostname(""), "zenohx.local.");
        assert_eq!(sanitize_hostname("---"), "zenohx.local.");
    }

    #[test]
    fn test_display_hostname() {
        assert_eq!(display_hostname("zenohx.local."), "zenohx.local");
        assert_eq!(display_hostname("zenohx.local"), "zenohx.local");
    }

    #[test]
    fn test_is_virtual_or_sub_interface() {
        assert!(is_virtual_or_sub_interface("docker0"));
        assert!(is_virtual_or_sub_interface("br-12345"));
        assert!(is_virtual_or_sub_interface("bridge0"));
        assert!(is_virtual_or_sub_interface("waydroid0"));
        assert!(is_virtual_or_sub_interface("veth5BUH1U"));
        assert!(is_virtual_or_sub_interface("CloudflareWARP"));
        assert!(is_virtual_or_sub_interface("tailscale0"));
        assert!(is_virtual_or_sub_interface("virbr0"));
        assert!(is_virtual_or_sub_interface("vEthernet (WSL)"));
        assert!(is_virtual_or_sub_interface("eth0:1"));

        assert!(!is_virtual_or_sub_interface("wlp0s20f3"));
        assert!(!is_virtual_or_sub_interface("enp3s0"));
        assert!(!is_virtual_or_sub_interface("eth0"));
        assert!(!is_virtual_or_sub_interface("wlan0"));
    }

    #[test]
    fn test_is_lan_ip() {
        // IPv4 private ranges (RFC 1918)
        assert!(is_lan_ip(&"192.168.101.10".parse().unwrap()));
        assert!(is_lan_ip(&"192.168.1.1".parse().unwrap()));
        assert!(is_lan_ip(&"10.0.0.1".parse().unwrap()));
        assert!(is_lan_ip(&"10.254.0.1".parse().unwrap()));
        assert!(is_lan_ip(&"172.16.0.1".parse().unwrap()));
        assert!(is_lan_ip(&"172.31.255.254".parse().unwrap()));

        // IPv6 Unique Local Address (RFC 4193 ULA)
        assert!(is_lan_ip(&"fd00::1".parse().unwrap()));
        assert!(is_lan_ip(&"fc00::1".parse().unwrap()));

        // Non-LAN addresses (Public, Loopback, Link-Local, CGNAT)
        assert!(!is_lan_ip(&"127.0.0.1".parse().unwrap()));
        assert!(!is_lan_ip(&"::1".parse().unwrap()));
        assert!(!is_lan_ip(&"169.254.1.1".parse().unwrap()));
        assert!(!is_lan_ip(&"100.96.0.5".parse().unwrap()));
        assert!(!is_lan_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_lan_ip(&"1.1.1.1".parse().unwrap()));
        assert!(!is_lan_ip(&"fe80::1".parse().unwrap()));
        assert!(!is_lan_ip(&"2606:4700::1".parse().unwrap()));
    }

    #[test]
    fn test_collect_local_ip_addresses() {
        let ips = collect_local_ip_addresses();
        assert!(!ips.is_empty(), "Should return at least one IP address");
        let mut unique_ips = ips.clone();
        unique_ips.sort();
        unique_ips.dedup();
        assert_eq!(ips.len(), unique_ips.len(), "IP list should not contain duplicates");

        // Verify all returned addresses are valid LAN IPs (or 127.0.0.1 fallback if offline)
        if ips.len() > 1 || ips[0] != IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)) {
            for ip in &ips {
                assert!(is_lan_ip(ip), "IP {ip} should be a private LAN IP");
            }
        }
    }
}
