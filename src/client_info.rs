//! Client IP and ISP detection, without any third-party service.

use hickory_resolver::TokioResolver;
use ipnet::IpNet;
use serde::Serialize;
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;

/// Resolve the effective client IP.
///
/// `X-Forwarded-For` is honoured only when the TCP peer is a trusted proxy. The
/// chain is walked from right to left, skipping trusted proxies; the first
/// untrusted hop is the client. Malformed entries stop the walk.
pub fn client_ip(peer: IpAddr, xff: Option<&str>, trusted: &[IpNet]) -> IpAddr {
    let peer = canonical(peer);
    let is_trusted = |ip: &IpAddr| trusted.iter().any(|n| n.contains(ip));
    if !is_trusted(&peer) {
        return peer;
    }
    let Some(xff) = xff else { return peer };
    let mut current = peer;
    for hop in xff.rsplit(',').map(str::trim) {
        match hop.parse::<IpAddr>() {
            Ok(ip) => {
                current = canonical(ip);
                if !is_trusted(&current) {
                    return current;
                }
            }
            Err(_) => return current,
        }
    }
    current
}

fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClientInfo {
    pub ip: String,
    pub hostname: Option<String>,
    pub isp: Option<String>,
    pub asn: Option<u32>,
}

/// Offline IP → ASN/organisation table loaded from a CSV/TSV file.
///
/// Accepted line formats (comment lines start with `#`):
///   `<cidr>,<asn>,<organisation>`  or  `<start_ip>\t<end_ip>\t<asn>\t<country>\t<organisation>`
/// (the latter is the iptoasn.com `ip2asn-combined.tsv` format).
#[derive(Default)]
pub struct IspDb {
    v4: BTreeMap<u32, (u32, u32, String)>,
    v6: BTreeMap<u128, (u128, u32, String)>,
}

impl IspDb {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(Self::parse(&text))
    }

    pub fn parse(text: &str) -> Self {
        let mut db = IspDb::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.contains('\t') {
                let f: Vec<&str> = line.split('\t').collect();
                if f.len() < 5 {
                    continue;
                }
                let (Ok(a), Ok(b), Ok(asn)) = (f[0].parse::<IpAddr>(), f[1].parse::<IpAddr>(), f[2].parse::<u32>())
                else {
                    continue;
                };
                if asn == 0 {
                    continue;
                }
                db.insert(a, b, asn, f[4].to_string());
            } else {
                let f: Vec<&str> = line.splitn(3, ',').collect();
                if f.len() < 3 {
                    continue;
                }
                let (Ok(net), Ok(asn)) = (
                    f[0].trim().parse::<IpNet>(),
                    f[1].trim().trim_start_matches("AS").parse::<u32>(),
                ) else {
                    continue;
                };
                db.insert(net.network(), net.broadcast(), asn, f[2].trim().to_string());
            }
        }
        db
    }

    fn insert(&mut self, a: IpAddr, b: IpAddr, asn: u32, org: String) {
        match (a, b) {
            (IpAddr::V4(a), IpAddr::V4(b)) => {
                self.v4.insert(a.into(), (b.into(), asn, org));
            }
            (IpAddr::V6(a), IpAddr::V6(b)) => {
                self.v6.insert(a.into(), (b.into(), asn, org));
            }
            _ => {}
        }
    }

    pub fn len(&self) -> usize {
        self.v4.len() + self.v6.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn lookup(&self, ip: IpAddr) -> Option<(u32, &str)> {
        match ip {
            IpAddr::V4(v) => {
                let k: u32 = v.into();
                let (_, (end, asn, org)) = self.v4.range(..=k).next_back()?;
                (k <= *end).then_some((*asn, org.as_str()))
            }
            IpAddr::V6(v) => {
                let k: u128 = v.into();
                let (_, (end, asn, org)) = self.v6.range(..=k).next_back()?;
                (k <= *end).then_some((*asn, org.as_str()))
            }
        }
    }
}

/// Derive a readable ISP-ish label from a reverse DNS name, e.g.
/// `lfbn-idf1-1-123.w90-1.abo.wanadoo.fr` → `wanadoo.fr`.
pub fn isp_from_hostname(host: &str) -> Option<String> {
    let host = host.trim_end_matches('.');
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return None;
    }
    // Keep three labels for well-known two-part public suffixes.
    let two_part = [
        "co.uk", "com.au", "co.jp", "com.br", "co.nz", "net.au", "ne.jp", "or.jp",
    ];
    let last2 = labels[labels.len() - 2..].join(".");
    let n = if two_part.contains(&last2.as_str()) && labels.len() >= 3 {
        3
    } else {
        2
    };
    Some(labels[labels.len() - n..].join("."))
}

