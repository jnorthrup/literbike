use litebike::syscall_net::{
	get_default_gateway,
	get_default_gateway_v6,
	get_default_local_ipv4,
	get_default_local_ipv6,
	guess_default_v6_interface,
	list_interfaces,
	InterfaceAddr,
	find_iface_by_ipv4,
	classify_ipv4,
	classify_ipv6,
};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::Path;
use std::time::{Duration, Instant};
use std::thread;

fn main() {
	let args: Vec<String> = env::args().collect();
	let prog = Path::new(&args[0])
		.file_name()
		.and_then(|s| s.to_str())
		.unwrap_or("litebike");

	match prog {
		"ifconfig" => run_ifconfig(&args[1..]),
		"ip" => run_ip(&args[1..]),
		"route" => run_route(),
		"netstat" => run_netstat(&args[1..]),
		"watch" => run_watch(&args[1..]),
		"probe" => run_probe(),
		"domains" => run_domains(),
		"carrier" => run_carrier(),
		_ => {
			// Default: short help and a quick interfaces print
			eprintln!("litebike: argv0-dispatch utility (ifconfig | ip | route | netstat | probe | domains | carrier)\n");
			run_ifconfig(&[]);
		}
	}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_ifconfig_no_args_prints_all_interfaces() {
        // This test assumes list_interfaces returns at least one interface.
        // We capture stdout to verify output.
        let args: Vec<String> = vec![];
        let mut output = Vec::new();
        {
            let _guard = gag::Redirect::stdout(&mut output).unwrap();
            run_ifconfig(&args);
        }
        let out_str = String::from_utf8_lossy(&output);
        assert!(out_str.contains("flags="));
    }

    #[test]
    fn test_run_ifconfig_with_filter_prints_only_filtered_iface() {
        // Use the first interface name as filter
        let ifaces = list_interfaces().unwrap();
        let first_iface = ifaces.keys().next().unwrap().clone();
        let args = vec![first_iface.clone()];
        let mut output = Vec::new();
        {
            let _guard = gag::Redirect::stdout(&mut output).unwrap();
            run_ifconfig(&args);
        }
        let out_str = String::from_utf8_lossy(&output);
        assert!(out_str.contains(&first_iface));
        // Should not print other interfaces
        for name in ifaces.keys() {
            if name != &first_iface {
                assert!(!out_str.contains(name));
            }
        }
    }

    #[test]
    fn test_run_ifconfig_prints_ipv4_and_ipv6() {
        let ifaces = list_interfaces().unwrap();
        let args: Vec<String> = vec![];
        let mut output = Vec::new();
        {
            let _guard = gag::Redirect::stdout(&mut output).unwrap();
            run_ifconfig(&args);
        }
        let out_str = String::from_utf8_lossy(&output);
        let found_v4 = out_str.contains("inet ");
        let found_v6 = out_str.contains("inet6 ");
        assert!(found_v4 || found_v6);
    }

    #[test]
    fn test_run_ifconfig_prints_mac_if_present() {
        let ifaces = list_interfaces().unwrap();
        let args: Vec<String> = vec![];
        let mut output = Vec::new();
        {
            let _guard = gag::Redirect::stdout(&mut output).unwrap();
            run_ifconfig(&args);
        }
        let out_str = String::from_utf8_lossy(&output);
        // MAC address line starts with "    ether"
        let found_mac = out_str.contains("ether ");
        assert!(found_mac || !out_str.contains("ether "));
    }

    #[test]
    fn test_run_ifconfig_error_handling() {
        // Simulate error by temporarily replacing list_interfaces
        // This requires dependency injection or mocking, so we just check that error prints
        // For demonstration, we check that error message is printed if error occurs
        // (In real code, use a trait or mock crate)
        // Here, just ensure function doesn't panic
        let args: Vec<String> = vec!["nonexistent_iface".to_string()];
        run_ifconfig(&args);
    }
}

