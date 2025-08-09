# Android per-network routing domains (IPv6 exclusive on cell)

This doc captures the per-network “exclusive domain” model and an overlay based on the device observed during testing.

## Model

```mermaid
flowchart LR
    subgraph Apps["Apps & Sockets"]
      A4["IPv4 sockets"] --> PBR
      A6["IPv6 sockets"] --> PBR
      DNS["Resolver (AAAA/A)"] --> PBR
    end

    PBR["Policy Routing (fwmark + ip rule per netId)"]

  subgraph WiFi["Wi‑Fi domain swlan0/wlan0"]
      W4["IPv4 addr (e.g., 192.168.225.152)"]
      W6["IPv6 addr (often none)"]
      WGW4["v4 default via home router"]
    end

    subgraph Cell["Cellular domain (rmnet_data7)"]
      C4["IPv4 CGNAT (e.g., 100.99.37.173)"]
      C6["IPv6 global (e.g., 2600:…)"]
  CGW6["v6 default via link‑local fe80::rmnet_data7"]
      CGW4["v4 default via CGNAT 100.99.37.1 (heuristic)"]
    end

    subgraph VPN["VPN domain (if active)"]
      V4["v4 tunnel"]
      V6["v6 tunnel"]
    end

    PBR -->|select netId| WiFi
    PBR -->|select netId| Cell
    PBR -->|select netId| VPN

  A6 -->|AAAA| Cell
  A4 -->|A| WiFi
  A4 -->|A_fallback| Cell
```

Notes

- Each active network has its own routing domain; sockets are steered per‑flow.
- IPv6 is commonly provisioned only on the cellular PDN; the v6 default gateway is link‑local on the iface.
- On managed devices (e.g., Knox), routing tables and netlink may be hidden; detection uses layered fallbacks.

## Overlay (example from this device)

- Wi‑Fi: `swlan0` IPv4 `192.168.225.152`
- Cellular: `rmnet_data7` IPv4 `100.99.37.173`, IPv6 `2600:1007:...`
- Default v4: `100.99.37.1` (derived)
- Default v6: not readable under policy; egress iface hint: `rmnet_data7`

Captured: 2025‑08‑09
