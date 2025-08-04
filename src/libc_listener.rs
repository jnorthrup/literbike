#[cfg(any(target_os = "linux", target_os = "android"))]
use libc::{
    socket, setsockopt, bind, listen, close,
    AF_INET, AF_INET6, SOCK_STREAM, SOCK_CLOEXEC, SOCK_NONBLOCK,
    SOL_SOCKET, SO_REUSEADDR, SO_REUSEPORT,
    c_int, c_void, socklen_t,
};
use std::io::{Error, Result};
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use tokio::net::TcpListener;

#[derive(Clone, Debug)]
pub struct ListenerOptions {
    pub reuse_addr: bool,
    pub reuse_port: bool,
    pub backlog: i32,
}

impl Default for ListenerOptions {
    fn default() -> Self {
        Self {
            reuse_addr: true,
            reuse_port: true,
            backlog: 128,
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn set_socket_option<T>(fd: RawFd, level: c_int, name: c_int, value: &T) -> Result<()> {
    let ret = setsockopt(
        fd,
        level,
        name,
        value as *const T as *const c_void,
        std::mem::size_of::<T>() as socklen_t,
    );
    if ret < 0 {
        return Err(Error::last_os_error());
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub async fn bind_with_options(addr: SocketAddr, options: &ListenerOptions) -> Result<TcpListener> {
    unsafe {
        let domain = match addr {
            SocketAddr::V4(_) => AF_INET,
            SocketAddr::V6(_) => AF_INET6,
        };
        
        let fd = socket(domain, SOCK_STREAM | SOCK_CLOEXEC | SOCK_NONBLOCK, 0);
        if fd < 0 {
            return Err(Error::last_os_error());
        }
        
        let close_on_error = |fd: RawFd| {
            let _ = close(fd);
        };
        
        if options.reuse_addr {
            let val: c_int = 1;
            if let Err(e) = set_socket_option(fd, SOL_SOCKET, SO_REUSEADDR, &val) {
                close_on_error(fd);
                return Err(e);
            }
        }
        
        if options.reuse_port {
            let val: c_int = 1;
            if let Err(e) = set_socket_option(fd, SOL_SOCKET, SO_REUSEPORT, &val) {
                close_on_error(fd);
                return Err(e);
            }
        }
        
        let (addr_ptr, addr_len) = match addr {
            SocketAddr::V4(v4) => {
                let mut raw_addr: libc::sockaddr_in = std::mem::zeroed();
                raw_addr.sin_family = AF_INET as u16;
                raw_addr.sin_port = v4.port().to_be();
                raw_addr.sin_addr.s_addr = u32::from_ne_bytes(v4.ip().octets());
                (
                    &raw_addr as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in>() as socklen_t,
                )
            }
            SocketAddr::V6(v6) => {
                let mut raw_addr: libc::sockaddr_in6 = std::mem::zeroed();
                raw_addr.sin6_family = AF_INET6 as u16;
                raw_addr.sin6_port = v6.port().to_be();
                raw_addr.sin6_addr.s6_addr = v6.ip().octets();
                raw_addr.sin6_flowinfo = v6.flowinfo();
                raw_addr.sin6_scope_id = v6.scope_id();
                (
                    &raw_addr as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in6>() as socklen_t,
                )
            }
        };
        
        if bind(fd, addr_ptr, addr_len) < 0 {
            let err = Error::last_os_error();
            close_on_error(fd);
            return Err(err);
        }
        
        if listen(fd, options.backlog) < 0 {
            let err = Error::last_os_error();
            close_on_error(fd);
            return Err(err);
        }
        
        Ok(TcpListener::from_raw_fd(fd))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub async fn bind_with_options(addr: SocketAddr, _options: &ListenerOptions) -> Result<TcpListener> {
    TcpListener::bind(addr).await
}