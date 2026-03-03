# LiteBike QUIC Implementation Journal

## Current Status: HTTP/3 Response Sent but Not Received by Client

### What Works
1. **TLS Handshake**: Complete - 1-RTT keys established successfully
2. **HANDSHAKE_DONE + SETTINGS**: Sent correctly before HTTP/3 request
3. **HTTP/3 Request**: Server receives request on stream 0
4. **HTTP/3 Response**: Server sends 200 OK with 554243 bytes (bw_test_pattern.png)

### Critical Issues Remaining

#### Issue 1: remote_cid Empty in 1-RTT Packets
**Log Evidence**:
```
1-RTT packet: DCID=[82, 59, bc, 84, e2, ef, 7f, 67], remote_cid=[]
```

**Root Cause**: For 1-RTT (short header) packets, SCID is not present in the packet. The server extracts CIDs from the packet header, but for existing connections, it should use stored CIDs from connection state.

**Fix Applied**: Modified CID extraction in `quic_server.rs` to use stored CIDs for existing connections:
```rust
let (client_scid, client_dcid) = {
    let conns = connections.lock();
    if let Some(existing) = conns.get(&remote_addr) {
        let state = existing.get_state();
        (state.remote_connection_id.bytes.clone(), state.local_connection_id.bytes.clone())
    } else {
        // New connection - extract from packet header
        (received_packet.header.source_connection_id.bytes.clone(),
         received_packet.header.destination_connection_id.bytes.clone())
    }
};
```

**Status**: DCID is now populated, but remote_cid still shows empty in debug prints from `send_1rtt_frames`. Need to verify the QuicEngine state is properly storing remote_cid.

#### Issue 2: Response Packets Lost (Chrome Times Out)
**Log Evidence**:
```
Sent HTTP/3 200 response (554243 bytes body) on stream 0
```
But client never receives the response.

**Possible Causes**:
1. DCID mismatch in 1-RTT packets
2. Packet number handling
3. Header protection issues
4. Connection ID length mismatch

#### Issue 3: Curl Segfaults After TLS Handshake
Curl's HTTP/3 implementation crashes after TLS completes but before receiving response. This may be related to malformed QUIC frames or SETTINGS format.

### Recent Fixes Applied

#### Fix 1: Packet Type Detection (packet_type_bits)
**Location**: `src/quic/quic_server.rs` lines 139, 440
**Change**: `(first_byte >> 5) & 0x03` → `(first_byte >> 4) & 0x03`
**Reason**: QUIC packet type bits are in bits 5-4, not 6-5

#### Fix 2: Ping Frame ACK
**Location**: `src/quic/quic_engine.rs` line 327
**Added**: Ping frames now trigger ACKs (ack-eliciting)

#### Fix 3: CID Extraction for Existing Connections
**Location**: `src/quic/quic_server.rs` lines 677-692
**Change**: For existing connections, use stored CIDs instead of packet header

### Debug Commands

```bash
# Monitor server logs
strings /tmp/server.log | grep -E "(DCID|remote_cid|Sent HTTP|Stream frame)"

# Test with curl
/opt/homebrew/opt/curl/bin/curl --http3-only -k https://127.0.0.1:4433/

# Test with Chrome (copy command from server output)
/Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome \
    --user-data-dir=/tmp/chrome-quic \
    --origin-to-force-quic-on=127.0.0.1:4433 \
    --ignore-certificate-errors \
    --enable-quic \
    https://127.0.0.1:4433/
```

### Next Steps
1. Verify QuicEngine state properly stores remote_connection_id during creation
2. Add debug logging to confirm CIDs are set in QuicEngine::new()
3. Check if 1-RTT packets are actually being received by client (network capture)
4. Verify STREAM frame format conforms to RFC 9000

## Packet Capture Evidence (2026-03-01)

### tshark Loopback Capture Session
Captured on macOS loopback (`lo0`) for QUIC test traffic on UDP 4433.

```bash
bash-3.2$ sudo tshark -i lo0 -f "udp port 4433" -s 0 -w /tmp/literbike-quic-$(date +%Y%m%d-%H%M%S).pcapng
Capturing on 'Loopback: lo0'
109 ^Z
[1]+  Stopped                 sudo tshark -i lo0 -f "udp port 4433" -s 0 -w /tmp/literbike-quic-$(date +%Y%m%d-%H%M%S).pcapng
bash-3.2$ sudo chown jim:staff /tmp/literbike-quic-*.pcapng
Password:
bash-3.2$ fg
sudo tshark -i lo0 -f "udp port 4433" -s 0 -w /tmp/literbike-quic-$(date +%Y%m%d-%H%M%S).pcapng
8570
```

### Notes
- The capture was successfully started on `lo0` with full packet snaplen (`-s 0`) and file output to `/tmp/literbike-quic-*.pcapng`.
- Job-control flow was validated:
  - `^Z` stopped capture after observed counter `109`.
  - Ownership fix (`chown jim:staff`) was applied while stopped.
  - `fg` resumed capture in foreground.
- Observed resumed counter/output `8570` confirms ongoing packet capture activity after resume.
