//! Where `wipe serve` listens and which URLs it hands out: the bind plan for each
//! exposure mode, this machine's LAN / Tailscale addresses, and a terminal QR
//! code so a phone can open the board without typing a token.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use wipe_core::model::Exposure;

/// A URL worth printing, with a short label (interface / network name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownUrl {
    /// e.g. `LAN (Wi-Fi)`, `tailscale`, `this machine`.
    pub label: String,
    /// The URL (with `?token=` for remote ones).
    pub url: String,
}

/// Whether `ip` is in Tailscale's CGNAT range (100.64.0.0/10).
pub fn is_tailscale(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (64..=127).contains(&o[1])
}

/// This machine's non-loopback, non-link-local IPv4 addresses with their
/// interface names, the primary (default-route) address first.
pub fn local_ipv4() -> Vec<(String, Ipv4Addr)> {
    let mut out: Vec<(String, Ipv4Addr)> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i.ip() {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified() => {
                Some((i.name.clone(), v4))
            }
            _ => None,
        })
        .collect();
    out.sort_by_key(|(_, ip)| *ip);
    out.dedup_by_key(|(_, ip)| *ip);
    if let Some(primary) = primary_ipv4() {
        if let Some(pos) = out.iter().position(|(_, ip)| *ip == primary) {
            let p = out.remove(pos);
            out.insert(0, p);
        }
    }
    out
}

/// The address the OS would use to reach the internet (no packet is sent - a
/// UDP "connect" only consults the routing table).
fn primary_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

/// This machine's Tailscale IPv4 address, if Tailscale is up.
pub fn tailscale_ipv4() -> Option<Ipv4Addr> {
    local_ipv4()
        .into_iter()
        .map(|(_, ip)| ip)
        .find(|ip| is_tailscale(*ip))
}

/// The MagicDNS name of this machine (e.g. `laptop.tail1234.ts.net`), when the
/// `tailscale` CLI is available and reports one.
pub fn tailscale_dns_name() -> Option<String> {
    let out = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let name = v["Self"]["DNSName"].as_str()?.trim_end_matches('.');
    (!name.is_empty()).then(|| name.to_string())
}

/// The sockets to bind for `expose` on `port`, or an explicit `host`.
pub fn bind_plan(
    expose: Exposure,
    host: Option<IpAddr>,
    port: u16,
) -> anyhow::Result<Vec<SocketAddr>> {
    if let Some(h) = host {
        return Ok(vec![SocketAddr::new(h, port)]);
    }
    let lo = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    Ok(match expose {
        Exposure::Local => vec![lo],
        Exposure::Lan | Exposure::Proxy => vec![SocketAddr::from((Ipv4Addr::UNSPECIFIED, port))],
        Exposure::Tailscale => {
            let ts = tailscale_ipv4().ok_or_else(|| {
                anyhow::anyhow!(
                    "no Tailscale address found on this machine - is Tailscale running and \
                     logged in (`tailscale up`)? serve on the LAN instead with `--expose lan`"
                )
            })?;
            vec![lo, SocketAddr::from((ts, port))]
        }
    })
}

/// Whether any bound address is reachable from other machines.
pub fn is_remote(addrs: &[SocketAddr]) -> bool {
    addrs.iter().any(|a| !a.ip().is_loopback())
}

/// The URLs to print for a serve bound to `addrs`: the local one first (no
/// token needed from this machine unless behind a proxy), then one per remote
/// address carrying the token.
pub fn urls(addrs: &[SocketAddr], token: Option<&str>, local_needs_token: bool) -> Vec<ShownUrl> {
    let port = addrs.first().map(|a| a.port()).unwrap_or(0);
    let q = |needs: bool| match (token, needs) {
        (Some(t), true) => format!("/?token={t}"),
        _ => String::new(),
    };
    let mut out = vec![ShownUrl {
        label: "this machine".into(),
        url: format!("http://localhost:{port}{}", q(local_needs_token)),
    }];
    let mut remote: Vec<(String, Ipv4Addr)> = Vec::new();
    for a in addrs {
        match a.ip() {
            IpAddr::V4(v4) if v4.is_unspecified() => remote.extend(local_ipv4()),
            IpAddr::V4(v4) if !v4.is_loopback() => remote.push((String::new(), v4)),
            IpAddr::V6(v6) if !v6.is_loopback() => out.push(ShownUrl {
                label: "IPv6".into(),
                url: format!("http://[{v6}]:{port}{}", q(true)),
            }),
            _ => {}
        }
    }
    for (name, ip) in remote {
        let label = if is_tailscale(ip) {
            "tailscale".to_string()
        } else if name.is_empty() {
            "network".to_string()
        } else {
            format!("network ({name})")
        };
        out.push(ShownUrl {
            label,
            url: format!("http://{ip}:{port}{}", q(true)),
        });
    }
    out
}

/// Render `text` as a compact QR code using Unicode half blocks (two modules
/// per character row), dark-on-light with a quiet zone so phones read it on
/// dark terminals too.
pub fn qr(text: &str) -> Option<String> {
    use qrcode::render::unicode::Dense1x2;
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    Some(
        code.render::<Dense1x2>()
            .dark_color(Dense1x2::Light)
            .light_color(Dense1x2::Dark)
            .quiet_zone(true)
            .build(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailscale_range_is_cgnat() {
        assert!(is_tailscale(Ipv4Addr::new(100, 64, 0, 1)));
        assert!(is_tailscale(Ipv4Addr::new(100, 101, 2, 3)));
        assert!(is_tailscale(Ipv4Addr::new(100, 127, 255, 255)));
        assert!(!is_tailscale(Ipv4Addr::new(100, 128, 0, 1)));
        assert!(!is_tailscale(Ipv4Addr::new(192, 168, 1, 2)));
    }

    #[test]
    fn bind_plans_per_mode() {
        let p = 6737;
        assert_eq!(
            bind_plan(Exposure::Local, None, p).unwrap(),
            vec![SocketAddr::from((Ipv4Addr::LOCALHOST, p))]
        );
        assert_eq!(
            bind_plan(Exposure::Lan, None, p).unwrap(),
            vec![SocketAddr::from((Ipv4Addr::UNSPECIFIED, p))]
        );
        let host: IpAddr = "192.168.5.5".parse().unwrap();
        assert_eq!(
            bind_plan(Exposure::Local, Some(host), p).unwrap(),
            vec![SocketAddr::new(host, p)]
        );
        assert!(!is_remote(&bind_plan(Exposure::Local, None, p).unwrap()));
        assert!(is_remote(&bind_plan(Exposure::Lan, None, p).unwrap()));
    }

    #[test]
    fn urls_put_the_token_only_on_remote_addresses() {
        let addrs = vec![
            SocketAddr::from((Ipv4Addr::LOCALHOST, 7000)),
            SocketAddr::from((Ipv4Addr::new(100, 90, 1, 2), 7000)),
        ];
        let u = urls(&addrs, Some("tok"), false);
        assert_eq!(u[0].url, "http://localhost:7000");
        assert_eq!(u[1].label, "tailscale");
        assert_eq!(u[1].url, "http://100.90.1.2:7000/?token=tok");
        // Behind a proxy the local URL needs the token as well.
        assert_eq!(
            urls(&addrs, Some("tok"), true)[0].url,
            "http://localhost:7000/?token=tok"
        );
    }

    #[test]
    fn qr_renders_a_block_grid() {
        let q = qr("http://192.168.1.2:6737/?token=abc").unwrap();
        assert!(q.lines().count() > 10);
        assert!(q.contains('█') || q.contains('▀') || q.contains('▄'));
    }
}