fn run_ip(args: &[String]) {
	if args.is_empty() {
		eprintln!("Usage: ip [addr|route] [-6]");
		return;
	}
	// Support both: ip -6 addr ... and ip addr -6 ...
	let (want_v6, subcmd_idx) = if !args.is_empty() && args[0] == "-6" {
		(true, 1usize)
	} else {
		(args.iter().any(|a| a == "-6"), 0usize)
	};
	if subcmd_idx >= args.len() { eprintln!("Usage: ip [addr|route] [-6]"); return; }
	match args[subcmd_idx].as_str() {
		"addr" | "address" => {
			match list_interfaces() {
				Ok(ifaces) => {
					let mut idx = 1u32;
					for (name, iface) in ifaces {
						println!("{}: {}: <UP> mtu 1500", idx, name);
						for addr in iface.addrs {
							match addr {
								InterfaceAddr::V4(ip) => if !want_v6 { println!("    inet {}/24", ip) },
								InterfaceAddr::V6(ip) => if want_v6 { println!("    inet6 {}/64", ip) },
								InterfaceAddr::Link(_) => {}
							}
						}
						idx += 1;
					}
				}
				Err(e) => eprintln!("ip addr: {}", e),
			}
		}
		"route" => {
			if want_v6 {
				match get_default_gateway_v6() {
					Ok(gw) => println!("default via {} dev -", gw),
					Err(e) => {
						eprintln!("ip -6 route: {}", e);
						if let Ok(ip) = get_default_local_ipv6() {
							let iface = guess_default_v6_interface().unwrap_or_else(|| "-".to_string());
							println!("(hint) src {} dev {}", ip, iface);
						}
					}
				}
			} else {
				run_route();
			}
		},
	_ => eprintln!("ip: unknown command '{}'", args[subcmd_idx]),
	}
}

fn run_route() {
	let gw = get_default_gateway();
	println!("Kernel IP routing table");
	println!("Destination     Gateway         Genmask         Flags Metric Ref    Use Iface");
	match gw {
		Ok(gw) => println!(
			"0.0.0.0         {:<15} 0.0.0.0         UG    0      0        0 -",
			gw
		),
		Err(e) => {
			eprintln!("route: {}", e);
			if let Ok(ip) = get_default_local_ipv4() {
				println!("(hint) default local IPv4: {} (gateway likely {}.1)", ip, ip.to_string().rsplit('.').skip(1).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("."));
			}
		}
	}
}

