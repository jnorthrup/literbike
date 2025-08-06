//! Enhanced egress connector with backoff logic integration

use std::io;
use std::sync::Arc;
use tokio::net::TcpStream;
use log::{debug, warn};

use crate::egress_backoff::{EgressManager, handle_with_backoff};

#[cfg(any(target_os = "android", target_os = "linux"))]
use std::os::fd::{FromRawFd, IntoRawFd};
#[cfg(any(target_os = "android", target_os = "linux"))]
use libc;

/// Global egress manager instance
lazy_static::lazy_static! {
    static ref EGRESS_MANAGER: Arc<tokio::sync::Mutex<EgressManager>> = {
        let mut manager = EgressManager::new();
        
        manager.add_egress("rmnet_data0".to_string(), true);
        manager.add_egress("rmnet_data1".to_string(), false);
        manager.add_egress("rmnet_data2".to_string(), false);
        
        Arc::new(tokio::sync::Mutex::new(manager))
    };
}

/// Connect with automatic egress selection and backoff
pub async fn connect_with_backoff(target: &str) -> io::Result<TcpStream> {
    let manager = EGRESS_MANAGER.lock().await;
    

    let owned_target = Arc::new(target.to_string()); // Use Arc to share ownership

    handle_with_backoff(&*manager, |egress_name| {
        let target_for_future = owned_target.clone();
        let egress_name_owned = egress_name.to_string();
        async move {
            connect_via_specific_egress(&target_for_future, &egress_name_owned).await
        }
    }).await
}

