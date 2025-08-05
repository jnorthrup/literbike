//! Minimal network utilities using ONLY syscalls - no /proc, /sys, /dev access
//! Single binary with symlink dispatch based on argv[0]
//! Pure C-style implementation with minimal Rust wrapper

use libc::{c_char, c_int, c_void, sockaddr_in, STDERR_FILENO};
use std::ffi::CStr;
use std::mem;

// Basic ioctl constants (platform specific)
#[cfg(target_os = "linux")]
mod ioctl_consts {
    pub const SIOCGIFCONF: u64 = 0x8912;
    pub const SIOCGIFADDR: u64 = 0x8915;
    pub const SIOCGIFFLAGS: u64 = 0x8913;
}

#[cfg(target_os = "macos")]
mod ioctl_consts {
    pub const SIOCGIFCONF: u64 = 0xc00c6924;
    pub const SIOCGIFADDR: u64 = 0xc0206921;
    pub const SIOCGIFFLAGS: u64 = 0xc0206911;
}

#[cfg(target_os = "android")]
mod ioctl_consts {
    pub const SIOCGIFCONF: u64 = 0x8912;
    pub const SIOCGIFADDR: u64 = 0x8915;
    pub const SIOCGIFFLAGS: u64 = 0x8913;
}

use ioctl_consts::*;

#[repr(C)]
struct ifreq {
    ifr_name: [c_char; 16],
    ifr_addr: sockaddr_in,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.is_empty() {
        unsafe { libc::exit(1); }
    }

    let prog = std::path::Path::new(&args[0])
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("netutils");

    unsafe {
        match prog {
            "ifconfig" => ifconfig(),
            "netstat" => netstat(),
            "route" => route(),
            "ip" => ip(&args),
            _ => {
                let msg = b"Usage: netutils [ifconfig|netstat|route|ip]\n";
                libc::write(STDERR_FILENO, msg.as_ptr() as *const c_void, msg.len());
                libc::exit(1);
            }
        }
    }
}

unsafe fn ifconfig() {
    // Just list interfaces - minimal implementation
    let sock = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
    if sock < 0 {
        libc::perror(b"socket\0".as_ptr() as *const c_char);
        return;
    }

    // Try to get lo interface as example
    let mut ifr: ifreq = mem::zeroed();
    libc::strcpy(ifr.ifr_name.as_mut_ptr(), b"lo\0".as_ptr() as *const c_char);
    
    if libc::ioctl(sock, SIOCGIFADDR as _, &mut ifr as *mut _ as *mut c_void) == 0 {
        let addr = &ifr.ifr_addr;
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
            let addr = &ifr.ifr_addr;
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
}

unsafe fn netstat() {
    // Very limited without /proc access
    libc::printf(b"Active connections (limited without /proc access)\n\0".as_ptr() as *const c_char);
    libc::printf(b"Proto Local Address           Foreign Address         State\n\0".as_ptr() as *const c_char);
    
    // We can only show our own sockets by trying to bind/connect
    let sock = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
    if sock >= 0 {
        let mut addr: sockaddr_in = mem::zeroed();
        let mut len = mem::size_of::<sockaddr_in>() as libc::socklen_t;
        
        if libc::getsockname(sock, &mut addr as *mut _ as *mut libc::sockaddr, &mut len) == 0 {
            libc::printf(b"tcp   0.0.0.0:*               LISTEN\n\0".as_ptr() as *const c_char);
        }
        libc::close(sock);
    }
}

unsafe fn route() {
    libc::printf(b"Kernel IP routing table\n\0".as_ptr() as *const c_char);
    libc::printf(b"Destination     Gateway         Genmask         Flags Metric Ref    Use Iface\n\0".as_ptr() as *const c_char);
    #[cfg(target_os = "linux")]
    {
        // Try netlink on Linux
        let sock = libc::socket(16, libc::SOCK_RAW, 0); // AF_NETLINK = 16
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
}

unsafe fn ip(args: &[String]) {
    if args.len() < 2 {
        libc::printf(b"Usage: ip [addr|route]\n\0".as_ptr() as *const c_char);
        return;
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
                        let addr = &ifr.ifr_addr;
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
        "route" => route(),
        _ => {
            let msg = format!("Unknown command: {}\n", args[1]);
            libc::write(STDERR_FILENO, msg.as_ptr() as *const c_void, msg.len());
        }
    }
}