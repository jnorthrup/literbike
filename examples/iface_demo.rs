use literbike::syscall_net::{get_default_gateway, list_interfaces};

fn main() -> std::io::Result<()> {
    let ifaces = list_interfaces()?;
    println!("Interfaces:");
    for (name, iface) in &ifaces {
        println!("  {}: index={}, flags=0x{:x}, addrs={:?}", name, iface.index, iface.flags, iface.addrs);
    }

    match get_default_gateway() {
        Ok(gw) => println!("Default gateway: {}", gw),
        Err(e) => eprintln!("Could not determine default gateway: {}", e),
    }

    Ok(())
}
