# LiterbikeControlPlane

macOS menu bar app for modelmux. Vrod icon, KEYMUX/PROVIDERS model hierarchy, live draw-through from provider APIs.

## Requirements

- macOS 13+ (arm64)
- Rust toolchain (`cargo`)
- Swift (`swiftc`, ships with Xcode CLT)
- API keys in environment (any combination of):
  `OPENAI_API_KEY`, `OPENROUTER_API_KEY`, `NVIDIA_API_KEY`, `DEEPSEEK_API_KEY`,
  `CEREBRAS_API_KEY`, `GROQ_API_KEY`, `XAI_API_KEY`, `GEMINI_API_KEY`,
  `PERPLEXITY_API_KEY`, `MOONSHOT_API_KEY`, `HUGGINGFACE_API_KEY`, `ARCEE_API_KEY`

## Build

```bash
cd /path/to/literbike

# Build modelmux backend
cargo build --bin modelmux

# Build Swift menu bar app
cd macos/LiterbikeControlPlane
swiftc -O -o LiterbikeControlPlane -framework AppKit -framework Foundation -framework Network Sources/main.swift
```

## Run

Must launch from the project root (icon path is relative to cwd):

```bash
cd /path/to/literbike

# Start modelmux (background)
./target/debug/modelmux &

# Wait for modelmux to bind :8888, then start the menu bar app
sleep 2 && ./macos/LiterbikeControlPlane/LiterbikeControlPlane &
```

## One-liner

```bash
cd /path/to/literbike && ./target/debug/modelmux & sleep 2 && ./macos/LiterbikeControlPlane/LiterbikeControlPlane &
```

## What it does

- **modelmux** listens on `127.0.0.1:8888` with OpenAI-compatible `/v1` endpoints
- On first request, draw-through fetches `/models` from every provider with a set API key
- **LiterbikeControlPlane** polls `/toolbar/state` every 5s and builds the menu:
  - **KEYMUX** -- groups models by API key. Click "FETCH" on unfetched providers to trigger draw-through.
  - **PROVIDERS** -- `PROVIDER / models / V1 / [MODEL]` hierarchy. Click a model to POST `/probe`.

## Stop

```bash
pkill -f LiterbikeControlPlane
pkill -f 'target/debug/modelmux'
```

## Endpoints

| Path | Method | Description |
|---|---|---|
| `/v1/models` | GET | OpenAI-compatible model list (triggers draw-through on cache miss) |
| `/v1/chat/completions` | POST | Chat completions proxy |
| `/toolbar/state` | GET | Menu bar state JSON |
| `/toolbar/actions` | POST | `{"action":"rescan_env"}` etc. |
| `/control/state` | GET | Full gateway state with keymux wiring |
| `/health` | GET | Health check |

## Icon

`literbike-vrod-icon.svg` in project root. Rendered as macOS template image (adapts to light/dark menu bar).
