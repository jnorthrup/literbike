#[cfg(any(target_os = "linux", target_os = "android"))]
use libc::{getrandom, GRND_NONBLOCK};
use libc::{open, read, close, O_RDONLY};
use std::io::{self, Error};
use std::ptr;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn random_bytes(buf: &mut [u8]) -> io::Result<()> {
    let len = buf.len();
    if len == 0 {
        return Ok(());
    }
    
    // Try getrandom() syscall first (Linux 3.17+)
    let ret = unsafe {
        getrandom(
            buf.as_mut_ptr() as *mut libc::c_void,
            len,
            GRND_NONBLOCK,
        )
    };
    
    if ret >= 0 && ret as usize == len {
        return Ok(());
    }
    
    // Fall back to /dev/urandom
    random_bytes_urandom(buf)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn random_bytes(buf: &mut [u8]) -> io::Result<()> {
    random_bytes_urandom(buf)
}

fn random_bytes_urandom(buf: &mut [u8]) -> io::Result<()> {
    unsafe {
        let fd = open(b"/dev/urandom\0".as_ptr() as *const libc::c_char, O_RDONLY);
        if fd < 0 {
            return Err(Error::last_os_error());
        }
        
        let mut total = 0;
        let len = buf.len();
        
        while total < len {
            let ret = read(
                fd,
                buf.as_mut_ptr().add(total) as *mut libc::c_void,
                len - total,
            );
            
            if ret < 0 {
                let err = Error::last_os_error();
                close(fd);
                return Err(err);
            }
            
            if ret == 0 {
                close(fd);
                return Err(Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "EOF reading /dev/urandom",
                ));
            }
            
            total += ret as usize;
        }
        
        close(fd);
        Ok(())
    }
}

pub fn random_u8() -> io::Result<u8> {
    let mut buf = [0u8; 1];
    random_bytes(&mut buf)?;
    Ok(buf[0])
}

pub fn random_u16() -> io::Result<u16> {
    let mut buf = [0u8; 2];
    random_bytes(&mut buf)?;
    Ok(u16::from_ne_bytes(buf))
}

pub fn random_u32() -> io::Result<u32> {
    let mut buf = [0u8; 4];
    random_bytes(&mut buf)?;
    Ok(u32::from_ne_bytes(buf))
}

pub fn random_u64() -> io::Result<u64> {
    let mut buf = [0u8; 8];
    random_bytes(&mut buf)?;
    Ok(u64::from_ne_bytes(buf))
}

pub fn random_range(min: u32, max: u32) -> io::Result<u32> {
    if min >= max {
        return Ok(min);
    }
    
    let range = max - min;
    let mut val = random_u32()?;
    
    // Simple modulo bias reduction
    let limit = u32::MAX - (u32::MAX % range);
    while val >= limit {
        val = random_u32()?;
    }
    
    Ok(min + (val % range))
}

pub fn fill_random(buf: &mut [u8]) -> io::Result<()> {
    random_bytes(buf)
}