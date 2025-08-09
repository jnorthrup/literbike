//! Low-level, cross-platform network operations using direct syscalls.
//! This module provides the foundation for building network tools by interacting
//! directly with the operating system's networking stack via libc.

use std::collections::HashMap;
use std::ffi::CStr;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4};
use std::os::unix::io::RawFd;

/// Gets the default gateway IP address by using the most direct, low-level method available.
///
/// This function will prioritize syscalls or direct kernel file parsing over shell commands.
pub fn get_default_gateway() -> io::Result<Ipv4Addr> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    return parse_proc_net_route();

    #[cfg(target_os = "macos")]
    return parse_netstat_route(); // Placeholder for sysctl implementation

    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    Err(io::Error::new(io::ErrorKind::Other, "Unsupported OS for getting default gateway"))
}

/// Gets the default IPv6 gateway address using the most direct method available.
pub fn get_default_gateway_v6() -> io::Result<Ipv6Addr> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    return parse_proc_net_ipv6_route();

    #[cfg(target_os = "macos")]
    return parse_netstat_route_v6();

    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    Err(io::Error::new(io::ErrorKind::Other, "Unsupported OS for getting default IPv6 gateway"))
}

/// Best-effort: guess the default IPv6 egress interface by creating an IPv6 UDP socket,
/// connecting to a well-known IPv6 address, and matching the chosen local address to an interface.
pub fn guess_default_v6_interface() -> Option<String> {
    use std::net::{SocketAddr, UdpSocket};

    // Bind an IPv6 socket
    let sock = UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0)).ok()?;
    let targets = [
        ("2001:4860:4860::8888", 80u16),
        ("2606:4700:4700::1111", 80u16),
    ];
    for (host, port) in targets {
        if sock.connect((host, port)).is_ok() {
            if let Ok(local) = sock.local_addr() {
                if let SocketAddr::V6(sa) = local {
                    // Map local IPv6 to interface name via list_interfaces
                    if let Ok(ifaces) = list_interfaces() {
                        for (name, iface) in ifaces {
                            if iface.addrs.iter().any(|a| matches!(a, InterfaceAddr::V6(ip) if ip == sa.ip())) { return Some(name);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Best-effort discovery of the default local IPv4 address by opening a UDP socket
/// to common public IPs and reading the chosen local address. This does not send any packets.
pub fn get_default_local_ipv4() -> io::Result<Ipv4Addr> {
    use std::net::{SocketAddr, UdpSocket};

    let sock = UdpSocket::bind(("0.0.0.0", 0))?;
    let targets = [("1.1.1.1", 80u16), ("8.8.8.8", 80u16), ("9.9.9.9", 80u16)];
    for (host, port) in targets {
        if sock.connect((host, port)).is_ok() {
            if let Ok(local) = sock.local_addr() {
                if let SocketAddr::V4(sa) = local {
                    return Ok(*sa.ip());
                }
            }
        }
    }
    Err(io::Error::new(io::ErrorKind::Other, "unable to determine local IPv4"))
}

/// Best-effort discovery of the default local IPv6 address by opening a UDP socket
/// to common public IPv6 IPs and reading the chosen local address. This does not send any packets.
pub fn get_default_local_ipv6() -> io::Result<Ipv6Addr> {
    use std::net::{SocketAddr, UdpSocket};

    let sock = UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0))?;
    let targets = [("2001:4860:4860::8888", 80u16), ("2606:4700:4700::1111", 80u16)];
    for (host, port) in targets {
        if sock.connect((host, port)).is_ok() {
            if let Ok(local) = sock.local_addr() {
                if let SocketAddr::V6(sa) = local {
                    return Ok(*sa.ip());
                }
            }
        }
    }
    Err(io::Error::new(io::ErrorKind::Other, "unable to determine local IPv6"))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn parse_proc_net_route() -> io::Result<Ipv4Addr> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    match File::open("/proc/net/route") {
        Ok(file) => {
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() > 2 && parts[1] == "00000000" {
                    let gateway_hex = parts[2];
                    if let Ok(gateway_int) = u32::from_str_radix(gateway_hex, 16) {
                        // The IP address in /proc/net/route is in little-endian format.
                        return Ok(Ipv4Addr::from(gateway_int.to_le_bytes()));
                    }
                }
            }

            Err(io::Error::new(io::ErrorKind::NotFound, "Default route not found in /proc/net/route"))
        }
        Err(e) => {
            // Some Android devices restrict /proc/net; fall back to `ip route` parsing.
            if e.kind() == io::ErrorKind::PermissionDenied || e.kind() == io::ErrorKind::NotFound {
                parse_ip_route_default()
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn parse_ip_route_default() -> io::Result<Ipv4Addr> {
    use std::process::Command;
    use std::str;

    // Try `ip route show default` first
    let output = Command::new("ip").args(["route", "show", "default"]).output();
    let stdout = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => {
            // Fallback attempts: `ip route get 8.8.8.8`, toybox ip, busybox ip
            if let Ok(alt) = Command::new("ip").args(["route", "get", "8.8.8.8"]).output() {
                if alt.status.success() {
                    String::from_utf8_lossy(&alt.stdout).into_owned()
                } else if let Ok(tb) = Command::new("toybox").args(["ip", "route", "show", "default"]).output() {
                    if tb.status.success() {
                        String::from_utf8_lossy(&tb.stdout).into_owned()
                    } else if let Ok(bb) = Command::new("busybox").args(["ip", "route", "show", "default"]).output() {
                        if bb.status.success() {
                            String::from_utf8_lossy(&bb.stdout).into_owned()
                        } else {
                            // Last resort: try busybox route -n parsing below
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    // Try toybox/busybox paths
                    if let Ok(tb) = Command::new("toybox").args(["ip", "route", "show", "default"]).output() {
                        if tb.status.success() {
                            String::from_utf8_lossy(&tb.stdout).into_owned()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                }
            } else {
                // No ip binary? try toybox directly
                if let Ok(tb) = Command::new("toybox").args(["ip", "route", "show", "default"]).output() {
                    if tb.status.success() {
                        String::from_utf8_lossy(&tb.stdout).into_owned()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
        }
    };

    for line in stdout.lines() {
        // Common formats:
        // default via 192.168.1.1 dev wlan0 ...
        // 8.8.8.8 via 192.168.1.1 dev wlan0 src 192.168.1.10 ...
        if let Some(pos) = line.find(" via ") {
            let rest = &line[pos + 5..];
            let gw = rest.split_whitespace().next().unwrap_or("");
            if let Ok(addr) = gw.parse() {
                return Ok(addr);
            }
        }
    }

    // Try `busybox route -n` output
    if let Ok(rt) = Command::new("busybox").args(["route", "-n"]).output() {
        if rt.status.success() {
            let text = String::from_utf8_lossy(&rt.stdout);
            for line in text.lines() {
                // Destination Gateway Genmask Flags ...
                // 0.0.0.0    192.168.1.1  0.0.0.0   UG ...
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() >= 2 && cols[0] == "0.0.0.0" {
                    if let Ok(addr) = cols[1].parse() {
                        return Ok(addr);
                    }
                }
            }
        }
    }

    // Android getprop fallback: dhcp.<iface>.gateway
    if let Some(addr) = android_getprop_gateway() {
        return Ok(addr);
    }

    // Last-resort best-effort guess: derive gateway from local IPv4 (x.y.z.1)
    if let Some(guess) = guess_gateway_from_local_ip() {
        return Ok(guess);
    }

    Err(io::Error::new(io::ErrorKind::Other, "ip route command failed"))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn parse_proc_net_ipv6_route() -> io::Result<Ipv6Addr> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::process::Command;

    // Try /proc/net/ipv6_route first
    match File::open("/proc/net/ipv6_route") {
        Ok(file) => {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line?;
                // Fields per line: dest(32) dest_plen src(32) src_plen gw(32) metric refcnt use flags iface
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() < 10 { continue; }
                let dest = cols[0];
                let dest_plen = cols[1];
                let gw_hex = cols[4];
                if dest == "00000000000000000000000000000000" && dest_plen == "00000000" {
                    if let Some(addr) = hex32_to_ipv6(gw_hex) {
                        return Ok(addr);
                    }
                }
            }
            // Not found
            Err(io::Error::new(io::ErrorKind::NotFound, "Default IPv6 route not found in /proc/net/ipv6_route"))
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::PermissionDenied || e.kind() == io::ErrorKind::NotFound {
                // Fallback: try ip -6 route show default via toybox/busybox variants
                let tries: &[&[&str]] = &[
                    &["ip", "-6", "route", "show", "default"],
                    &["ip", "-6", "route", "get", "2001:4860:4860::8888"],
                    &["toybox", "ip", "-6", "route", "show", "default"],
                    &["busybox", "ip", "-6", "route", "show", "default"],
                ];
                for cmd in tries {
                    if let Ok(out) = Command::new(cmd[0]).args(&cmd[1..]).output() {
                        if out.status.success() {
                            let text = String::from_utf8_lossy(&out.stdout);
                            for line in text.lines() {
                                if let Some(pos) = line.find(" via ") {
                                    let rest = &line[pos + 5..];
                                    let gw = rest.split_whitespace().next().unwrap_or("");
                                    // Strip scope-id if present (e.g., %wlan0)
                                    let gw_clean = gw.split('%').next().unwrap_or(gw);
                                    if let Ok(ip) = gw_clean.parse() {
                                        return Ok(ip);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(io::Error::new(io::ErrorKind::Other, "ip -6 route command failed"))
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn hex32_to_ipv6(s: &str) -> Option<Ipv6Addr> {
    if s.len() < 32 { return None; }
    // Convert 32 hex chars to 16 bytes, accounting for per-32-bit little-endian representation
    let mut bytes = [0u8; 16];
    let mut out = 0;
    for i in (0..32).step_by(8) {
        // order: [6..8][4..6][2..4][0..2]
        let order = [(6,8),(4,6),(2,4),(0,2)];
        for (a,b) in order {
            let idx = i + a;
            if idx+2 <= s.len() {
                let byte = u8::from_str_radix(&s[idx..idx+2], 16).ok()?;
                bytes[out] = byte;
                out += 1;
            }
        }
    }
    Some(Ipv6Addr::from(bytes))
}

#[cfg(target_os = "macos")]
fn parse_netstat_route_v6() -> io::Result<Ipv6Addr> {
    use std::process::Command;
    use std::str;
    let output = Command::new("netstat").args(["-rn", "-f", "inet6"]).output()?;
    if !output.status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "netstat inet6 failed"));
    }
    let stdout = str::from_utf8(&output.stdout)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8"))?;
    for line in stdout.lines() {
        if line.starts_with("default") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                let gw = parts[1].split('%').next().unwrap_or(parts[1]);
                if let Ok(addr) = gw.parse() { return Ok(addr); }
            }
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "Default IPv6 route not found"))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn android_getprop_gateway() -> Option<Ipv4Addr> {
    use std::process::Command;

    // Common Android iface names to try; include env-driven interface for hints.
    let mut candidates: Vec<String> = vec![
        "wlan0", "swlan0", "eth0", "rmnet0", "rmnet_data0", "rmnet_data1", "rmnet_data7",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect();
    if let Ok(hint) = std::env::var("LITEBIKE_INTERFACE") {
        let hint = hint.trim().to_string();
        if !hint.is_empty() {
            candidates.insert(0, hint);
        }
    }

    for iface in &candidates {
        let key = format!("dhcp.{}.gateway", iface);
        if let Ok(out) = Command::new("getprop").arg(&key).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    if let Ok(ip) = s.parse() {
                        return Some(ip);
                    }
                }
            }
        }
    }
    None
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn guess_gateway_from_local_ip() -> Option<Ipv4Addr> {
    use std::net::{SocketAddr, UdpSocket};

    // Create a UDP socket and "connect" to a well-known public IP.
    // This doesn't send traffic but lets us discover the chosen local IP.
    let sock = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    // Try multiple common targets to maximize success without DNS.
    let targets = [
        ("1.1.1.1", 80u16),
        ("8.8.8.8", 80u16),
        ("9.9.9.9", 80u16),
    ];
    for (host, port) in targets {
        if sock.connect((host, port)).is_ok() {
            if let Ok(local) = sock.local_addr() {
                if let SocketAddr::V4(sa) = local {
                    let o = sa.ip().octets();
                    return Some(Ipv4Addr::new(o[0], o[1], o[2], 1));
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn parse_netstat_route() -> io::Result<Ipv4Addr> {
    use std::process::Command;
    use std::str;

    let output = Command::new("netstat").arg("-rn").arg("-f").arg("inet").output()?;
    if !output.status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "netstat command failed"));
    }

    let stdout = str::from_utf8(&output.stdout).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "netstat output is not valid UTF-8"))?;
    
    for line in stdout.lines() {
        if line.starts_with("default") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                if let Ok(addr) = parts[1].parse() {
                    return Ok(addr);
                }
            }
        }
    }

    Err(io::Error::new(io::ErrorKind::NotFound, "Default route not found in netstat output"))
}


use libc;

/// Represents a network interface.
#[derive(Debug, Clone)]
pub struct Interface {
    pub name: String,
    pub index: u32,
    pub flags: u32,
    pub addrs: Vec<InterfaceAddr>,
}

/// Represents an address associated with a network interface.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InterfaceAddr {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
    Link(Vec<u8>), // MAC address
}

/// Enumerates all network interfaces on the system using `getifaddrs`.
///
/// This function is the syscall-based equivalent of running `ifconfig -a` or `ip addr`.
/// It returns a map of interface names to `Interface` structs.
pub fn list_interfaces() -> io::Result<HashMap<String, Interface>> {
    let mut ifaddrs_ptr = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut ifaddrs_ptr) } != 0 {
        return Err(io::Error::last_os_error());
    }

    let mut interfaces = HashMap::new();
    let mut current = ifaddrs_ptr;

    while !current.is_null() {
        let ifa = unsafe { &*current };
        let name = unsafe { CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned() };
        let flags = ifa.ifa_flags;
        let index = unsafe { libc::if_nametoindex(ifa.ifa_name) };

        let entry = interfaces.entry(name.clone()).or_insert_with(|| Interface {
            name: name.clone(),
            index,
            flags,
            addrs: Vec::new(),
        });

        if let Some(addr) = unsafe { sockaddr_to_interface_addr(ifa.ifa_addr) } {
            if !entry.addrs.contains(&addr) {
                entry.addrs.push(addr);
            }
        }
        
        current = ifa.ifa_next;
    }

    unsafe { libc::freeifaddrs(ifaddrs_ptr) };

    Ok(interfaces)
}

/// Converts a `sockaddr` pointer to a Rust-native `InterfaceAddr`.
unsafe fn sockaddr_to_interface_addr(sockaddr: *const libc::sockaddr) -> Option<InterfaceAddr> {
    if sockaddr.is_null() {
        return None;
    }

    match (*sockaddr).sa_family as i32 {
        libc::AF_INET => {
            let sockaddr_in = &*(sockaddr as *const libc::sockaddr_in);
            let addr = Ipv4Addr::from(u32::from_be(sockaddr_in.sin_addr.s_addr));
            Some(InterfaceAddr::V4(addr))
        }
        libc::AF_INET6 => {
            let sockaddr_in6 = &*(sockaddr as *const libc::sockaddr_in6);
            let addr = Ipv6Addr::from(sockaddr_in6.sin6_addr.s6_addr);
            Some(InterfaceAddr::V6(addr))
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        libc::AF_PACKET => {
            let sockaddr_ll = &*(sockaddr as *const libc::sockaddr_ll);
            let mac = sockaddr_ll.sll_addr[..sockaddr_ll.sll_halen as usize].to_vec();
            Some(InterfaceAddr::Link(mac))
        }
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd", target_os = "openbsd"))]
        libc::AF_LINK => {
            let sockaddr_dl = &*(sockaddr as *const libc::sockaddr_dl);
            let mac_ptr = (sockaddr_dl.sdl_data.as_ptr() as *const u8).add(sockaddr_dl.sdl_nlen as usize);
            let mac = std::slice::from_raw_parts(mac_ptr, sockaddr_dl.sdl_alen as usize).to_vec();
            Some(InterfaceAddr::Link(mac))
        }
        _ => None,
    }
}

/// Creates a new socket using direct syscalls.
///
/// # Arguments
/// * `domain` - The communication domain (e.g., `libc::AF_INET` for IPv4, `libc::AF_INET6` for IPv6).
/// * `socket_type` - The socket type (e.g., `libc::SOCK_STREAM` for TCP, `libc::SOCK_DGRAM` for UDP).
/// * `protocol` - The protocol to be used (e.g., `libc::IPPROTO_TCP` for TCP, `libc::IPPROTO_UDP` for UDP).
///
/// # Returns
/// A `Result` containing the raw file descriptor of the new socket on success, or an `io::Error` on failure.
pub fn socket_create(domain: i32, socket_type: i32, protocol: i32) -> io::Result<RawFd> {
    let fd = unsafe { libc::socket(domain, socket_type, protocol) };
    if fd == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

/// Binds a socket to a specified address using direct syscalls.
///
/// # Arguments
/// * `fd` - The raw file descriptor of the socket to bind.
/// * `addr` - The address to bind the socket to.
///
/// # Returns
/// A `Result` indicating success or an `io::Error` on failure.
pub fn socket_bind(fd: RawFd, addr: &SocketAddrV4) -> io::Result<()> {
    let sockaddr = libc::sockaddr_in {
        #[cfg(target_os = "macos")]
        sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
        sin_family: libc::AF_INET as _,
        sin_port: addr.port().to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::from(*addr.ip()).to_be(),
        },
        sin_zero: [0; 8],
    };

    let ret = unsafe {
        libc::bind(
            fd,
            &sockaddr as *const _ as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Listens for incoming connections on a bound socket using direct syscalls.
///
/// # Arguments
/// * `fd` - The raw file descriptor of the socket to listen on.
/// * `backlog` - The maximum length of the queue of pending connections.
///
/// # Returns
/// A `Result` indicating success or an `io::Error` on failure.
pub fn socket_listen(fd: RawFd, backlog: i32) -> io::Result<()> {
    let ret = unsafe { libc::listen(fd, backlog) };
    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Accepts a new connection on a listening socket using direct syscalls.
///
/// # Arguments
/// * `fd` - The raw file descriptor of the listening socket.
///
/// # Returns
/// A `Result` containing the raw file descriptor of the new connection and its peer address on success, or an `io::Error` on failure.
pub fn socket_accept(fd: RawFd) -> io::Result<(RawFd, SocketAddrV4)> {
    let mut sockaddr = libc::sockaddr_in {
        #[cfg(target_os = "macos")]
        sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
        sin_family: 0 as _,
        sin_port: 0,
        sin_addr: libc::in_addr { s_addr: 0 },
        sin_zero: [0; 8],
    };
    let mut len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;

    let conn_fd = unsafe {
        libc::accept(
            fd,
            &mut sockaddr as *mut _ as *mut libc::sockaddr,
            &mut len,
        )
    };

    if conn_fd == -1 {
        Err(io::Error::last_os_error())
    } else {
        let ip = Ipv4Addr::from(u32::from_be(sockaddr.sin_addr.s_addr));
        let port = u16::from_be(sockaddr.sin_port);
        Ok((conn_fd, SocketAddrV4::new(ip, port)))
    }
}

/// Connects a socket to a remote address using direct syscalls.
///
/// # Arguments
/// * `fd` - The raw file descriptor of the socket to connect.
/// * `addr` - The remote address to connect to.
///
/// # Returns
/// A `Result` indicating success or an `io::Error` on failure.
pub fn socket_connect(fd: RawFd, addr: &SocketAddrV4) -> io::Result<()> {
    let sockaddr = libc::sockaddr_in {
        #[cfg(target_os = "macos")]
        sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
        sin_family: libc::AF_INET as _,
        sin_port: addr.port().to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::from(*addr.ip()).to_be(),
        },
        sin_zero: [0; 8],
    };

    let ret = unsafe {
        libc::connect(
            fd,
            &sockaddr as *const _ as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Reads data from a socket using direct syscalls.
///
/// # Arguments
/// * `fd` - The raw file descriptor of the socket to read from.
/// * `buf` - The buffer to read data into.
///
/// # Returns
/// A `Result` containing the number of bytes read on success, or an `io::Error` on failure.
pub fn socket_read(fd: RawFd, buf: &mut [u8]) -> io::Result<usize> {
    let ret = unsafe {
        libc::read(
            fd,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len() as libc::size_t,
        )
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(ret as usize)
    }
}

/// Writes data to a socket using direct syscalls.
///
/// # Arguments
/// * `fd` - The raw file descriptor of the socket to write to.
/// * `buf` - The buffer containing data to write.
///
/// # Returns
/// A `Result` containing the number of bytes written on success, or an `io::Error` on failure.
pub fn socket_write(fd: RawFd, buf: &[u8]) -> io::Result<usize> {
    let ret = unsafe {
        libc::write(
            fd,
            buf.as_ptr() as *const libc::c_void,
            buf.len() as libc::size_t,
        )
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(ret as usize)
    }
}

/// Closes a socket using direct syscalls.
///
/// # Arguments
/// * `fd` - The raw file descriptor of the socket to close.
///
/// # Returns
/// A `Result` indicating success or an `io::Error` on failure.
pub fn socket_close(fd: RawFd) -> io::Result<()> {
    let ret = unsafe { libc::close(fd) };
    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_interfaces_syscall() {
        // This test performs a live syscall to list interfaces.
        // It checks for basic success and that it found at least one interface,
        // typically the loopback 'lo0' or 'lo'.
        let interfaces = list_interfaces().expect("Failed to list interfaces via syscall");
        
        assert!(!interfaces.is_empty(), "Should find at least one network interface");

        let loopback_found = interfaces.keys().any(|name| name == "lo0" || name == "lo");
        assert!(loopback_found, "Should find a loopback interface ('lo' or 'lo0')");

        // Check if the loopback has an IPv4 address.
        let loopback = interfaces.values().find(|iface| iface.name == "lo0" || iface.name == "lo").unwrap();
        let has_ipv4 = loopback.addrs.iter().any(|addr| matches!(addr, InterfaceAddr::V4(_)));
        assert!(has_ipv4, "Loopback interface should have an IPv4 address");
    }

    #[test]
    fn test_tcp_socket_operations() {
        use std::net::Ipv4Addr;
        use std::thread;
        use std::time::Duration;

        let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);

        // Server side
        let server_thread = thread::spawn(move || {
            let listener_fd = socket_create(libc::AF_INET, libc::SOCK_STREAM, 0)
                .expect("Failed to create listener socket");
            socket_bind(listener_fd, &addr).expect("Failed to bind listener socket");
            socket_listen(listener_fd, 1).expect("Failed to listen on socket");

            let (conn_fd, peer_addr) = socket_accept(listener_fd)
                .expect("Failed to accept connection");
            println!("Server accepted connection from: {}", peer_addr);

            let mut buffer = [0; 1024];
            let bytes_read = socket_read(conn_fd, &mut buffer)
                .expect("Failed to read from socket");
            println!("Server received: {}", String::from_utf8_lossy(&buffer[..bytes_read]));

            let response = b"Hello from server!";
            socket_write(conn_fd, response)
                .expect("Failed to write to socket");

            socket_close(conn_fd).expect("Failed to close connection socket");
            socket_close(listener_fd).expect("Failed to close listener socket");
        });

        // Give server a moment to start listening
        thread::sleep(Duration::from_millis(100));

        // Client side
        let client_fd = socket_create(libc::AF_INET, libc::SOCK_STREAM, 0)
            .expect("Failed to create client socket");
        socket_connect(client_fd, &addr).expect("Failed to connect client socket");

        let message = b"Hello from client!";
        socket_write(client_fd, message).expect("Failed to write to socket");

        let mut buffer = [0; 1024];
        let bytes_read = socket_read(client_fd, &mut buffer)
            .expect("Failed to read from socket");
        println!("Client received: {}", String::from_utf8_lossy(&buffer[..bytes_read]));

        socket_close(client_fd).expect("Failed to close client socket");

        server_thread.join().expect("Server thread panicked");
    }
}