#[cfg(any(target_os = "linux", target_os = "android"))]
use libc::{accept4, setsockopt, c_int, c_void, socklen_t};
#[cfg(any(target_os = "linux", target_os = "android"))]
use libc::{
    SOCK_CLOEXEC, SOCK_NONBLOCK,
    SOL_SOCKET, SO_KEEPALIVE, SO_RCVBUF, SO_SNDBUF,
    IPPROTO_TCP, TCP_NODELAY, TCP_KEEPIDLE, TCP_KEEPINTVL, TCP_KEEPCNT,
};
use std::io::{Error, Result};
use std::os::unix::io::{AsRawFd, RawFd};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Debug)]
pub struct TcpTuningOptions {
    pub nodelay: bool,
    pub keepalive: bool,
    pub keepalive_idle_secs: Option<u32>,
    pub keepalive_interval_secs: Option<u32>,
    pub keepalive_count: Option<u32>,
    pub send_buffer_size: Option<u32>,
    pub recv_buffer_size: Option<u32>,
}

impl Default for TcpTuningOptions {
    fn default() -> Self {
        Self {
            nodelay: true,
            keepalive: true,
            keepalive_idle_secs: Some(30),
            keepalive_interval_secs: Some(10),
            keepalive_count: Some(3),
            send_buffer_size: None,
            recv_buffer_size: None,
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
pub async fn accept_with_options(
    listener: &TcpListener,
    options: &TcpTuningOptions,
) -> Result<(TcpStream, std::net::SocketAddr)> {
    loop {
        let (stream, addr) = listener.accept().await?;
        let fd = stream.as_raw_fd();
        
        unsafe {
            apply_socket_options(fd, options)?;
        }
        
        return Ok((stream, addr));
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub unsafe fn apply_socket_options(fd: RawFd, options: &TcpTuningOptions) -> Result<()> {
    if options.nodelay {
        let val: c_int = 1;
        set_socket_option(fd, IPPROTO_TCP, TCP_NODELAY, &val)?;
    }
    
    if options.keepalive {
        let val: c_int = 1;
        set_socket_option(fd, SOL_SOCKET, SO_KEEPALIVE, &val)?;
        
        if let Some(idle) = options.keepalive_idle_secs {
            set_socket_option(fd, IPPROTO_TCP, TCP_KEEPIDLE, &(idle as c_int))?;
        }
        
        if let Some(interval) = options.keepalive_interval_secs {
            set_socket_option(fd, IPPROTO_TCP, TCP_KEEPINTVL, &(interval as c_int))?;
        }
        
        if let Some(count) = options.keepalive_count {
            set_socket_option(fd, IPPROTO_TCP, TCP_KEEPCNT, &(count as c_int))?;
        }
    }
    
    if let Some(size) = options.send_buffer_size {
        set_socket_option(fd, SOL_SOCKET, SO_SNDBUF, &(size as c_int))?;
    }
    
    if let Some(size) = options.recv_buffer_size {
        set_socket_option(fd, SOL_SOCKET, SO_RCVBUF, &(size as c_int))?;
    }
    
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub async fn accept_with_options(
    listener: &TcpListener,
    _options: &TcpTuningOptions,
) -> Result<(TcpStream, std::net::SocketAddr)> {
    listener.accept().await
}

pub fn apply_stream_options(stream: &TcpStream, options: &TcpTuningOptions) -> Result<()> {
    if options.nodelay {
        stream.set_nodelay(true)?;
    }
    
    #[cfg(any(target_os = "linux", target_os = "android"))]
    unsafe {
        let fd = stream.as_raw_fd();
        
        if options.keepalive {
            let val: c_int = 1;
            set_socket_option(fd, SOL_SOCKET, SO_KEEPALIVE, &val)?;
            
            if let Some(idle) = options.keepalive_idle_secs {
                set_socket_option(fd, IPPROTO_TCP, TCP_KEEPIDLE, &(idle as c_int))?;
            }
            
            if let Some(interval) = options.keepalive_interval_secs {
                set_socket_option(fd, IPPROTO_TCP, TCP_KEEPINTVL, &(interval as c_int))?;
            }
            
            if let Some(count) = options.keepalive_count {
                set_socket_option(fd, IPPROTO_TCP, TCP_KEEPCNT, &(count as c_int))?;
            }
        }
        
        if let Some(size) = options.send_buffer_size {
            set_socket_option(fd, SOL_SOCKET, SO_SNDBUF, &(size as c_int))?;
        }
        
        if let Some(size) = options.recv_buffer_size {
            set_socket_option(fd, SOL_SOCKET, SO_RCVBUF, &(size as c_int))?;
        }
    }
    
    Ok(())
}