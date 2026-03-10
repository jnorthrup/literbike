# Litebike / Literbike Split Chart

This chart captures the intended product boundary as local launch truth.

```mermaid
flowchart LR
    A["Clients / Agents"] --> B["litebike"]
    B --> C["literbike"]

    subgraph L["litebike"]
        L1["Edge ingress"]
        L2["Unified port / agent8888 surface"]
        L3["Local protocol classification"]
        L4["Lean proxy / router companion"]
        L5["Fast operator-facing control path"]
    end

    subgraph R["literbike"]
        R1["Heavy runtime plane"]
        R2["keymux"]
        R3["modelmux"]
        R4["QUIC / transport depth"]
        R5["API translation / adapters"]
        R6["DHT / Kademlia foundations"]
        R7["CAS / lazy projection gateway"]
        R8["Broader service orchestration"]
    end

    B --> L1
    B --> L2
    B --> L3
    B --> L4
    B --> L5

    C --> R1
    C --> R2
    C --> R3
    C --> R4
    C --> R5
    C --> R6
    C --> R7
    C --> R8
```

## Short Read

- `litebike` owns the outer shell, edge-facing ingress, and the canonical
  `agent8888` surface on port `8888`.
- That one `litebike` surface subsumes both repos when `literbike` is mounted.
- `literbike` owns the heavier heart/backplane, including `keymux`,
  `modelmux`, deeper transport, adapters, and longer-horizon service/storage
  work.
- The intended handoff is: classify early in `litebike`, then route heavier
  transport/service/runtime work into `literbike`.
