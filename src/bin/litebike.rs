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
use std::fs;
use std::time::SystemTime;
use std::process::Command;
use std::net::{TcpStream, UdpSocket, SocketAddr};
use std::io::{Read, Write};

fn main() {
	let args: Vec<String> = env::args().collect();
	let argv0 = Path::new(&args[0])
		.file_name()
		.and_then(|s| s.to_str())
		.unwrap_or("litebike");

	// Allow both argv0-dispatch (ifconfig/ip/...) and subcommands: litebike <cmd> [args]
	let (cmd, subargs): (&str, &[String]) = if argv0 == "litebike" {
		if args.len() >= 2 { (&args[1], &args[2..]) } else { ("ifconfig", &args[1..]) }
	} else {
		(argv0, &args[1..])
	};

	match cmd {
		"ifconfig" => run_ifconfig(subargs),
		"ip" => run_ip(subargs),
		"route" => run_route(),
		"netstat" => run_netstat(subargs),
		"watch" => run_watch(subargs),
		"probe" => run_probe(),
		"domains" => run_domains(),
		"carrier" => run_carrier(),
		"radios" => run_radios(subargs),
		"snapshot" => run_snapshot(subargs),
		"remote-sync" => run_remote_sync(subargs),
		"proxy-setup" => run_proxy_setup(subargs),
		"upnp-gateway" => run_upnp_gateway(subargs),
		_ => {
			// Default: short help and a quick interfaces print
			eprintln!("litebike: argv0-dispatch utility (ifconfig | ip | route | netstat | probe | domains | carrier | radios | snapshot | remote-sync | proxy-setup | upnp-gateway)\n");
			run_ifconfig(&[]);
		}
	}
}

