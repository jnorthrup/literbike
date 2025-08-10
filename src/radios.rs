use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use crate::syscall_net::{classify_ipv4, classify_ipv6, list_interfaces, InterfaceAddr};

#[derive(Debug, Serialize, Deserialize)]
pub struct RadioIface {
    pub name: String,
    pub domain: String,   // wifi | cell | vpn | other
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    pub mac: Option<String>,
    pub v4_mode: String,  // none|private|cgnat|public|loopback|link-local|special
    pub v6_mode: String,  // none|link-local|unique-local|global|loopback|unspecified|multicast
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RadiosReport {
    pub interfaces: Vec<RadioIface>,
    pub android_props: HashMap<String, String>,
}

fn domain_for_name(name: &str) -> &'static str {
    if name.starts_with("rmnet") || name.starts_with("ccmni") || name.starts_with("wwan") {
        "cell"
    } else if name.starts_with("wlan") || name.starts_with("swlan") || name.starts_with("wifi") {
        "wifi"
    } else if name.starts_with("tun") || name.starts_with("tap") || name.starts_with("wg") || name.starts_with("utun") {
        "vpn"
    } else {
        "other"
    }
}

pub fn gather_radios() -> RadiosReport {
    let mut ifs = Vec::new();
    if let Ok(map) = list_interfaces() {
        for (name, iface) in map {
            let mut v4s = Vec::new();
            let mut v6s = Vec::new();
            let mut mac: Option<String> = None;
            for a in iface.addrs {
                match a {
                    InterfaceAddr::V4(ip) => v4s.push(ip.to_string()),
                    InterfaceAddr::V6(ip) => v6s.push(ip.to_string()),
                    InterfaceAddr::Link(m) => {
                        if mac.is_none() {
                            mac = Some(format!(
                                "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                                m.get(0).copied().unwrap_or(0),
                                m.get(1).copied().unwrap_or(0),
                                m.get(2).copied().unwrap_or(0),
                                m.get(3).copied().unwrap_or(0),
                                m.get(4).copied().unwrap_or(0),
                                m.get(5).copied().unwrap_or(0)
                            ));
                        }
                    }
                }
            }

            // Modes summarize first available class
            let v4_mode = if let Some(first) = v4s.get(0) {
                let ip: std::net::Ipv4Addr = first.parse().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
                classify_ipv4(ip).to_string()
            } else {
                "none".to_string()
            };
            let v6_mode = if let Some(first) = v6s.get(0) {
                let ip: std::net::Ipv6Addr = first.parse().unwrap_or(std::net::Ipv6Addr::UNSPECIFIED);
                classify_ipv6(ip).to_string()
            } else {
                "none".to_string()
            };

            ifs.push(RadioIface {
                name: name.clone(),
                domain: domain_for_name(&name).to_string(),
                ipv4: v4s,
                ipv6: v6s,
                mac,
                v4_mode,
                v6_mode,
            });
        }
    }

    #[cfg(any(target_os = "android"))]
    let android_props = crate::syscall_net::android_carrier_props();
    #[cfg(not(any(target_os = "android")))]
    let android_props: HashMap<String, String> = HashMap::new();

    RadiosReport { interfaces: ifs, android_props }
}

pub fn print_radios_human(report: &RadiosReport) {
    println!("radios: interfaces={} android_props={}", report.interfaces.len(), report.android_props.len());
    for i in &report.interfaces {
        println!(
            "{:<10} domain={:<5} v4={:<16} v6_count={:<2} mac={}",
            i.name,
            i.domain,
            i.ipv4.get(0).cloned().unwrap_or_else(|| "-".into()),
            i.ipv6.len(),
            i.mac.clone().unwrap_or_else(|| "-".into())
        );
    }
    if !report.android_props.is_empty() {
        println!("android props:");
        let mut keys: Vec<_> = report.android_props.keys().cloned().collect();
        keys.sort();
        for k in keys { println!("{} = {}", k, report.android_props[&k]); }
    }
}

// --- Remote parsing helpers ---

#[derive(Deserialize)]
struct IpAddrInfo { family: String, local: String }

#[derive(Deserialize)]
struct IpLinkRec {
    ifname: String,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    addr_info: Option<Vec<IpAddrInfo>>,
}