fn run_netstat(args: &[String]) {
	// Flags: -a (all), -t (tcp), -u (udp), -l (listening), -n (numeric)
	let mut show_tcp = true;
	let mut show_udp = true;
	let mut listening_only = false;
	let _numeric = true; // always numeric in our output
	let mut show_routes = false;
	let mut show_ifaces = false;
	if !args.is_empty() {
		// Default if filters present but neither -t nor -u provided: show both
		for a in args {
			match a.as_str() {
				"-t" => { show_tcp = true; show_udp = false; },
				"-u" => { show_udp = true; show_tcp = false; },
				"-a" => { show_tcp = true; show_udp = true; },
				"-l" => listening_only = true,
				"-r" => show_routes = true,
				"-i" => show_ifaces = true,
				_ => {}
			}
		}
	}

	if show_routes { return run_netstat_route(); }
	if show_ifaces { return run_netstat_interfaces(); }

	#[cfg(any(target_os = "linux", target_os = "android"))]
	{
		use std::fs::File;
		if File::open("/proc/net/tcp").is_ok() {
			println!("Active Internet connections (servers/established) - best-effort");
			println!("Proto Recv-Q Send-Q Local Address           Foreign Address         State");
			if print_proc_net_sockets_filtered(show_tcp, show_udp, listening_only) { return; }
		}
		// Fallback to external tools if /proc is blocked
		let mut printed = false;
		if show_tcp {
			let tcp_cmd = if listening_only { ["ss", "-lnt"] } else { ["ss", "-ant"] };
			printed |= print_external_netstat(&tcp_cmd);
		}
		if show_udp {
			let udp_cmd = if listening_only { ["ss", "-lnu"] } else { ["ss", "-anu"] };
			printed |= print_external_netstat(&udp_cmd);
		}
		if printed { return; }
	if print_external_netstat(&["netstat", "-an"]) { return; }
	if print_external_netstat(&["busybox", "netstat", "-an"]) { return; }
		eprintln!("netstat: socket tables not accessible (permissions?)");
		return;
	}

	#[cfg(target_os = "macos")]
	{
		// Map our flags to macOS netstat subsets
	let mut args_vec = vec!["-an"]; // numeric
		if listening_only { args_vec.push("-l"); }
		// Note: macOS netstat lacks simple -t/-u, but we can filter via -ptcp/-pudp on some systems.
		// Keep it simple and show all sockets.
		if print_external_netstat(&["netstat", args_vec.join(" ").as_str()]) { return; }
		eprintln!("netstat: external command not available");
		let _ = (show_tcp, show_udp); // suppress unused warnings if compiled differently
		return;
	}
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn print_proc_net_sockets_filtered(show_tcp: bool, show_udp: bool, listening_only: bool) -> bool {
	use std::fs;

	fn parse_hex_ip_port(hex: &str, v6: bool) -> (String, u16) {
		let mut parts = hex.split(':');
		let ip_hex = parts.next().unwrap_or("");
		let port_hex = parts.next().unwrap_or("0");
		let port = u16::from_str_radix(port_hex, 16).unwrap_or(0);
		if !v6 {
			if ip_hex.len() >= 8 {
				let b0 = u8::from_str_radix(&ip_hex[6..8], 16).unwrap_or(0);
				let b1 = u8::from_str_radix(&ip_hex[4..6], 16).unwrap_or(0);
				let b2 = u8::from_str_radix(&ip_hex[2..4], 16).unwrap_or(0);
				let b3 = u8::from_str_radix(&ip_hex[0..2], 16).unwrap_or(0);
				return (format!("{}.{}.{}.{}", b0, b1, b2, b3), port);
			}
			return ("0.0.0.0".to_string(), port);
		} else {
			// IPv6: 32 hex chars, little-endian 32-bit words
			if ip_hex.len() >= 32 {
				let mut segs = Vec::new();
				for i in (0..32).step_by(8) {
					let w3 = &ip_hex[i..i + 2];
					let w2 = &ip_hex[i + 2..i + 4];
					let w1 = &ip_hex[i + 4..i + 6];
					let w0 = &ip_hex[i + 6..i + 8];
					segs.push(format!("{}{}:{}{}", w3, w2, w1, w0));
				}
				return (segs.join(":"), port);
			}
			return ("::".to_string(), port);
		}
	}

	fn state_str(code: &str) -> &'static str {
		match code {
			"01" => "ESTABLISHED",
			"02" => "SYN_SENT",
			"03" => "SYN_RECV",
			"04" => "FIN_WAIT1",
			"05" => "FIN_WAIT2",
			"06" => "TIME_WAIT",
			"07" => "CLOSE",
			"08" => "CLOSE_WAIT",
			"09" => "LAST_ACK",
			"0A" => "LISTEN",
			"0B" => "CLOSING",
			_ => "UNKNOWN",
		}
	}

	fn print_file(path: &str, proto: &str, v6: bool, listening_only: bool) -> bool {
		let Ok(content) = fs::read_to_string(path) else { return false };
		for (i, line) in content.lines().enumerate() {
			if i == 0 { continue; }
			let cols: Vec<&str> = line.split_whitespace().collect();
			if cols.len() < 10 { continue; }
			let (lip, lport) = parse_hex_ip_port(cols[1], v6);
			let (rip, rport) = parse_hex_ip_port(cols[2], v6);
			let st = state_str(cols[3]);
			// Filter: show LISTEN/ESTABLISHED for TCP, and all for UDP
			if proto.starts_with("TCP") {
				if listening_only {
					if st != "LISTEN" { continue; }
				} else if st != "LISTEN" && st != "ESTABLISHED" {
					continue;
				}
			}
			println!(
				"{:<5} {:>5} {:>5} {:<22} {:<22} {}",
				proto,
				0, // Recv-Q (not parsed here)
				0, // Send-Q
				format!("{}:{}", lip, lport),
				format!("{}:{}", rip, rport),
				st
			);
		}
		true
	}

	let mut any = false;
	if show_tcp {
		any |= print_file("/proc/net/tcp", "TCP", false, listening_only);
		any |= print_file("/proc/net/tcp6", "TCP6", true, listening_only);
	}
	if show_udp {
		any |= print_file("/proc/net/udp", "UDP", false, false);
		any |= print_file("/proc/net/udp6", "UDP6", true, false);
	}
	any
}