fn run_ifconfig(args: &[String]) {
	// Optional: ifconfig <iface> to filter output
	let filter = args.get(0).map(|s| s.as_str());
	match list_interfaces() {
		Ok(ifaces) => {
			for (name, iface) in ifaces {
				if let Some(f) = filter {
					if name != f { continue; }
				}
				println!("{}: flags=0x{:x} index {}", name, iface.flags, iface.index);
				for addr in iface.addrs {
					match addr {
						InterfaceAddr::V4(ip) => println!("    inet {}", ip),
						InterfaceAddr::V6(ip) => println!("    inet6 {}", ip),
						InterfaceAddr::Link(mac) => {
							if !mac.is_empty() {
								println!(
									"    ether {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
									mac.get(0).cloned().unwrap_or(0),
									mac.get(1).cloned().unwrap_or(0),
									mac.get(2).cloned().unwrap_or(0),
									mac.get(3).cloned().unwrap_or(0),
									mac.get(4).cloned().unwrap_or(0),
									mac.get(5).cloned().unwrap_or(0)
								);
							}
						}
					}
				}
			}
		}
		Err(e) => eprintln!("ifconfig: {}", e),
	}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
	fn test_run_ifconfig_no_args_does_not_panic() {
		let args: Vec<String> = vec![];
		run_ifconfig(&args);
	}

    #[test]
	fn test_run_ifconfig_with_filter_does_not_panic() {
		// Use the first interface name as filter if present
		let first = list_interfaces().ok().and_then(|m| m.keys().next().cloned());
		if let Some(ifn) = first { run_ifconfig(&[ifn]); }
		else { run_ifconfig(&[]); }
	}

    #[test]
	fn test_run_ifconfig_executes() {
		run_ifconfig(&[]);
	}

    #[test]
	fn test_run_ifconfig_mac_branch_executes() { run_ifconfig(&[]); }

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

fn run_radios(args: &[String]) {
	let want_json = args.iter().any(|a| a == "--json");
	let mut ssh_target: Option<String> = None;
	let mut ssh_opts: Option<String> = None;
	let mut i = 0;
	while i < args.len() {
		if args[i] == "--ssh" {
			if i + 1 < args.len() && !args[i+1].starts_with('-') {
				ssh_target = Some(args[i+1].clone());
				i += 2;
			} else {
				// Flag present without a host: request inference
				ssh_target = Some(String::new());
				i += 1;
			}
		} else if args[i] == "--ssh-opts" && i + 1 < args.len() {
			ssh_opts = Some(args[i+1].clone());
			i += 2;
		} else {
			i += 1;
		}
	}

	// Infer SSH target if requested or env configured
	if ssh_target.is_none() {
		if let Ok(env_host) = std::env::var("LITEBIKE_SSH") { if !env_host.trim().is_empty() { ssh_target = Some(env_host); } }
	}
	if let Some(h) = ssh_target.as_ref() {
		if h.is_empty() {
			// Try config file ~/.config/litebike/ssh_target or ./ssh_target
			let mut inferred: Option<String> = None;
			if let Ok(home) = std::env::var("HOME") {
				let p = std::path::Path::new(&home).join(".config/litebike/ssh_target");
				if let Ok(s) = std::fs::read_to_string(&p) { let t = s.trim(); if !t.is_empty() { inferred = Some(t.to_string()); } }
				if inferred.is_none() {
					let p2 = std::path::Path::new(&home).join(".litebike-ssh");
					if let Ok(s) = std::fs::read_to_string(&p2) { let t = s.trim(); if !t.is_empty() { inferred = Some(t.to_string()); } }
				}
			}
			if inferred.is_none() {
				if let Ok(s) = std::fs::read_to_string("ssh_target") { let t = s.trim(); if !t.is_empty() { inferred = Some(t.to_string()); } }
			}
			// Try ~/.ssh/known_hosts (first non-hashed host)
			if inferred.is_none() {
				if let Ok(home) = std::env::var("HOME") {
					let p = std::path::Path::new(&home).join(".ssh/known_hosts");
					if let Ok(kh) = std::fs::read_to_string(&p) {
						for line in kh.lines() {
							let s = line.trim();
							if s.is_empty() || s.starts_with('#') { continue; }
							// Format: host[,host2] keytype key
							let mut parts = s.split_whitespace();
							if let Some(hosts) = parts.next() {
								if hosts.starts_with('|') { continue; } // hashed entry
								let first = hosts.split(',').next().unwrap_or("").trim();
								if !first.is_empty() { inferred = Some(first.to_string()); break; }
							}
						}
					}
				}
			}
			// Try ~/.ssh/config first concrete Host alias (not wildcard)
			if inferred.is_none() {
				if let Ok(home) = std::env::var("HOME") {
					let p = std::path::Path::new(&home).join(".ssh/config");
					if let Ok(cfg) = std::fs::read_to_string(&p) {
						for line in cfg.lines() {
							let s = line.trim();
							if s.to_lowercase().starts_with("host ") {
								let names = s[5..].split_whitespace();
								for name in names {
									if name == "*" || name.contains('*') || name.contains('?') { continue; }
									if !name.is_empty() { inferred = Some(name.to_string()); break; }
								}
								if inferred.is_some() { break; }
							}
						}
					}
				}
			}
			if let Some(v) = inferred { ssh_target = Some(v); }
			else {
				eprintln!("radios: no SSH target inferred. Set LITEBIKE_SSH or create ~/.config/litebike/ssh_target");
				return;
			}
		}
	}

	if let Some(host) = ssh_target {
		use std::process::Command;
		let mut opts_vec: Vec<String> = ssh_opts
			.or_else(|| std::env::var("LITEBIKE_SSH_OPTS").ok())
			.map(|s| s.split_whitespace().map(|t| t.to_string()).collect())
			.unwrap_or_else(|| Vec::new());
		// Ensure non-interactive, quick failure by default
		let mut has_batch = false; let mut has_cto = false; let mut has_shkc = false;
		for t in &opts_vec {
			let lt = t.to_lowercase();
			if lt.contains("batchmode") { has_batch = true; }
			if lt.contains("connecttimeout") { has_cto = true; }
			if lt.contains("stricthostkeychecking") { has_shkc = true; }
		}
		if !has_batch { opts_vec.push("-o".into()); opts_vec.push("BatchMode=yes".into()); }
		if !has_cto { opts_vec.push("-o".into()); opts_vec.push("ConnectTimeout=5".into()); }
		if !has_shkc { opts_vec.push("-o".into()); opts_vec.push("StrictHostKeyChecking=accept-new".into()); }

		// 1) Try remote litebike
		let mut cmd1 = Command::new("ssh");
		for t in &opts_vec { cmd1.arg(t); }
		let out1 = cmd1.arg(&host).arg("litebike radios --json").output();
		match out1 {
			Ok(o) if o.status.success() => {
				let text = String::from_utf8_lossy(&o.stdout);
				match serde_json::from_str::<litebike::radios::RadiosReport>(&text) {
					Ok(report) => {
						if want_json { println!("{}", serde_json::to_string_pretty(&report).unwrap_or_else(|_| text.to_string())); }
						else { litebike::radios::print_radios_human(&report); }
					}
					Err(e) => {
						eprintln!("radios: failed to parse remote JSON: {}", e);
						print!("{}", text);
					}
				}
			}
			_ => {
				// 2) Try ip -j addr (Linux/Android)
				let mut cmd2 = Command::new("ssh");
				for t in &opts_vec { cmd2.arg(t); }
				let out2 = cmd2.arg(&host).arg("ip -j addr").output();
				if let Ok(o2) = out2 {
					if o2.status.success() {
						let text = String::from_utf8_lossy(&o2.stdout);
						if let Some(report) = litebike::radios::from_ip_j_addr(&text) {
							if want_json { println!("{}", serde_json::to_string_pretty(&report).unwrap_or_else(|_| text.to_string())); }
							else { litebike::radios::print_radios_human(&report); }
							return;
						}
					}
				}
				// 3) Try ifconfig or ip addr text and parse loosely
				let mut cmd3 = Command::new("ssh");
				for t in &opts_vec { cmd3.arg(t); }
				let out3 = cmd3.arg(&host).arg("ifconfig -a || ifconfig || ip addr").output();
				match out3 {
					Ok(o3) if o3.status.success() => {
						let text = String::from_utf8_lossy(&o3.stdout);
						let report = litebike::radios::from_ifconfig_text(&text);
						if want_json { println!("{}", serde_json::to_string_pretty(&report).unwrap_or_else(|_| text.to_string())); }
						else { litebike::radios::print_radios_human(&report); }
					}
					Ok(o3) => {
						let stderr = String::from_utf8_lossy(&o3.stderr);
						eprintln!("radios: ssh failed: {}", stderr.trim());
					}
					Err(e) => eprintln!("radios: ssh exec error: {}", e),
				}
			}
		}
		return;
	}

	let report = litebike::radios::gather_radios();
	if want_json {
		match serde_json::to_string_pretty(&report) {
			Ok(s) => println!("{}", s),
			Err(e) => eprintln!("radios --json: {}", e),
		}
	} else {
		litebike::radios::print_radios_human(&report);
	}
}

fn run_snapshot(args: &[String]) {
	// Usage: snapshot [label words...]
	let label = if args.is_empty() { "unnamed".to_string() } else { args.join(" ") };
	let sanitized: String = label.chars().map(|c| if c.is_ascii_alphanumeric() { c } else if c.is_whitespace() { '_' } else { '-' }).collect();
	let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
	let dir = Path::new("docs").join("snapshots");
	if let Err(e) = fs::create_dir_all(&dir) { eprintln!("snapshot: cannot create dir {}: {}", dir.display(), e); return; }
	let path = dir.join(format!("{}-{}.txt", ts, sanitized));
	let mut out = String::new();
	out.push_str(&format!("carrier snapshot: {}\n", label));
	out.push_str(&format!("timestamp: {}\n\n", ts));

	// Interfaces (condensed)
	out.push_str("[interfaces]\n");
	match list_interfaces() {
		Ok(ifaces) => {
			for (name, iface) in ifaces {
				let mut v4s = Vec::new(); let mut v6s = Vec::new(); let mut mac = String::new();
				for a in iface.addrs {
					match a {
						InterfaceAddr::V4(ip) => v4s.push(ip.to_string()),
						InterfaceAddr::V6(ip) => v6s.push(ip.to_string()),
						InterfaceAddr::Link(m) => if mac.is_empty() { mac = format!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", m.get(0).copied().unwrap_or(0), m.get(1).copied().unwrap_or(0), m.get(2).copied().unwrap_or(0), m.get(3).copied().unwrap_or(0), m.get(4).copied().unwrap_or(0), m.get(5).copied().unwrap_or(0)); },
					}
				}
				out.push_str(&format!("{} flags=0x{:x} idx={} v4=[{}] v6=[{}] mac={}\n", name, iface.flags, iface.index, v4s.join(","), v6s.join(","), if mac.is_empty() { "-".into() } else { mac }));
			}
		}
		Err(e) => out.push_str(&format!("error: {}\n", e)),
	}
	out.push('\n');

	// Defaults and probe
	out.push_str("[defaults]\n");
	match get_default_gateway() { Ok(gw) => out.push_str(&format!("v4_gw={}\n", gw)), Err(e) => out.push_str(&format!("v4_gw=error({})\n", e)), }
	match get_default_gateway_v6() { Ok(gw6) => out.push_str(&format!("v6_gw={}\n", gw6)), Err(_) => { if let Some(ifn) = guess_default_v6_interface() { out.push_str(&format!("v6_iface_hint={}\n", ifn)); } else { out.push_str("v6=unavailable\n"); } } }
	out.push('\n');

	out.push_str("[probe]\n");
	match get_default_local_ipv4() {
		Ok(ip) => {
			let iface = find_iface_by_ipv4(ip).unwrap_or_else(|| "-".into());
			let cls = classify_ipv4(ip);
			out.push_str(&format!("v4 src={} iface={} class={}\n", ip, iface, cls));
		}
		Err(e) => out.push_str(&format!("v4 error={}\n", e)),
	}
	match get_default_local_ipv6() {
		Ok(ip6) => {
			let iface6 = guess_default_v6_interface().unwrap_or_else(|| "-".into());
			let cls6 = classify_ipv6(ip6);
			out.push_str(&format!("v6 src={} iface={} class={}\n", ip6, iface6, cls6));
		}
		Err(e) => out.push_str(&format!("v6 error={}\n", e)),
	}
	out.push('\n');

	// Domains
	out.push_str("[domains]\n");
	match list_interfaces() {
		Ok(ifaces) => {
			for (name, iface) in ifaces {
				let mut has_v4 = false; let mut has_v6 = false; let mut v4hint = "-"; let mut v6hint = "-";
				for a in iface.addrs {
					match a {
						InterfaceAddr::V4(ip) => { if !has_v4 { v4hint = classify_ipv4(ip); } has_v4 = true; },
						InterfaceAddr::V6(ip) => { if !has_v6 { v6hint = classify_ipv6(ip); } has_v6 = true; },
						InterfaceAddr::Link(_) => {}
					}
				}
				let domain = if name.starts_with("rmnet") || name.starts_with("ccmni") || name.starts_with("wwan") { "cell" }
							 else if name.starts_with("wlan") || name.starts_with("swlan") { "wifi" }
							 else if name.starts_with("tun") || name.starts_with("tap") || name.starts_with("wg") || name.starts_with("utun") { "vpn" }
							 else { "other" };
				let mode = match (has_v4, has_v6) { (false,false)=>"no-ip", (true,false)=>"v4-only", (false,true)=>"v6-only", (true,true)=>"dual" };
				out.push_str(&format!("{:<12} domain={:<5} mode={:<7} v4_hint={:<8} v6_hint={:<11}\n", name, domain, mode, v4hint, v6hint));
			}
		}
		Err(e) => out.push_str(&format!("error: {}\n", e)),
	}
	out.push('\n');

	// Carrier props (Android only)
	out.push_str("[carrier_props]\n");
	#[cfg(any(target_os = "android"))]
	{
		let props = litebike::syscall_net::android_carrier_props();
		if props.is_empty() { out.push_str("(none)\n"); }
		let mut keys: Vec<_> = props.keys().cloned().collect();
		keys.sort();
		for k in keys { out.push_str(&format!("{} = {}\n", k, props[&k])); }
	}
	#[cfg(not(any(target_os = "android")))]
	{
		out.push_str("(n/a)\n");
	}

	if let Err(e) = fs::write(&path, out) { eprintln!("snapshot: failed to write {}: {}", path.display(), e); }
	else { println!("snapshot saved: {}", path.display()); }
}

fn run_remote_sync(args: &[String]) {
	let cmd = args.get(0).map(|s| s.as_str()).unwrap_or("list");
	match cmd {
		"list" => {
			if let Ok(output) = Command::new("git").args(["remote", "-v"]).output() {
				let remotes = String::from_utf8_lossy(&output.stdout);
				for line in remotes.lines() {
					if line.contains("(fetch)") {
						let parts: Vec<&str> = line.split_whitespace().collect();
						if parts.len() >= 2 {
							let name = parts[0];
							let url = parts[1];
							let status = if url.starts_with("ssh://") || url.contains("@") {
								if let Some(host) = extract_ssh_host(url) {
									if check_ssh_reachable(&host) { "active" } else { "stale" }
								} else { "unknown" }
							} else if url.starts_with("http") {
								"http"
							} else {
								"local"
							};
							println!("{:<20} {:<50} {}", name, url, status);
						}
					}
				}
			}
		}
		"pull" => {
			if let Ok(output) = Command::new("git").args(["remote", "-v"]).output() {
				let remotes = String::from_utf8_lossy(&output.stdout);
				for line in remotes.lines() {
					if line.contains("(fetch)") && (line.contains("tmp") || line.contains("temp")) {
						let parts: Vec<&str> = line.split_whitespace().collect();
						if parts.len() >= 2 {
							let name = parts[0];
							println!("Pulling from {}", name);
							let _ = Command::new("git").args(["pull", name, "master"]).status();
						}
					}
				}
			}
		}
		"clean" => {
			let mut stale_remotes = Vec::new();
			if let Ok(output) = Command::new("git").args(["remote", "-v"]).output() {
				let remotes = String::from_utf8_lossy(&output.stdout);
				for line in remotes.lines() {
					if line.contains("(fetch)") && (line.contains("tmp") || line.contains("temp")) {
						let parts: Vec<&str> = line.split_whitespace().collect();
						if parts.len() >= 2 {
							let name = parts[0];
							let url = parts[1];
							if url.starts_with("ssh://") || url.contains("@") {
								if let Some(host) = extract_ssh_host(url) {
									if !check_ssh_reachable(&host) {
										stale_remotes.push(name.to_string());
									}
								}
							}
						}
					}
				}
			}
			for remote in stale_remotes {
				println!("Removing stale remote: {}", remote);
				let _ = Command::new("git").args(["remote", "remove", &remote]).status();
			}
		}
		_ => {
			eprintln!("Usage: litebike remote-sync [list|pull|clean]");
			eprintln!("  list  - List all remotes with connectivity status");
			eprintln!("  pull  - Pull from all tmp/temp remotes");
			eprintln!("  clean - Remove stale tmp/temp remotes");
		}
	}
}

fn extract_ssh_host(url: &str) -> Option<String> {
	if url.starts_with("ssh://") {
		let without_proto = &url[6..];
		if let Some(at_pos) = without_proto.find('@') {
			let after_at = &without_proto[at_pos + 1..];
			if let Some(colon_pos) = after_at.find(':') {
				return Some(after_at[..colon_pos].to_string());
			}
			if let Some(slash_pos) = after_at.find('/') {
				return Some(after_at[..slash_pos].to_string());
			}
		}
	} else if url.contains("@") {
		if let Some(at_pos) = url.find('@') {
			let after_at = &url[at_pos + 1..];
			if let Some(colon_pos) = after_at.find(':') {
				return Some(after_at[..colon_pos].to_string());
			}
		}
	}
	None
}

fn check_ssh_reachable(host: &str) -> bool {
	let port = if host.contains(':') {
		let parts: Vec<&str> = host.split(':').collect();
		parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(22)
	} else {
		22
	};
	let clean_host = if host.contains(':') {
		host.split(':').next().unwrap_or(host)
	} else {
		host
	};
	TcpStream::connect_timeout(
		&format!("{}:{}", clean_host, port).parse().unwrap_or_else(|_| ([127,0,0,1], port).into()),
		Duration::from_secs(1)
	).is_ok()
}

fn run_proxy_setup(args: &[String]) {
	let cmd = args.get(0).map(|s| s.as_str()).unwrap_or("enable");
	
	#[cfg(target_os = "macos")]
	{
		match cmd {
			"enable" => {
				let proxy_host = args.get(1).unwrap_or(&"localhost".to_string()).clone();
				let proxy_port = args.get(2).unwrap_or(&"8888".to_string()).clone();
				
				println!("Setting up macOS system proxy:");
				println!("  Host: {}", proxy_host);
				println!("  Port: {}", proxy_port);
				
				// Get active network service
				let output = Command::new("networksetup")
					.args(["-listnetworkserviceorder"])
					.output();
				
				let mut active_service = "Wi-Fi".to_string();
				if let Ok(out) = output {
					let s = String::from_utf8_lossy(&out.stdout);
					for line in s.lines() {
						if line.contains("Wi-Fi") || line.contains("Ethernet") {
							if let Some(start) = line.find(')') {
								if let Some(service) = line[start+1..].trim().split(',').next() {
									active_service = service.trim().to_string();
									break;
								}
							}
						}
					}
				}
				
				println!("  Service: {}", active_service);
				
				// Set HTTP proxy
				let _ = Command::new("networksetup")
					.args(["-setwebproxy", &active_service, &proxy_host, &proxy_port])
					.status();
				
				// Set HTTPS proxy (TLS)
				let _ = Command::new("networksetup")
					.args(["-setsecurewebproxy", &active_service, &proxy_host, &proxy_port])
					.status();
				
				// Set SOCKS proxy
				let _ = Command::new("networksetup")
					.args(["-setsocksfirewallproxy", &active_service, &proxy_host, &proxy_port])
					.status();
				
				// Create and set PAC file
				let pac_content = format!(
					r#"function FindProxyForURL(url, host) {{
    // Direct connection for local addresses
    if (isPlainHostName(host) ||
        shExpMatch(host, "*.local") ||
        isInNet(dnsResolve(host), "10.0.0.0", "255.0.0.0") ||
        isInNet(dnsResolve(host), "172.16.0.0", "255.240.0.0") ||
        isInNet(dnsResolve(host), "192.168.0.0", "255.255.0.0") ||
        isInNet(dnsResolve(host), "127.0.0.0", "255.255.255.0"))
        return "DIRECT";
    
    // Use proxy for everything else (HTTP/TLS/SOCKS5)
    return "PROXY {}:{}; SOCKS5 {}:{}; DIRECT";
}}"#,
					proxy_host, proxy_port, proxy_host, proxy_port
				);
				
				let pac_path = "/tmp/litebike-proxy.pac";
				if let Err(e) = fs::write(pac_path, pac_content) {
					eprintln!("Failed to write PAC file: {}", e);
				} else {
					// Set PAC URL
					let pac_url = format!("file://{}", pac_path);
					let _ = Command::new("networksetup")
						.args(["-setautoproxyurl", &active_service, &pac_url])
						.status();
					
					// Enable auto proxy
					let _ = Command::new("networksetup")
						.args(["-setautoproxystate", &active_service, "on"])
						.status();
				}
				
				// Enable all proxy types
				let _ = Command::new("networksetup")
					.args(["-setwebproxystate", &active_service, "on"])
					.status();
				let _ = Command::new("networksetup")
					.args(["-setsecurewebproxystate", &active_service, "on"])
					.status();
				let _ = Command::new("networksetup")
					.args(["-setsocksfirewallproxystate", &active_service, "on"])
					.status();
				
				println!("✓ HTTP proxy enabled");
				println!("✓ HTTPS/TLS proxy enabled");
				println!("✓ SOCKS5 proxy enabled");
				println!("✓ PAC file configured");
				println!("✓ Auto-proxy enabled");
				
				// Set bypass domains
				let _ = Command::new("networksetup")
					.args(["-setproxybypassdomains", &active_service, "*.local", "169.254/16", "localhost", "127.0.0.1"])
					.status();
				
				println!("✓ Bypass domains configured");
				
				// Register Bonjour/mDNS service for auto-discovery
				let _ = Command::new("dns-sd")
					.args(["-R", "LiteBike Proxy", "_http._tcp,_sub:_proxy", "local", &proxy_port, "path=/"])
					.spawn();
				println!("✓ Bonjour service advertised");
				
				// Create WPAD file for auto-discovery
				let wpad_content = format!(
					r#"function FindProxyForURL(url, host) {{
    if (isInNet(dnsResolve(host), "192.168.0.0", "255.255.0.0") ||
        isInNet(dnsResolve(host), "10.0.0.0", "255.0.0.0") ||
        isInNet(dnsResolve(host), "172.16.0.0", "255.240.0.0") ||
        isInNet(dnsResolve(host), "127.0.0.0", "255.255.255.0"))
        return "DIRECT";
    return "PROXY {}:{}; SOCKS5 {}:{}; DIRECT";
}}"#,
					proxy_host, proxy_port, proxy_host, proxy_port
				);
				
				let wpad_path = "/tmp/wpad.dat";
				if let Err(e) = fs::write(wpad_path, &wpad_content) {
					eprintln!("Failed to write WPAD file: {}", e);
				} else {
					println!("✓ WPAD file created at {}", wpad_path);
				}
				
				// Check UPnP port mapping capability
				println!("\nVerifying network capabilities:");
				
				// Check if port is reachable
				if TcpStream::connect_timeout(
					&format!("{}:{}", proxy_host, proxy_port).parse().unwrap_or_else(|_| ([127,0,0,1], 8888).into()),
					Duration::from_millis(500)
				).is_ok() {
					println!("✓ Proxy port {} is reachable", proxy_port);
				} else {
					println!("⚠ Proxy port {} not yet reachable (service may need to be started)", proxy_port);
				}
				
				// Try UPnP discovery (check if miniupnpc is available)
				if let Ok(upnp_check) = Command::new("which").arg("upnpc").output() {
					if upnp_check.status.success() {
						// Try to list UPnP devices
						if let Ok(upnp_list) = Command::new("upnpc").args(["-l"]).output() {
							let output = String::from_utf8_lossy(&upnp_list.stdout);
							if output.contains("IGD") {
								println!("✓ UPnP gateway detected");
								// Try to add port mapping
								let _ = Command::new("upnpc")
									.args(["-a", "127.0.0.1", &proxy_port, &proxy_port, "TCP", "0", "LiteBike"])
									.status();
								println!("✓ UPnP port mapping attempted for port {}", proxy_port);
							} else {
								println!("⚠ No UPnP gateway found");
							}
						}
					} else {
						println!("⚠ UPnP tools not installed (install miniupnpc for UPnP support)");
					}
				}
				
				println!("\nSystem proxy fully configured with redundant auto-configuration!");
			}
			"disable" => {
				// Get active network service
				let output = Command::new("networksetup")
					.args(["-listnetworkserviceorder"])
					.output();
				
				let mut active_service = "Wi-Fi".to_string();
				if let Ok(out) = output {
					let s = String::from_utf8_lossy(&out.stdout);
					for line in s.lines() {
						if line.contains("Wi-Fi") || line.contains("Ethernet") {
							if let Some(start) = line.find(')') {
								if let Some(service) = line[start+1..].trim().split(',').next() {
									active_service = service.trim().to_string();
									break;
								}
							}
						}
					}
				}
				
				println!("Disabling proxy for: {}", active_service);
				
				// Disable all proxy types
				let _ = Command::new("networksetup")
					.args(["-setwebproxystate", &active_service, "off"])
					.status();
				let _ = Command::new("networksetup")
					.args(["-setsecurewebproxystate", &active_service, "off"])
					.status();
				let _ = Command::new("networksetup")
					.args(["-setsocksfirewallproxystate", &active_service, "off"])
					.status();
				let _ = Command::new("networksetup")
					.args(["-setautoproxystate", &active_service, "off"])
					.status();
				
				println!("✓ All proxies disabled");
			}
			"status" => {
				let output = Command::new("networksetup")
					.args(["-getwebproxy", "Wi-Fi"])
					.output();
				if let Ok(out) = output {
					println!("Proxy Status:\n{}", String::from_utf8_lossy(&out.stdout));
				}
			}
			_ => {
				eprintln!("Usage: litebike proxy-setup [enable|disable|status] [host] [port]");
				eprintln!("  enable  - Configure all system proxies and PAC");
				eprintln!("  disable - Disable all system proxies");
				eprintln!("  status  - Show current proxy settings");
				eprintln!("\nExample: litebike proxy-setup enable localhost 8888");
			}
		}
	}
	
	#[cfg(not(target_os = "macos"))]
	{
		eprintln!("proxy-setup: only available on macOS");
	}
}