pub fn from_ip_j_addr(json: &str) -> Option<RadiosReport> {
    let recs: Vec<IpLinkRec> = serde_json::from_str(json).ok()?;
    let mut ifs = Vec::new();
    for r in recs {
        let mut v4 = Vec::new();
        let mut v6 = Vec::new();
        if let Some(addrs) = r.addr_info.as_ref() {
            for a in addrs {
                if a.family == "inet" { v4.push(a.local.clone()); }
                else if a.family == "inet6" { v6.push(a.local.clone()); }
            }
        }
        let mac = r.address.clone();
        let v4_mode = if let Some(first) = v4.get(0) {
            let ip: std::net::Ipv4Addr = first.parse().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
            classify_ipv4(ip).to_string()
        } else { "none".to_string() };
        let v6_mode = if let Some(first) = v6.get(0) {
            let ip: std::net::Ipv6Addr = first.parse().unwrap_or(std::net::Ipv6Addr::UNSPECIFIED);
            classify_ipv6(ip).to_string()
        } else { "none".to_string() };
        ifs.push(RadioIface {
            name: r.ifname.clone(),
            domain: domain_for_name(&r.ifname).to_string(),
            ipv4: v4,
            ipv6: v6,
            mac,
            v4_mode,
            v6_mode,
        });
    }
    Some(RadiosReport { interfaces: ifs, android_props: HashMap::new() })
}

pub fn from_ifconfig_text(text: &str) -> RadiosReport {
    let mut ifs = Vec::new();
    let mut cur_name: Option<String> = None;
    let mut cur_v4: Vec<String> = Vec::new();
    let mut cur_v6: Vec<String> = Vec::new();
    let mut cur_mac: Option<String> = None;

    let flush = |ifs: &mut Vec<RadioIface>, name: &mut Option<String>, v4: &mut Vec<String>, v6: &mut Vec<String>, mac: &mut Option<String>| {
        if let Some(n) = name.take() {
            let v4_mode = if let Some(first) = v4.get(0) {
                let ip: std::net::Ipv4Addr = first.parse().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
                classify_ipv4(ip).to_string()
            } else { "none".to_string() };
            let v6_mode = if let Some(first) = v6.get(0) {
                let ip: std::net::Ipv6Addr = first.parse().unwrap_or(std::net::Ipv6Addr::UNSPECIFIED);
                classify_ipv6(ip).to_string()
            } else { "none".to_string() };
            ifs.push(RadioIface { name: n.clone(), domain: domain_for_name(&n).to_string(), ipv4: std::mem::take(v4), ipv6: std::mem::take(v6), mac: mac.take(), v4_mode, v6_mode });
        }
    };

    for line in text.lines() {
        if let Some(colon) = line.find(':') {
            // Heuristic: a new iface line starts at col 0 and contains flags or <...>
            if !line.starts_with(' ') && !line.starts_with('\t') {
                // Flush previous
                flush(&mut ifs, &mut cur_name, &mut cur_v4, &mut cur_v6, &mut cur_mac);
                cur_name = Some(line[..colon].trim().to_string());
                continue;
            }
        }
        let s = line.trim();
        if let Some(idx) = s.find("inet ") {
            // macOS/Linux common: "inet <ip>" or "inet addr:<ip>"
            let rest = &s[idx+5..];
            let ip = rest.split_whitespace().next().unwrap_or("");
            if !ip.is_empty() { cur_v4.push(ip.to_string()); }
        } else if let Some(idx) = s.find("inet addr:") {
            let rest = &s[idx+10..];
            let ip = rest.split_whitespace().next().unwrap_or("");
            if !ip.is_empty() { cur_v4.push(ip.to_string()); }
        } else if let Some(idx) = s.find("inet6 ") {
            let rest = &s[idx+6..];
            let ip = rest.split_whitespace().next().unwrap_or("");
            if !ip.is_empty() { cur_v6.push(ip.trim_end_matches("%" ).to_string()); }
        } else if let Some(idx) = s.find("inet6 addr:") {
            let rest = &s[idx+11..];
            let ip = rest.split_whitespace().next().unwrap_or("");
            if !ip.is_empty() { cur_v6.push(ip.to_string()); }
        } else if let Some(idx) = s.find("ether ") {
            let rest = &s[idx+6..];
            let mac = rest.split_whitespace().next().unwrap_or("");
            if !mac.is_empty() { cur_mac = Some(mac.to_string()); }
        }
    }
    // Flush last
    flush(&mut ifs, &mut cur_name, &mut cur_v4, &mut cur_v6, &mut cur_mac);
    RadiosReport { interfaces: ifs, android_props: HashMap::new() }
}

