# LiteBike: Resilient Network Toolkit

This project is a ground-up rewrite focused on creating a maximally resilient, portable, and dependency-free set of network utilities. Our development is guided by the following core priorities.

## Core Priorities@

### 1. Stealth Command & Control on Port 8888

*   **Goal:** Establish a   data tunneling channel which is harmless and lacks any security apis or claims, excepting what is proxied with libssl.when the host is a default port on 8888 we can optimize this to nc or ssh tunnelling from localhost 8888 

### 2. Foundational Network Tools on Direct Syscalls

*   **Goal:** Build all network utilities directly on kernel-level syscalls for ultimate portability and resilience.
*   **Implementation:**
    *   All core logic for interface enumeration, address management, and socket operations will be implemented in `src/syscall_net.rs`.
    *   We will bypass standard libraries where possible, interacting directly with the OS via the `libc` crate to avoid abstractions and potential points of failure or restriction.

### 3. Rigorous Interface Compatibility Testing

*   **Goal:** Ensure the network tools are reliable across a wide spectrum of real-world and virtual network configurations.
*   **Implementation:**
    *   **Legacy Interfaces:** Test against standard hardware interfaces (`en0`, `eth0`).
    *   **Alternate Interfaces:** Validate functionality on virtual interfaces (VPNs, tunnels), aggregated links, and platform-specific interfaces (e.g., Android's `rmnet_data`, `swlan0`).

### 4. Self-Contained, Minimal TLS

*   **Goal:** Implement the necessary TLS 1.3 components for the C2 channel without relying on external cryptographic libraries like OpenSSL.
*   **Implementation:**
    *   Develop a minimal, self-contained TLS handshake and record layer processing logic.
    *   Source and integrate basic, audited cryptographic primitives for AES and ChaCha20 directly into the codebase, ensuring no external library dependencies are needed.

### 5. Zero-Dependency Binary

*   **Goal:** Create a single, static, and highly portable binary with no external runtime dependencies.
*   **Implementation:**
    *   The only compile-time dependency is `libc`, the direct interface to the kernel.
    *   The final compiled binary will be self-contained, requiring nothing more than a compatible kernel to run. This eliminates supply chain risks and maximizes portability.

### 6. Temporary Git Remote Management

#6 temporary git remote e.g. deploying to host using ssh and git