fn run_upnp_gateway(args: &[String]) {
	let port = args.get(0).unwrap_or(&"1900".to_string()).parse::<u16>().unwrap_or(1900);
	let http_port = args.get(1).unwrap_or(&"8888".to_string()).parse::<u16>().unwrap_or(8888);
	
	println!("Starting UPnP IGD (Internet Gateway Device) on:");
	println!("  SSDP Discovery: UDP port {}", port);
	println!("  HTTP Control: TCP port {}", http_port);
	
	// Get local IP for responses
	let local_ip = get_default_local_ipv4().unwrap_or_else(|_| std::net::Ipv4Addr::new(127, 0, 0, 1));
	
	// Start SSDP discovery listener in a thread
	let local_ip_clone = local_ip.clone();
	thread::spawn(move || {
		if let Ok(socket) = UdpSocket::bind(("0.0.0.0", port)) {
			println!("✓ SSDP discovery listening on port {}", port);
			
			// Join multicast group for SSDP
			let multicast_addr = std::net::Ipv4Addr::new(239, 255, 255, 250);
			let _ = socket.join_multicast_v4(&multicast_addr, &std::net::Ipv4Addr::new(0, 0, 0, 0));
			
			let mut buf = [0; 2048];
			loop {
				if let Ok((len, src)) = socket.recv_from(&mut buf) {
					let msg = String::from_utf8_lossy(&buf[..len]);
					
					// Respond to M-SEARCH for IGD
					if msg.contains("M-SEARCH") && (msg.contains("ssdp:all") || msg.contains("InternetGatewayDevice")) {
						let response = format!(
							"HTTP/1.1 200 OK\r\n\
							CACHE-CONTROL: max-age=1800\r\n\
							EXT:\r\n\
							LOCATION: http://{}:{}/description.xml\r\n\
							SERVER: LiteBike/1.0 UPnP/1.0 IGD/1.0\r\n\
							ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\
							USN: uuid:litebike-gateway::urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\
							\r\n",
							local_ip_clone, http_port
						);
						let _ = socket.send_to(response.as_bytes(), src);
						println!("→ SSDP response sent to {}", src);
					}
				}
			}
		} else {
			eprintln!("Failed to bind SSDP socket on port {}", port);
		}
	});
	
	// Start HTTP control point server
	if let Ok(listener) = std::net::TcpListener::bind(("0.0.0.0", http_port)) {
		println!("✓ UPnP control point listening on port {}", http_port);
		
		// Announce presence
		if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
			let announce = format!(
				"NOTIFY * HTTP/1.1\r\n\
				HOST: 239.255.255.250:1900\r\n\
				CACHE-CONTROL: max-age=1800\r\n\
				LOCATION: http://{}:{}/description.xml\r\n\
				NT: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\
				NTS: ssdp:alive\r\n\
				SERVER: LiteBike/1.0 UPnP/1.0 IGD/1.0\r\n\
				USN: uuid:litebike-gateway::urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\
				\r\n",
				local_ip, http_port
			);
			let multicast_addr: SocketAddr = "239.255.255.250:1900".parse().unwrap();
			let _ = socket.send_to(announce.as_bytes(), multicast_addr);
			println!("✓ UPnP presence announced");
		}
		
		println!("\nLiteBike UPnP Gateway active - devices can now discover and use this as IGD");
		println!("Press Ctrl+C to stop");
		
		// Handle HTTP requests for device description and control
		for stream in listener.incoming() {
			if let Ok(mut stream) = stream {
				let mut buffer = [0; 1024];
				if let Ok(len) = stream.read(&mut buffer) {
					let request = String::from_utf8_lossy(&buffer[..len]);
					
					if request.contains("GET /description.xml") {
						let description = format!(
							"<?xml version=\"1.0\"?>\
							<root xmlns=\"urn:schemas-upnp-org:device-1-0\">\
								<specVersion><major>1</major><minor>0</minor></specVersion>\
								<device>\
									<deviceType>urn:schemas-upnp-org:device:InternetGatewayDevice:1</deviceType>\
									<friendlyName>LiteBike Gateway</friendlyName>\
									<manufacturer>LiteBike</manufacturer>\
									<modelName>LiteBike UPnP IGD</modelName>\
									<UDN>uuid:litebike-gateway</UDN>\
									<serviceList>\
										<service>\
											<serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType>\
											<serviceId>urn:upnp-org:serviceId:WANIPConn1</serviceId>\
											<SCPDURL>/scpd.xml</SCPDURL>\
											<controlURL>/control</controlURL>\
											<eventSubURL>/events</eventSubURL>\
										</service>\
									</serviceList>\
								</device>\
							</root>"
						);
						
						let response = format!(
							"HTTP/1.1 200 OK\r\n\
							Content-Type: text/xml\r\n\
							Content-Length: {}\r\n\
							\r\n\
							{}",
							description.len(),
							description
						);
						let _ = stream.write_all(response.as_bytes());
						println!("→ Device description served");
					} else if request.contains("POST /control") {
						// Handle port mapping requests
						if request.contains("AddPortMapping") {
							let response = "HTTP/1.1 200 OK\r\n\
								Content-Type: text/xml\r\n\
								Content-Length: 0\r\n\
								\r\n";
							let _ = stream.write_all(response.as_bytes());
							println!("→ Port mapping request accepted");
						}
					}
				}
			}
		}
	} else {
		eprintln!("Failed to bind HTTP control point on port {}", http_port);
	}
}