// --- Remote parsers (fallback when remote doesn't have litebike) ---

#[derive(Debug, Deserialize)]
struct IpJAddrIface {
    ifname: Option<String>,
    #[serde(default)]
    addr_info: Vec<IpJAddrInfo>,
    #[allow(dead_code)]
    #[serde(default)]
    address: Option<String>, // some variants
}

#[derive(Debug, Deserialize)]
struct IpJAddrInfo {
    family: Option<String>,  // "inet" | "inet6"
    local: Option<String>,
}

pub fn try_parse_ip_j_addr(json: &str) -> Option<RadiosReport> {
    let ifaces: Vec<IpJAddrIface> = serde_json::from_str(json).ok()?;
    let mut out = Vec::new();
    for it in ifaces {
        let name = it.ifname.unwrap_or_else(|| "-".to_string());
        if name == "-" { continue; }
        let mut v4 = Vec::new();
        let mut v6 = Vec::new();
        for ai in it.addr_info {
            match ai.family.as_deref() {
                Some("inet") => if let Some(ip) = ai.local.clone() { v4.push(ip); },
                Some("inet6") => if let Some(ip) = ai.local.clone() { v6.push(ip); },
                _ => {}
            }
        }
        // MAC is not exposed by ip -j addr without -j link; set None
        let v4_mode = v4.get(0)
            .and_then(|s| s.parse().ok())
            .map(|ip| classify_ipv4(ip).to_string())
            .unwrap_or_else(|| "none".to_string());
        let v6_mode = v6.get(0)
            .and_then(|s| s.parse().ok())
            .map(|ip| classify_ipv6(ip).to_string())
            .unwrap_or_else(|| "none".to_string());
        out.push(RadioIface{ name: name.clone(), domain: domain_for_name(&name).to_string(), ipv4: v4, ipv6: v6, mac: None, v4_mode, v6_mode });
    }
    Some(RadiosReport{ interfaces: out, android_props: HashMap::new() })
}

pub fn parse_ifconfig_like(text: &str) -> RadiosReport {
    let mut ifs: HashMap<String, RadioIface> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(colon) = line.find(':') {
            // Interface header like "en0: flags=..."; ensure no leading spaces
            if !line.starts_with(' ') && !line.starts_with('\t') {
                let name = line[..colon].trim().to_string();
                if !name.is_empty() {
                    current = Some(name.clone());
                    ifs.entry(name.clone()).or_insert_with(|| RadioIface{
                        name: name.clone(), domain: domain_for_name(&name).to_string(), ipv4: vec![], ipv6: vec![], mac: None, v4_mode: "none".into(), v6_mode: "none".into()
                    });
                    continue;
                }
            }
        }
        let Some(cur) = current.clone() else { continue };
        let entry = ifs.get_mut(&cur).unwrap();
        let l = line.trim();
        if l.starts_with("inet6 ") {
            let ip = l.split_whitespace().nth(1).unwrap_or("");
            if !ip.is_empty() { entry.ipv6.push(ip.split('%').next().unwrap_or(ip).to_string()); }
        } else if l.starts_with("inet ") {
            let ip = l.split_whitespace().nth(1).unwrap_or("");
            if !ip.is_empty() { entry.ipv4.push(ip.to_string()); }
        } else if l.starts_with("ether ") {
            let mac = l.split_whitespace().nth(1).unwrap_or("");
            if !mac.is_empty() { entry.mac = Some(mac.to_string()); }
        }
    }
    // finalize modes
    let mut list = Vec::new();
    for (_, mut e) in ifs {
        e.v4_mode = e.ipv4.get(0)
            .and_then(|s| s.parse().ok())
            .map(|ip| classify_ipv4(ip).to_string())
            .unwrap_or_else(|| "none".to_string());
        e.v6_mode = e.ipv6.get(0)
            .and_then(|s| s.parse().ok())
            .map(|ip| classify_ipv6(ip).to_string())
            .unwrap_or_else(|| "none".to_string());
        list.push(e);
    }
    RadiosReport { interfaces: list, android_props: HashMap::new() }
}
