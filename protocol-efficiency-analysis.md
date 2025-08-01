# Protocol Efficiency Analysis: LiteBike Proxy

## Current Implementation Stats

### Binary Size: 1.2MB (stripped, release build)

### Protocols Supported:
1. **HTTP/HTTPS** - Full proxy with CONNECT tunneling
2. **SOCKS5** - RFC 1928 compliant implementation  
3. **TLS** - Direct passthrough with detection
4. **DNS-over-HTTPS** - Via trust-dns resolver
5. **UPnP** - Port forwarding via igd-next
6. **Bonjour/mDNS** - Service discovery (in main binary)
7. **Universal Detection** - Single port handles multiple protocols

### Dependency Analysis:

#### Core Dependencies (10 direct):
- `tokio` - Async runtime (essential)
- `trust-dns-resolver` - DoH + mDNS support
- `pnet` - Network interface detection
- `igd-next` - UPnP/NAT traversal
- `log` + `env_logger` - Logging
- `serde` + `serde_json` - Config/protocol parsing
- `httparse` - HTTP parsing
- `libc` - System calls

#### Transitive Dependencies: ~140 total

### Protocol Implementation Complexity:

```
HTTP Handler: ~35 lines
SOCKS5 Handler: ~90 lines  
TLS Handler: ~35 lines
Protocol Detection: ~32 lines
Universal Handler: ~30 lines
Total Core Logic: ~222 lines
```

### Efficiency Metrics:

**Current**: 7 protocols / 10 dependencies = **0.7 protocols per dependency**

**Binary overhead per protocol**: 1.2MB / 7 = **~171KB per protocol**

## Optimization Potential with Tight Tokenizer

### Proposed Minimal Dependencies (5):
1. `tokio` - Keep async runtime
2. `minimal-dns` - Custom DNS packet parser (~500 lines)
3. `micro-upnp` - Minimal SOAP messages (~300 lines)
4. `libc` - System calls
5. `log` - Logging

### Custom Protocol Parsers:
- **HTTP**: Hand-rolled parser (~200 lines)
- **SOCKS5**: Byte-level parser (~150 lines)
- **DNS**: Minimal packet encoder/decoder (~500 lines)
- **UPnP**: Template-based SOAP (~300 lines)
- **mDNS**: Subset of DNS (~200 lines)
- **TLS**: Detection only (~50 lines)

### Projected Efficiency:
**Target**: 7 protocols / 5 dependencies = **1.4 protocols per dependency**

**Estimated binary size**: ~800KB (33% reduction)

## Trade-offs:

### Current Approach (Full Dependencies):
✅ Battle-tested implementations
✅ Full protocol compliance
✅ Extensive feature support
❌ Larger binary size
❌ More attack surface

### Tight Tokenizer Approach:
✅ Minimal binary size
✅ Reduced attack surface
✅ Full control over parsing
❌ More code to maintain
❌ Potential compliance issues
❌ Missing edge cases

## Conclusion:

The current implementation achieves good efficiency (0.7 protocols/dep) with proven libraries. A custom tokenizer approach could double this efficiency but requires significant engineering effort for marginal gains. The 1.2MB binary is already impressively small for supporting 7 complex protocols.

For mobile/embedded use cases like Termux, the current approach strikes a good balance between size and reliability.