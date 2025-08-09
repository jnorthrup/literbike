use litebike::syscall_net::{get_default_gateway, list_interfaces, InterfaceAddr};
use std::env;
use std::path::Path;

fn main() {
	let args: Vec<String> = env::args().collect();
	let prog = Path::new(&args[0])
		.file_name()
		.and_then(|s| s.to_str())
		.unwrap_or("litebike");

	match prog {
		"ifconfig" => run_ifconfig(),
		"ip" => run_ip(&args[1..]),
		"route" => run_route(),
		"netstat" => run_netstat(),
		_ => {
			// Default: short help and a quick interfaces print
			eprintln!("litebike: argv0-dispatch utility (ifconfig | ip | route | netstat)\n");
			run_ifconfig();
		}
	}
}

fn run_ifconfig() {
	match list_interfaces() {
		Ok(ifaces) => {
			for (name, iface) in ifaces {
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
	match get_default_gateway() {
		Ok(gw) => {
			println!("Kernel IP routing table");
			println!("Destination     Gateway         Genmask         Flags Metric Ref    Use Iface");
			println!("0.0.0.0         {:<15} 0.0.0.0         UG    0      0        0 -", gw);
		}
		Err(e) => eprintln!("route: {}", e),
	}
}

fn run_netstat() {
	println!("Active Internet connections (servers/established) - minimal");
	println!("Proto Recv-Q Send-Q Local Address           Foreign Address         State");
	// Minimal placeholder; extend with netlink or /proc parsing as needed
}