/// Connect via a specific egress path
async fn connect_via_specific_egress(target: &str, egress_name: &str) -> io::Result<TcpStream> {
    debug!("Attempting connection to {} via egress: {}", target, egress_name);
    
    // Parse target
    let (host, port_str) = target.rsplit_once(':')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid target format"))?;
    let port: u16 = port_str.parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid port"))?;

    // Resolve address
    let mut addrs = tokio::net::lookup_host(format!("{}:{}", host, port)).await?;
    let addr = addrs.next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No address resolved"))?;

    // Platform-specific connection
    #[cfg(any(target_os = "android", target_os = "linux"))]
    {
        connect_via_egress_linux(addr, egress_name).await
    }
    
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        // Fallback for other platforms
        let _ = egress_name;
        TcpStream::connect(addr).await
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
async fn connect_via_egress_linux(addr: SocketAddr, egress_name: &str) -> io::Result<TcpStream> {
    unsafe {
        // Create socket
        let domain = match addr {
            SocketAddr::V4(_) => libc::AF_INET,
            SocketAddr::V6(_) => libc::AF_INET6,
        };
        
        let fd = libc::socket(domain, libc::SOCK_STREAM | libc::SOCK_NONBLOCK, 0);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Auto-close on error
        struct FdGuard(i32);
        impl Drop for FdGuard {
            fn drop(&mut self) {
                if self.0 >= 0 {
                    unsafe { libc::close(self.0); }
                }
            }
        }
        let mut guard = FdGuard(fd);

        // Configure socket based on egress
        configure_socket_for_egress(fd, egress_name, &addr)?;

        // Prepare sockaddr
        let (sockaddr_ptr, socklen) = match addr {
            SocketAddr::V4(v4) => {
                let mut sa: libc::sockaddr_in = std::mem::zeroed();
                sa.sin_family = libc::AF_INET as u16;
                sa.sin_port = u16::to_be(v4.port());
                sa.sin_addr = libc::in_addr { 
                    s_addr: u32::from_ne_bytes(v4.ip().octets()) 
                };
                (
                    &sa as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t
                )
            }
            SocketAddr::V6(v6) => {
                let mut sa: libc::sockaddr_in6 = std::mem::zeroed();
                sa.sin6_family = libc::AF_INET6 as u16;
                sa.sin6_port = u16::to_be(v6.port());
                sa.sin6_addr = libc::in6_addr { 
                    s6_addr: v6.ip().octets() 
                };
                sa.sin6_flowinfo = v6.flowinfo();
                sa.sin6_scope_id = v6.scope_id();
                (
                    &sa as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t
                )
            }
        };

        // Connect
        let ret = libc::connect(fd, sockaddr_ptr, socklen);
        if ret != 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::EINPROGRESS) {
                return Err(err);
            }
        }

        // Convert to Tokio TcpStream
        let std_stream = std::net::TcpStream::from_raw_fd(fd);
        guard.0 = -1; // Prevent double-close
        std_stream.set_nonblocking(true)?;
        
        Ok(TcpStream::from_std(std_stream)?)
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn configure_socket_for_egress(fd: i32, egress_name: &str, addr: &SocketAddr) -> io::Result<()> {
    unsafe {
        // Try to bind to specific interface
        match egress_name {
            "rmnet_data0" | "rmnet_data1" | "rmnet_data2" | "wlan0" | "swlan0" => {
                // Android interfaces - use SO_BINDTODEVICE
                let ifname = std::ffi::CString::new(egress_name)?;
                let ret = libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_BINDTODEVICE,
                    ifname.as_ptr() as *const libc::c_void,
                    ifname.as_bytes_with_nul().len() as libc::socklen_t
                );
                
                if ret != 0 {
                    warn!("SO_BINDTODEVICE({}) failed: {}", egress_name, io::Error::last_os_error());
                } else {
                    info!("Bound to interface: {}", egress_name);
                }
            }
            "primary" => {
                // Try to use primary network interface
                if let Ok(ip_str) = std::env::var("PRIMARY_EGRESS_IP") {
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        bind_to_ip(fd, ip, addr)?;
                    }
                }
            }
            "secondary" => {
                // Try to use secondary network interface
                if let Ok(ip_str) = std::env::var("SECONDARY_EGRESS_IP") {
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        bind_to_ip(fd, ip, addr)?;
                    }
                }
            }
            _ => {
                debug!("Unknown egress name: {}", egress_name);
            }
        }

        // Set socket options for better performance
        set_performance_options(fd)?;
        
        Ok(())
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn bind_to_ip(fd: i32, ip: IpAddr, target_addr: &SocketAddr) -> io::Result<()> {
    unsafe {
        match (ip, target_addr) {
            (IpAddr::V4(ipv4), SocketAddr::V4(_)) => {
                let mut sa: libc::sockaddr_in = std::mem::zeroed();
                sa.sin_family = libc::AF_INET as u16;
                sa.sin_port = 0; // Ephemeral port
                sa.sin_addr = libc::in_addr { 
                    s_addr: u32::from_ne_bytes(ipv4.octets()) 
                };
                
                let ret = libc::bind(
                    fd,
                    &sa as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t
                );
                
                if ret != 0 {
                    return Err(io::Error::last_os_error());
                }
                debug!("Bound to IPv4: {}", ipv4);
            }
            (IpAddr::V6(ipv6), SocketAddr::V6(_)) => {
                let mut sa: libc::sockaddr_in6 = std::mem::zeroed();
                sa.sin6_family = libc::AF_INET6 as u16;
                sa.sin6_port = 0; // Ephemeral port
                sa.sin6_addr = libc::in6_addr { 
                    s6_addr: ipv6.octets() 
                };
                
                let ret = libc::bind(
                    fd,
                    &sa as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t
                );
                
                if ret != 0 {
                    return Err(io::Error::last_os_error());
                }
                debug!("Bound to IPv6: {}", ipv6);
            }
            _ => {
                debug!("IP family mismatch, skipping bind");
            }
        }
        Ok(())
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn set_performance_options(fd: i32) -> io::Result<()> {
    unsafe {
        // Enable TCP keepalive
        let keepalive = 1i32;
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_KEEPALIVE,
            &keepalive as *const _ as *const libc::c_void,
            std::mem::size_of::<i32>() as libc::socklen_t
        );

        // Set TCP nodelay for low latency
        let nodelay = 1i32;
        libc::setsockopt(
            fd,
            libc::IPPROTO_TCP,
            libc::TCP_NODELAY,
            &nodelay as *const _ as *const libc::c_void,
            std::mem::size_of::<i32>() as libc::socklen_t
        );

        // Set socket buffer sizes for better throughput
        let sndbuf = 1048576i32; // 1MB
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            &sndbuf as *const _ as *const libc::c_void,
            std::mem::size_of::<i32>() as libc::socklen_t
        );

        let rcvbuf = 1048576i32; // 1MB
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVBUF,
            &rcvbuf as *const _ as *const libc::c_void,
            std::mem::size_of::<i32>() as libc::socklen_t
        );

        Ok(())
    }
}

/// Start background health checker
pub async fn start_health_checker() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        
        loop {
            interval.tick().await;
            
            let manager = EGRESS_MANAGER.lock().await;
            manager.health_check_all().await;
            
            // Log stats periodically
            let stats = manager.get_stats();
            for (name, stat) in stats {
                if stat.error_rate > 10.0 {
                    warn!(
                        "Egress {} has high error rate: {:.1}% ({} requests, state: {:?})",
                        name, stat.error_rate, stat.total_requests, stat.state
                    );
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_with_backoff() {
        // This would need a test server
        let result = connect_with_backoff("127.0.0.1:8080").await;
        // Just check it attempts connection
        assert!(result.is_err() || result.is_ok());
    }
}