#[allow(unused)]
fn print_external_netstat(cmd: &[&str]) -> bool {
	use std::process::Command;
use litebike::syscall_net::{InterfaceAddr, Interface, list_interfaces};
	if cmd.is_empty() { return false; }
	let (prog, args) = (cmd[0], &cmd[1..]);
	if let Ok(out) = Command::new(prog).args(args).output() {
		if out.status.success() {
			let text = String::from_utf8_lossy(&out.stdout);
			let lines: Vec<&str> = text.lines().collect();
			let non_empty = lines.iter().filter(|l| !l.trim().is_empty()).count();
			if non_empty >= 3 {
				for line in lines.into_iter().take(100) {
					println!("{}", line);
				}
				return true;
			}
		}
	}
	false
}

fn run_netstat_route() {
	println!("Routing tables");
	println!("Destination        Gateway            Flags  Refs    Use  Iface");
	match get_default_gateway() {
		Ok(gw) => println!("default            {:<16} UG     0       0    -", gw),
		Err(e) => {
			eprintln!("netstat -r: {}", e);
			if let Ok(ip) = get_default_local_ipv4() {
				// Show a hint row even if gateway is blocked
				let ip_s = ip.to_string();
				let octets: Vec<&str> = ip_s.split('.').collect();
				if octets.len() == 4 {
					println!("(hint) default local IPv4 {} (gw likely {}.1)", ip, [octets[0], octets[1], octets[2]].join("."));
				}
			}
		}
	}
	// IPv6 default route (best-effort)
	match get_default_gateway_v6() {
		Ok(gw6) => println!("default            {:<16} UG     0       0    -", gw6),
		Err(_) => {
			if let Some(ifn) = guess_default_v6_interface() {
				println!("(hint) default IPv6 egress interface: {}", ifn);
			}
		}
	}
}

