use litebike::syscall_net::{get_default_gateway, get_default_local_ipv4, list_interfaces, InterfaceAddr};
use std::env;
use std::path::Path;

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
		_ => {
			// Default: short help and a quick interfaces print
			eprintln!("litebike: argv0-dispatch utility (ifconfig | ip | route | netstat)\n");
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

fn run_ip(args: &[String]) {
	if args.is_empty() {
		eprintln!("Usage: ip [addr|route]");
		return;
	}
	match args[0].as_str() {
		"addr" | "address" => {
			match list_interfaces() {
				Ok(ifaces) => {
					let mut idx = 1u32;
					for (name, iface) in ifaces {
						println!("{}: {}: <UP> mtu 1500", idx, name);
						for addr in iface.addrs {
							match addr {
								InterfaceAddr::V4(ip) => println!("    inet {}/24", ip),
								InterfaceAddr::V6(ip) => println!("    inet6 {}/64", ip),
								InterfaceAddr::Link(_) => {}
							}
						}
						idx += 1;
					}
				}
				Err(e) => eprintln!("ip addr: {}", e),
			}
		}
		"route" => run_route(),
		_ => eprintln!("ip: unknown command '{}'", args[0]),
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
