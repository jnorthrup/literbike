// LiteBike - Simple NC-style proxy concentrator
// nc on local 8888, direct routing upstream

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::thread;
use std::collections::HashMap;
use libc::{c_char, c_int, c_void, sockaddr, sockaddr_in, socklen_t, sa_family_t, STDERR_FILENO};
use std::ffi::CStr;
use std::mem;
use std::ptr;

// Basic ioctl constants (platform specific)
#[cfg(any(target_os = "linux", target_os = "android"))]
mod ioctl_consts {
    pub const SIOCGIFCONF: u64 = 0x8912;
    pub const SIOCGIFADDR: u64 = 0x8915;
    pub const SIOCGIFFLAGS: u64 = 0x8913;
    pub const SIOCGIFNETMASK: u64 = 0x891b;
    pub const SIOCGIFHWADDR: u64 = 0x8927;
}

#[cfg(target_os = "macos")]
mod ioctl_consts {
    pub const SIOCGIFCONF: u64 = 0xc00c6924;
    pub const SIOCGIFADDR: u64 = 0xc0206921;
    pub const SIOCGIFFLAGS: u64 = 0xc0206911;
    pub const SIOCGIFNETMASK: u64 = 0xc0206925;
}

use ioctl_consts::*;

// Netlink constants for Linux/Android
#[cfg(any(target_os = "linux", target_os = "android"))]
mod netlink {
    pub const AF_NETLINK: i32 = 16;
    pub const NETLINK_ROUTE: i32 = 0;
    pub const NETLINK_INET_DIAG: i32 = 4;

    pub const RTM_GETLINK: u16 = 18;
    pub const RTM_GETADDR: u16 = 22;
    pub const RTM_GETROUTE: u16 = 26;

    pub const TCPDIAG_GETSOCK: u16 = 18;
    pub const INET_DIAG_REQ_V2: u16 = 2;
    
    pub const NLM_F_REQUEST: u16 = 0x01;
    pub const NLM_F_DUMP: u16 = 0x300;
    
    pub const NLMSG_DONE: u16 = 3;
    pub const NLMSG_ERROR: u16 = 2;
    
