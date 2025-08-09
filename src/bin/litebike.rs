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
		"netstat" => run_netstat(),
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

fn run_netstat() {
	println!("Active Internet connections (servers/established) - minimal");
	println!("Proto Recv-Q Send-Q Local Address           Foreign Address         State");
	// Minimal placeholder; extend with netlink or /proc parsing as needed
}
