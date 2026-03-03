# QUIC Chrome Testing - Root Cause Found

## Summary

**Major Finding**: The server is sending `HANDSHAKE_DONE` and 1-RTT packets **too early** - before Chrome completes the TLS 1.3 handshake.

## Root Cause Analysis

### The Problem

1. **TLS Handshake Flow**:
   - Chrome sends: Initial packets with ClientHello
   - Server responds: Initial packets with ServerHello, Certificate, etc.
   - Chrome should send: Handshake packets with Finished
   - **But Chrome never sends Handshake packets!**

2. **Server's Logic** (quic_engine.rs:566-605):
   ```rust
   let should_send_done = {
       let test_ok = self.crypto_provider.encrypt_packet(
           EncryptionLevel::OneRtt, 0, &[], &mut vec![0u8]
       ).is_ok();
       test_ok
   };
   ```
   This checks if 1-RTT encryption is **technically possible**, not if the TLS handshake is complete!

3. **Expected Flow** (RFC 9001):
   - Client: Initial (ClientHello) → Server: Initial+Handshake (ServerHello+Finished)
   - Client: Handshake (Finished) → Server: Handshake (ACK) + 1-RTT (HANDSHAKE_DONE)
   - Client: 1-RTT (HTTP requests)

4. **Actual Flow** (from packet capture):
   - Client: Initial (ClientHello) → Server: Initial+Handshake+1-RTT (HANDSHAKE_DONE)
   - Client: Continues sending Initial packets (not Handshake)
   - Client: Sends CONNECTION_CLOSE
   - **Never reaches 1-RTT or HTTP requests**

### Evidence from Packet Capture

**Connection 1 (port 53981)**:
```
Frame 1: Client → Server: Initial (1282 bytes)  [ClientHello]
Frame 2: Client → Server: Initial (1282 bytes)  [More TLS data]
Frame 3: Server → Client: Handshake (1836 bytes) [ServerHello, Cert, etc.]
Frame 4: Server → Client: 1-RTT (59 bytes)      [HANDSHAKE_DONE - TOO EARLY!]
Frame 5-8: Client → Server: Initial (1282 bytes) [Chrome still on Initial]
Frame 8: Client → Server: Initial with CONNECTION_CLOSE
```

Chrome receives the 1-RTT packet but **never sends Handshake-level crypto** (TLS Finished).

### The Fix Required

Instead of checking if 1-RTT encryption is technically possible, the server should:

1. **Wait for client's Finished message** in Handshake-level crypto
2. **Only then** send HANDSHAKE_DONE
3. **Only then** transition to 1-RTT for application data

## Tests Run

1. ✅ **TLS Crypto Provider**: Fixed and working
2. ✅ **1-RTT Packet Encryption**: Fixed header protection offset bug
3. ✅ **Server Sends 1-RTT**: Now sending packets without errors
4. ❌ **TLS Handshake Completion**: Chrome never sends Finished message
5. ❌ **HTTP Requests**: Never reached (Chrome closes connection first)

## Next Steps

**To fix the issue**:

1. **Modify `send_handshake_responses`** in `quic_engine.rs`:
   - Don't send HANDSHAKE_DONE based on crypto provider readiness
   - Wait for Chrome to send Handshake-level crypto with TLS Finished
   - Verify TLS handshake state matches RFC 9001

2. **Add TLS state tracking**:
   - Track when client Finished is received
   - Only send HANDSHAKE_DONE after client Finished
   - Proper state machine for TLS 1.3 over QUIC

3. **Test with SSLKEYLOGFILE**:
   - Set `export SSLKEYLOGFILE=/tmp/ssl-keys.log`
   - Use Wireshark to decrypt QUIC traffic
   - Verify TLS handshake message flow

## Files Modified

- `src/quic/quic_engine.rs:686` - Fixed 1-RTT header protection sample offset
- `src/quic/tls/mod.rs:55` - Added crypto provider initialization
- `src/quic/quic_engine.rs:566-605` - Needs modification for proper HANDSHAKE_DONE timing

## Command Reference

```bash
# Test the server
export SSLKEYLOGFILE=/tmp/ssl-keys.log
cargo run --release --features='warp git2 tls-quic ring' --bin litebike -- quic-vqa 4433

# Monitor QUIC traffic
tshark -i lo -f 'udp port 4433' -w /tmp/quic-test.pcap

# Analyze packets
tshark -r /tmp/quic-test.pcap -Y 'quic' -V

# Check for HTTP requests
tshark -r /tmp/quic-test.pcap -Y 'quic.frame_type == 0x08 || quic.frame_type == 0x09'
```

## Research References

- RFC 9001: Using TLS to Secure QUIC
- Cloudflare Quiche #1680: Similar Chrome handshake issues
- QUIC Illustrated: https://quic.xargs.org/

**Status**: Server sends packets correctly, but TLS 1.3 handshake timing is wrong. Needs RFC 9001 compliance fix.