    pub const IFLA_IFNAME: u16 = 3;
    pub const IFLA_ADDRESS: u16 = 1;
    pub const IFA_ADDRESS: u16 = 1;
    pub const IFA_LOCAL: u16 = 2;
    pub const RTA_DST: u16 = 1;
    pub const RTA_GATEWAY: u16 = 5;
    pub const RTA_OIF: u16 = 4;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
use netlink ::*;

#[repr(C)]
struct ifreq {
    ifr_name: [c_char; 16],
    ifr_data: [u8; 24], // Union data
}

#[repr(C)]
struct ifreq_flags {
    ifr_name: [c_char; 16],
    ifr_flags: u16,
}

#[repr(C)]
struct ifconf {
    ifc_len: c_int,
    ifc_buf: *mut c_char,
}

// Netlink structures
#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct nlmsghdr {
    nlmsg_len: u32,
    nlmsg_type: u16,
    nlmsg_flags: u16,
    nlmsg_seq: u32,
    nlmsg_pid: u32,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct ifinfomsg {
    ifi_family: u8,
    ifi_pad: u8,
    ifi_type: u16,
    ifi_index: i32,
    ifi_flags: u32,
    ifi_change: u32,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct ifaddrmsg {
    ifa_family: u8,
    ifa_prefixlen: u8,
    ifa_flags: u8,
    ifa_scope: u8,
    ifa_index: u32,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct rtmsg {
    rtm_family: u8,
    rtm_dst_len: u8,
    rtm_src_len: u8,
    rtm_tos: u8,
    rtm_table: u8,
    rtm_protocol: u8,
    rtm_scope: u8,
    rtm_type: u8,
    rtm_flags: u32,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct rtattr {
    rta_len: u16,
    rta_type: u16,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct sockaddr_nl {
    nl_family: sa_family_t,
    nl_pad: u16,
    nl_pid: u32,
    nl_groups: u32,
}

#[repr(C)]
struct inet_diag_req_v2 {
    sdiag_family: u8,
    sdiag_protocol: u8,
    idiag_ext: u8,
    pad: u8,
    idiag_states: u32,
    id: [u32; 8],
}

#[repr(C)]
struct inet_diag_msg {
    idiag_family: u8,
    idiag_state: u8,
    idiag_timer: u8,
    idiag_retrans: u8,
    id: [u32; 8],
    idiag_expires: u32,
    idiag_rqueue: u32,
    idiag_wqueue: u32,
    idiag_uid: u32,
    idiag_inode: u32,
}

// TCP state constants (from linux/tcp_states.h)
mod tcp_states {
    pub const TCP_ESTABLISHED: u8 = 1;
    pub const TCP_SYN_SENT: u8 = 2;
    pub const TCP_SYN_RECV: u8 = 3;
    pub const TCP_FIN_WAIT1: u8 = 4;
    pub const TCP_FIN_WAIT2: u8 = 5;
    pub const TCP_TIME_WAIT: u8 = 6;
    pub const TCP_CLOSE: u8 = 7;
    pub const TCP_CLOSE_WAIT: u8 = 8;
    pub const TCP_LAST_ACK: u8 = 9;
    pub const TCP_LISTEN: u8 = 10;
    pub const TCP_CLOSING: u8 = 11;
}

#[derive(Clone)]
struct LiteBikeConfig {
    listen_port: u16,
    default_upstream: SocketAddr,
    protocol_routes: HashMap<String, SocketAddr>,
}

impl Default for LiteBikeConfig {
    fn default() -> Self {
        let protocol_routes = HashMap::new();
        
        Self {
            listen_port: 8888,
            default_upstream: "1.1.1.1:80".parse().unwrap(),
            protocol_routes,
        }
    }
}

fn detect_protocol(data: &[u8]) -> Option<&'static str> {
    if data.len() < 2 { return None; }
    
    if data[0] == 0x05 {
        return Some("socks5");
    }
    
    if let Ok(text) = std::str::from_utf8(&data[..data.len().min(8)]) {
        if text.starts_with("GET ") || text.starts_with("POST") || text.starts_with("CONN") {
            return Some("http");
        }
    }
    
    None
}

fn handle_client(mut client: TcpStream, config: &LiteBikeConfig) -> io::Result<()> {
    let client_addr = client.peer_addr().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap());
    println!("Client: {}", client_addr);
    
    let mut peek_buf = [0u8; 1024];
    let bytes_read = client.peek(&mut peek_buf).unwrap_or(0);
    
    if bytes_read > 0 {
        if let Ok(request) = std::str::from_utf8(&peek_buf[..bytes_read]) {
            if request.starts_with("CONNECT ") {
                // Handle HTTPS CONNECT method
                if let Some(host_line) = request.lines().next() {
                    if let Some(host_port) = host_line.strip_prefix("CONNECT ").and_then(|s| s.split(' ').next()) {
                        println!("CONNECT to {}", host_port);
                        match TcpStream::connect(host_port) {
                            Ok(mut upstream_conn) => {
                                let _ = client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
                                
                                // Clone for bidirectional relay
                                let mut client_read = client.try_clone()?;
                                let mut upstream_write = upstream_conn.try_clone()?;
                                let mut upstream_read = upstream_conn;
                                let mut client_write = client;
                                
                                // Client -> upstream thread
                                thread::spawn(move || {
                                    let mut buf = [0u8; 4096];
                                    loop {
                                        match client_read.read(&mut buf) {
                                            Ok(0) => break,
                                            Ok(n) => {
                                                if upstream_write.write_all(&buf[..n]).is_err() { break; }
                                            }
                                            Err(_) => break,
                                        }
                                    }
                                });
                                
                                // Upstream -> client (main thread)
                                let mut buf = [0u8; 4096];
                                loop {
                                    match upstream_read.read(&mut buf) {
                                        Ok(0) => break,
                                        Ok(n) => {
                                            if client_write.write_all(&buf[..n]).is_err() { break; }
                                        }
                                        Err(_) => break,
                                    }
                                }
                                return Ok(());
                            }
                            Err(e) => {
                                let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n");
                                return Err(e);
                            }
                        }
                    }
                }
            } else if request.starts_with("GET ") || request.starts_with("POST ") {
                // Handle HTTP requests - extract host and forward
                println!("HTTP request");
            }
        }
    }
    
    let upstream = config.default_upstream;
    println!("Routing {} -> {}", client_addr, upstream);
    
    let mut upstream_conn = TcpStream::connect(upstream)?;
    
    // Clone for bidirectional relay
    let mut client_read = client.try_clone()?;
    let mut upstream_write = upstream_conn.try_clone()?;
    let mut upstream_read = upstream_conn;
    let mut client_write = client;
    
    // Client -> upstream thread
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match client_read.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if upstream_write.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    
    // Upstream -> client (main thread)
    let mut buf = [0u8; 4096];
    loop {
        match upstream_read.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if client_write.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    
    println!("Closed: {}", client_addr);
    Ok(())
}

unsafe fn ifconfig() -> io::Result<()> {
    // Just list interfaces - minimal implementation
    let sock = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
    if sock < 0 {
        libc::perror(b"socket\0".as_ptr() as *const c_char);
        return Err(io::Error::last_os_error());
    }

    // Try to get lo interface as example
    let mut ifr: ifreq = mem::zeroed();
    libc::strcpy(ifr.ifr_name.as_mut_ptr(), b"lo\0".as_ptr() as *const c_char);
    
    if libc::ioctl(sock, SIOCGIFADDR as _, &mut ifr as *mut _ as *mut c_void) == 0 {
        let addr = unsafe { &*(ifr.ifr_data.as_ptr() as *const libc::sockaddr_in) };
        let ip = addr.sin_addr.s_addr;
        libc::printf(b"lo: inet %d.%d.%d.%d\n\0".as_ptr() as *const c_char, 
                     (ip & 0xff) as c_int,
                     ((ip >> 8) & 0xff) as c_int,
                     ((ip >> 16) & 0xff) as c_int,
                     ((ip >> 24) & 0xff) as c_int);
    }

    // Try common interface names
    let ifaces = [b"eth0\0".as_ptr(), b"wlan0\0".as_ptr(), b"en0\0".as_ptr(), b"en1\0".as_ptr()];
    for iface in &ifaces {
        libc::strcpy(ifr.ifr_name.as_mut_ptr(), *iface as *const c_char);
        if libc::ioctl(sock, SIOCGIFADDR as _, &mut ifr as *mut _ as *mut c_void) == 0 {
            let addr = unsafe { &*(ifr.ifr_data.as_ptr() as *const libc::sockaddr_in) };
            let ip = addr.sin_addr.s_addr;
            let name = CStr::from_ptr(ifr.ifr_name.as_ptr()).to_bytes();
            libc::printf(b"%s: inet %d.%d.%d.%d\n\0".as_ptr() as *const c_char,
                         name.as_ptr() as *const c_char,
                         (ip & 0xff) as c_int,
                         ((ip >> 8) & 0xff) as c_int,
                         ((ip >> 16) & 0xff) as c_int,
                         ((ip >> 24) & 0xff) as c_int);
        }
    }

    libc::close(sock);
    Ok(())
}

unsafe fn netstat() -> io::Result<()> {
    libc::printf(b"Active Internet connections (w/o servers)\n\0".as_ptr() as *const c_char);
    libc::printf(b"Proto Recv-Q Send-Q Local Address           Foreign Address         State\n\0".as_ptr() as *const c_char);

    let sock = libc::socket(AF_NETLINK, libc::SOCK_RAW, NETLINK_INET_DIAG);
    if sock < 0 {
        libc::perror(b"socket\0".as_ptr() as *const c_char);
        return Err(io::Error::last_os_error());
    }

    let mut local_addr: sockaddr_nl = mem::zeroed();
    local_addr.nl_family = AF_NETLINK as sa_family_t;
    if libc::bind(sock, &local_addr as *const _ as *const sockaddr, mem::size_of::<sockaddr_nl>() as socklen_t) < 0 {
        libc::perror(b"bind\0".as_ptr() as *const c_char);
        libc::close(sock);
        return Err(io::Error::last_os_error());
    }

    let mut req: [u8; 28] = mem::zeroed();
    let nlh = &mut *(req.as_mut_ptr() as *mut nlmsghdr);
    nlh.nlmsg_len = 28;
    nlh.nlmsg_type = TCPDIAG_GETSOCK;
    nlh.nlmsg_flags = NLM_F_REQUEST | NLM_F_DUMP;

    let idr = (req.as_mut_ptr().add(16)) as *mut inet_diag_req_v2;
    (*idr).sdiag_family = libc::AF_INET as u8;

    if libc::send(sock, req.as_mut_ptr() as *const c_void, req.len(), 0) < 0 {
        libc::perror(b"send\0".as_ptr() as *const c_char);
        libc::close(sock);
        return Err(io::Error::last_os_error());
    }

    let mut buf = [0u8; 8192];
    loop {
        let len = libc::recv(sock, buf.as_mut_ptr() as *mut c_void, buf.len(), 0);
        if len <= 0 {
            break;
        }

        let mut nlh = buf.as_ptr() as *const nlmsghdr;
        while (nlh as *const u8) < (buf.as_ptr().add(len as usize)) {
            if (*nlh).nlmsg_type == NLMSG_DONE {
                break;
            }
            if (*nlh).nlmsg_type == NLMSG_ERROR {
                libc::printf(b"netlink error\n\0".as_ptr() as *const c_char);
                break;
            }

            let idm = (nlh as *const u8).add(mem::size_of::<nlmsghdr>()) as *const inet_diag_msg;
            let laddr = (*idm).id[0].to_be();
            let lport = (*idm).id[4].to_be() >> 16;
            let raddr = (*idm).id[1].to_be();
            let rport = (*idm).id[4].to_be() & 0xffff;

            libc::printf(
                b"tcp   %6d %6d %-21s %-21s %s\n\0".as_ptr() as *const c_char,
                (*idm).idiag_rqueue,
                (*idm).idiag_wqueue,
                format_addr(laddr, lport).as_ptr(),
                format_addr(raddr, rport).as_ptr(),
                get_tcp_state((*idm).idiag_state)
            );

            let align_to = 4;
            let aligned_len = ((*nlh).nlmsg_len + (align_to - 1)) & !(align_to - 1);
            nlh = (nlh as *const u8).add(aligned_len as usize) as *const nlmsghdr;
        }
    }

    libc::close(sock);
    Ok(())
}

fn format_addr(addr: u32, port: u32) -> [c_char; 22] {
    let mut buf = [0 as c_char; 22];
    let ip_str = format!("{}.{}.{}.{}", (addr >> 24) & 0xff, (addr >> 16) & 0xff, (addr >> 8) & 0xff, addr & 0xff);
    let full_addr = format!("{}:{}", ip_str, port);
    for (i, c) in full_addr.chars().enumerate() {
        if i < 21 {
            buf[i] = c as c_char;
        }
    }
    buf
}

fn get_tcp_state(state: u8) -> *const c_char {
    match state {
        tcp_states::TCP_ESTABLISHED => b"ESTABLISHED\0".as_ptr() as *const c_char,
        tcp_states::TCP_SYN_SENT => b"SYN_SENT\0".as_ptr() as *const c_char,
        tcp_states::TCP_SYN_RECV => b"SYN_RECV\0".as_ptr() as *const c_char,
        tcp_states::TCP_FIN_WAIT1 => b"FIN_WAIT1\0".as_ptr() as *const c_char,
        tcp_states::TCP_FIN_WAIT2 => b"FIN_WAIT2\0".as_ptr() as *const c_char,
        tcp_states::TCP_TIME_WAIT => b"TIME_WAIT\0".as_ptr() as *const c_char,
        tcp_states::TCP_CLOSE => b"CLOSE\0".as_ptr() as *const c_char,
        tcp_states::TCP_CLOSE_WAIT => b"CLOSE_WAIT\0".as_ptr() as *const c_char,
        tcp_states::TCP_LAST_ACK => b"LAST_ACK\0".as_ptr() as *const c_char,
        tcp_states::TCP_LISTEN => b"LISTEN\0".as_ptr() as *const c_char,
        tcp_states::TCP_CLOSING => b"CLOSING\0".as_ptr() as *const c_char,
        _ => b"UNKNOWN\0".as_ptr() as *const c_char,
    }
}

unsafe fn route() -> io::Result<()> {
    libc::printf(b"Kernel IP routing table\n\0".as_ptr() as *const c_char);
    libc::printf(b"Destination     Gateway         Genmask         Flags Metric Ref    Use Iface\n\0".as_ptr() as *const c_char);
    #[cfg(target_os = "linux")]
    {
        // Try netlink on Linux
        let sock = libc::socket(AF_NETLINK, libc::SOCK_RAW, NETLINK_ROUTE);
        if sock >= 0 {
            libc::printf(b"0.0.0.0         0.0.0.0         0.0.0.0         U     0      0        0 lo\n\0".as_ptr() as *const c_char);
            libc::close(sock);
        } else {
            libc::printf(b"(netlink requires root/CAP_NET_ADMIN)\n\0".as_ptr() as *const c_char);
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        libc::printf(b"(route listing not supported on this platform via syscalls)\n\0".as_ptr() as *const c_char);
    }
    Ok(())
}

unsafe fn ip(args: &[String]) -> io::Result<()> {
    if args.len() < 2 {
        libc::printf(b"Usage: ip [addr|route]\n\0".as_ptr() as *const c_char);
        return Ok(())
    }

    match args[1].as_str() {
        "addr" | "address" => {
            libc::printf(b"1: lo: <LOOPBACK,UP>\n\0".as_ptr() as *const c_char);
            libc::printf(b"    inet 127.0.0.1/8\n\0".as_ptr() as *const c_char);
            
            // Try to find active interfaces
            let sock = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
            if sock >= 0 {
                let mut idx = 2;
                let ifaces = [b"eth0\0".as_ptr(), b"wlan0\0".as_ptr(), b"en0\0".as_ptr(), b"en1\0".as_ptr()];
                for iface in &ifaces {
                    let mut ifr: ifreq = mem::zeroed();
                    libc::strcpy(ifr.ifr_name.as_mut_ptr(), *iface as *const c_char);
                    if libc::ioctl(sock, SIOCGIFADDR as _, &mut ifr as *mut _ as *mut c_void) == 0 {
                        let name = CStr::from_ptr(ifr.ifr_name.as_ptr()).to_bytes();
                        let addr = unsafe { &*(ifr.ifr_data.as_ptr() as *const libc::sockaddr_in) };
                        let ip = addr.sin_addr.s_addr;
                        libc::printf(b"%d: %s: <UP>\n\0".as_ptr() as *const c_char, idx, name.as_ptr());
                        libc::printf(b"    inet %d.%d.%d.%d/24\n\0".as_ptr() as *const c_char,
                                     (ip & 0xff) as c_int,
                                     ((ip >> 8) & 0xff) as c_int,
                                     ((ip >> 16) & 0xff) as c_int,
                                     ((ip >> 24) & 0xff) as c_int);
                        idx += 1;
                    }
                }
                libc::close(sock);
            }
        }
        "route" => route()?,
        _ => {
            let msg = format!("Unknown command: {}
", args[1]);
            libc::write(STDERR_FILENO, msg.as_ptr() as *const c_void, msg.len());
        }
    }
    Ok(())
}


fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.is_empty() {
        unsafe { libc::exit(1); }
    }

    let prog = std::path::Path::new(&args[0])
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("litebike");

    unsafe {
        match prog {
            "ifconfig" => ifconfig(),
            "netstat" => netstat(),
            "route" => route(),
            "ip" => ip(&args),
            _ => {
                // Original proxy server logic
                let config = LiteBikeConfig::default();
                
                println!("LiteBike NC Proxy");
                println!("Listen: {}", config.listen_port);
                println!("Default: {}", config.default_upstream);
                for (proto, addr) in &config.protocol_routes {
                    println!("{} -> {}", proto, addr);
                }
                
                let listener = TcpListener::bind(format!("0.0.0.0:{}", config.listen_port))?;
                println!("Ready on port {}", config.listen_port);
                
                for stream in listener.incoming() {
                    match stream {
                        Ok(client) => {
                            let config = config.clone();
                            thread::spawn(move || {
                                if let Err(e) = handle_client(client, &config) {
                                    eprintln!("Error: {}", e);
                                }
                            });
                        }
                        Err(e) => eprintln!("Accept error: {}", e),
                    }
                }
                Ok(())
            }
        }
    }
}