pub async fn lookup(ip: IpAddr, resolver: Option<&TokioResolver>, isp_db: Option<&IspDb>) -> ClientInfo {
    let hostname = match resolver {
        Some(r) => tokio::time::timeout(Duration::from_secs(2), r.reverse_lookup(ip))
            .await
            .ok()
            .and_then(|r| r.ok())
            .and_then(|names| {
                names.answers().iter().find_map(|rec| match &rec.data {
                    hickory_resolver::proto::rr::RData::PTR(p) => Some(p.0.to_utf8().trim_end_matches('.').to_string()),
                    _ => None,
                })
            }),
        None => None,
    };
    let db_hit = isp_db.and_then(|db| db.lookup(ip));
    let isp = db_hit
        .map(|(_, org)| org.to_string())
        .or_else(|| hostname.as_deref().and_then(isp_from_hostname));
    ClientInfo {
        ip: ip.to_string(),
        hostname,
        isp,
        asn: db_hit.map(|(a, _)| a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets(s: &str) -> Vec<IpNet> {
        crate::config::parse_proxies(s).unwrap()
    }

    #[test]
    fn untrusted_peer_ignores_header() {
        let ip = client_ip("203.0.113.5".parse().unwrap(), Some("1.1.1.1"), &nets("10.0.0.0/8"));
        assert_eq!(ip.to_string(), "203.0.113.5");
    }

    #[test]
    fn no_proxies_configured_ignores_header() {
        let ip = client_ip("10.0.1.1".parse().unwrap(), Some("8.8.8.8"), &[]);
        assert_eq!(ip.to_string(), "10.0.1.1");
    }

    #[test]
    fn trusted_peer_uses_header() {
        let ip = client_ip("10.0.1.1".parse().unwrap(), Some("100.80.1.2"), &nets("10.0.0.0/8"));
        assert_eq!(ip.to_string(), "100.80.1.2");
    }

    #[test]
    fn spoofed_left_entries_are_ignored() {
        // Client sent "X-Forwarded-For: 6.6.6.6", the trusted proxy appended the real IP.
        let ip = client_ip(
            "10.0.1.1".parse().unwrap(),
            Some("6.6.6.6, 100.80.1.2"),
            &nets("10.0.0.0/8"),
        );
        assert_eq!(ip.to_string(), "100.80.1.2");
    }

    #[test]
    fn trusted_peer_without_header() {
        let ip = client_ip("10.0.1.1".parse().unwrap(), None, &nets("10.0.0.0/8"));
        assert_eq!(ip.to_string(), "10.0.1.1");
    }

    #[test]
    fn garbage_header_falls_back() {
        let ip = client_ip("10.0.1.1".parse().unwrap(), Some("not-an-ip"), &nets("10.0.0.0/8"));
        assert_eq!(ip.to_string(), "10.0.1.1");
    }

    #[test]
    fn mapped_v6_is_canonicalised() {
        let ip = client_ip("::ffff:203.0.113.9".parse().unwrap(), None, &[]);
        assert_eq!(ip.to_string(), "203.0.113.9");
    }

    #[test]
    fn isp_db_csv_and_tsv() {
        let db = IspDb::parse("# c\n100.64.0.0/10,AS41231,Tailscale Inc\n90.0.0.0\t90.127.255.255\t3215\tFR\tOrange\n");
        assert_eq!(db.len(), 2);
        assert_eq!(db.lookup("100.80.1.2".parse().unwrap()), Some((41231, "Tailscale Inc")));
        assert_eq!(db.lookup("90.1.2.3".parse().unwrap()), Some((3215, "Orange")));
        assert_eq!(db.lookup("91.0.0.1".parse().unwrap()), None);
    }

    #[test]
    fn isp_label_from_rdns() {
        assert_eq!(
            isp_from_hostname("lfbn-idf1-1-1.w90-1.abo.wanadoo.fr.").as_deref(),
            Some("wanadoo.fr")
        );
        assert_eq!(
            isp_from_hostname("host.example.co.uk").as_deref(),
            Some("example.co.uk")
        );
        assert_eq!(isp_from_hostname("localhost"), None);
    }
}