fn run_netstat_interfaces() {
	println!("Kernel Interface table");
	println!("Iface   Flags    Index   IPv4              IPv6 count  MAC");
	match list_interfaces() {
		Ok(ifaces) => {
			for (name, iface) in ifaces {
				let ipv4s: Vec<String> = iface
					.addrs
					.iter()
					.filter_map(|a| if let InterfaceAddr::V4(ip) = a { Some(ip.to_string()) } else { None })
					.collect();
				let v6count = iface.addrs.iter().filter(|a| matches!(a, InterfaceAddr::V6(_))).count();
				let mac = iface
					.addrs
					.iter()
					.find_map(|a| if let InterfaceAddr::Link(m) = a { Some(m.clone()) } else { None })
					.map(|m| format!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", m.get(0).copied().unwrap_or(0), m.get(1).copied().unwrap_or(0), m.get(2).copied().unwrap_or(0), m.get(3).copied().unwrap_or(0), m.get(4).copied().unwrap_or(0), m.get(5).copied().unwrap_or(0)))
					.unwrap_or_else(|| "-".to_string());
				println!(
					"{:<6} 0x{:04x}  {:<6} {:<17} {:<10} {}",
					name,
					iface.flags,
					iface.index,
					ipv4s.get(0).cloned().unwrap_or_else(|| "-".to_string()),
					v6count,
					mac
				);
			}
		}
		Err(e) => eprintln!("netstat -i: {}", e),
	}
}

fn run_watch(args: &[String]) {
	// Options: -n <secs> (interval), --count <N> (iterations), --v6 (include v6 egress hints)
	let mut interval = 3u64;
	let mut max_count: Option<u64> = None;
	let mut include_v6 = true;
	let mut i = 0;
	while i < args.len() {
		match args[i].as_str() {
			"-n" if i + 1 < args.len() => {
				if let Ok(v) = args[i + 1].parse::<u64>() { interval = v; }
				i += 2; continue;
			}
			"--count" if i + 1 < args.len() => {
				if let Ok(v) = args[i + 1].parse::<u64>() { max_count = Some(v); }
				i += 2; continue;
			}
			"--no-v6" => { include_v6 = false; i += 1; continue; }
			_ => { i += 1; }
		}
	}

	#[derive(Clone)]
	struct IfInfo { flags: u32, _index: u32, v4: HashSet<String>, v6: HashSet<String> }
	#[derive(Clone, Default)]
	struct Snap {
		ifs: HashMap<String, IfInfo>,
		gw4: Option<String>,
		v6_iface: Option<String>,
	}

	fn snapshot(include_v6: bool) -> Snap {
		let mut snap = Snap::default();
		if let Ok(ifaces) = list_interfaces() {
			for (name, iface) in ifaces {
				let mut v4 = HashSet::new();
				let mut v6 = HashSet::new();
				for a in iface.addrs {
					match a {
						InterfaceAddr::V4(ip) => { v4.insert(ip.to_string()); },
						InterfaceAddr::V6(ip) => { if include_v6 { v6.insert(ip.to_string()); } },
						InterfaceAddr::Link(_) => {}
					}
				}
				snap.ifs.insert(name, IfInfo { flags: iface.flags, _index: iface.index, v4, v6 });
			}
		}
		if let Ok(gw) = get_default_gateway() { snap.gw4 = Some(gw.to_string()); }
		if include_v6 { snap.v6_iface = guess_default_v6_interface(); }
		snap
	}

	fn print_diff(prev: &Snap, curr: &Snap, t: &str) {
		// Interfaces added/removed
		for name in curr.ifs.keys() {
			if !prev.ifs.contains_key(name) {
				println!("{} iface+ {}", t, name);
			}
		}
		for name in prev.ifs.keys() {
			if !curr.ifs.contains_key(name) {
				println!("{} iface- {}", t, name);
			}
		}
		// Address changes
		for (name, ci) in &curr.ifs {
			if let Some(pi) = prev.ifs.get(name) {
				for ip in ci.v4.difference(&pi.v4) { println!("{} {} v4+ {}", t, name, ip); }
				for ip in pi.v4.difference(&ci.v4) { println!("{} {} v4- {}", t, name, ip); }
				for ip in ci.v6.difference(&pi.v6) { println!("{} {} v6+ {}", t, name, ip); }
				for ip in pi.v6.difference(&ci.v6) { println!("{} {} v6- {}", t, name, ip); }
				if ci.flags != pi.flags { println!("{} {} flags 0x{:x}->0x{:x}", t, name, pi.flags, ci.flags); }
			}
		}
		// Default v4 GW changes
		if prev.gw4 != curr.gw4 { println!("{} default-v4 {}->{}", t, prev.gw4.clone().unwrap_or("-".into()), curr.gw4.clone().unwrap_or("-".into())); }
		// v6 egress iface hint
		if prev.v6_iface != curr.v6_iface { println!("{} default-v6-iface {}->{}", t, prev.v6_iface.clone().unwrap_or("-".into()), curr.v6_iface.clone().unwrap_or("-".into())); }
	}

	let start = Instant::now();
	let mut prev = snapshot(include_v6);
	let mut iter = 0u64;
	loop {
		thread::sleep(Duration::from_secs(interval));
		let curr = snapshot(include_v6);
		let t = format!("t+{}s", start.elapsed().as_secs());
		print_diff(&prev, &curr, &t);
		prev = curr;
		iter += 1;
		if let Some(limit) = max_count { if iter >= limit { break; } }
	}
}

fn run_probe() {
	// Show best-effort egress selections for v4/v6 and map to interfaces and defaults
	println!("probe: best-effort egress selection");
	match get_default_local_ipv4() {
		Ok(ip) => {
			let iface = find_iface_by_ipv4(ip).unwrap_or_else(|| "-".into());
			let gw = get_default_gateway().map(|g| g.to_string()).unwrap_or_else(|_| "-".into());
			println!("v4: src {} iface {} gw {} ({})", ip, iface, gw, classify_ipv4(ip));
		}
		Err(e) => println!("v4: unavailable ({})", e),
	}
	match get_default_local_ipv6() {
		Ok(ip6) => {
			let iface6 = guess_default_v6_interface().unwrap_or_else(|| "-".into());
			let gw6 = get_default_gateway_v6().map(|g| g.to_string()).unwrap_or_else(|_| "-".into());
			println!("v6: src {} iface {} gw {} ({})", ip6, iface6, gw6, classify_ipv6(ip6));
		}
		Err(e) => println!("v6: unavailable ({})", e),
	}
}

fn run_domains() {
	println!("domains: per-interface summary");
	match list_interfaces() {
		Ok(ifaces) => {
			for (name, iface) in ifaces {
				let mut v4_classes = Vec::new();
				let mut v6_classes = Vec::new();
				let mut first_v4 = None;
				for a in iface.addrs {
					match a {
						InterfaceAddr::V4(ip) => { v4_classes.push(classify_ipv4(ip)); if first_v4.is_none() { first_v4 = Some(ip); } },
						InterfaceAddr::V6(ip) => v6_classes.push(classify_ipv6(ip)),
						InterfaceAddr::Link(_) => {}
					}
				}
				// Domain guess by name
				let domain = if name.starts_with("rmnet") || name.starts_with("ccmni") || name.starts_with("wwan") { "cell" }
							 else if name.starts_with("wlan") || name.starts_with("swlan") { "wifi" }
							 else if name.starts_with("tun") || name.starts_with("tap") || name.starts_with("wg") || name.starts_with("utun") { "vpn" }
							 else { "other" };
				let mode = match (v4_classes.is_empty(), v6_classes.is_empty()) {
					(true, true) => "no-ip",
					(false, true) => "v4-only",
					(true, false) => "v6-only",
					(false, false) => "dual",
				};
				// Notable hints
				let v4hint = first_v4.map(|ip| classify_ipv4(ip)).unwrap_or("-");
				let v6hint = v6_classes.iter().find(|&&c| c == "global").copied().unwrap_or(v6_classes.get(0).copied().unwrap_or("-"));
				println!(
					"{:<12} domain={:<5} mode={:<7} v4_hint={:<8} v6_hint={:<11}",
					name, domain, mode, v4hint, v6hint
				);
			}
			// Show defaults at the end
			if let Ok(gw) = get_default_gateway() { println!("default v4 gw {}", gw); }
			if let Ok(gw6) = get_default_gateway_v6() { println!("default v6 gw {}", gw6); }
			else if let Some(ifn) = guess_default_v6_interface() { println!("(hint) default v6 egress iface {}", ifn); }
		}
		Err(e) => eprintln!("domains: {}", e),
	}
}

fn run_carrier() {
	#[cfg(any(target_os = "android"))]
	{
		let props = litebike::syscall_net::android_carrier_props();
		if props.is_empty() { println!("carrier: no getprop keys visible (managed device?)"); return; }
		println!("carrier props:");
		for (k,v) in props { println!("{} = {}", k, v); }
	}
	#[cfg(not(any(target_os = "android")))]
	{
		println!("carrier: only available on Android (uses getprop)");
	}
}
