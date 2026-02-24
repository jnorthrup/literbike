#![cfg(feature = "warp")]

use literbike::knox_proxy;
use std::io;

#[test]
fn parse_socks5_ipv4() -> io::Result<()> {
    // Build a minimal SOCKS5 connect request for IPv4: VER(5), CMD(1), RSV(0), ATYP(1=IPv4), ADDR(4), PORT(2)
    let mut buf = vec![0u8; 10];
    buf[0] = 0x05; // VER
    buf[1] = 0x01; // CMD CONNECT
    buf[2] = 0x00; // RSV
    buf[3] = 0x01; // ATYP = IPv4
    buf[4] = 192; buf[5] = 0; buf[6] = 2; buf[7] = 1; // 192.0.2.1
    buf[8] = 0x1F; buf[9] = 0x90; // port 8080

    let addr = knox_proxy::parse_socks5_target(&buf, buf.len())?;
    assert_eq!(addr, "192.0.2.1:8080");
    Ok(())
}

#[test]
fn parse_socks5_domain() -> io::Result<()> {
    // Domain example: VER, CMD, RSV, ATYP=3, LEN, DOMAIN..., PORT
    let domain = b"example.com";
    let domain_len = domain.len() as u8;
    let mut buf = vec![0u8; 7 + domain.len()];
    buf[0] = 0x05; buf[1] = 0x01; buf[2] = 0x00; buf[3] = 0x03; // domain
    buf[4] = domain_len;
    buf[5..5 + domain.len()].copy_from_slice(domain);
    let port_index = 5 + domain.len();
    buf[port_index] = 0x00; buf[port_index + 1] = 0x50; // port 80

    let addr = knox_proxy::parse_socks5_target(&buf, buf.len())?;
    assert_eq!(addr, "example.com:80");
    Ok(())
}

#[test]
fn parse_socks5_ipv6() -> io::Result<()> {
    // IPv6 example: 2001:db8::1
    let ipv6_octets: [u8; 16] = [
        0x20, 0x01, 0x0d, 0xb8,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x01,
    ];
    // Build buffer: VER, CMD, RSV, ATYP(0x04), 16 bytes addr, 2 bytes port
    let mut buf = vec![0u8; 4 + 16 + 2];
    buf[0] = 0x05; buf[1] = 0x01; buf[2] = 0x00; buf[3] = 0x04;
    buf[4..20].copy_from_slice(&ipv6_octets);
    buf[20] = 0x1F; buf[21] = 0x90; // port 8080

    let addr = knox_proxy::parse_socks5_target(&buf, buf.len())?;
    assert_eq!(addr, "[2001:db8::1]:8080");
    Ok(())
}
