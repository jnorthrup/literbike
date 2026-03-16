# Track: modelmux as desktop AI proxy (replaces opencode/claude shell guts)

## Objective

Replace opencode's scattered Node.js model routing with literbike's
modelmux/keymux as a resident desktop daemon. No JS VM in the routing
path. No node connection leaks. No Gigacage VSZ blowup from spawning
multiple Bun processes per session.

## Context

- opencode uses Bun as runtime; each session spawns Bun → JSC Gigacage
  reserves ~485GB VSZ per process (bun issue #28138)
- opencode snapshots accumulate in ~/.local/share/opencode/snapshot/
  with no TTL → 200GB+ leak over time (GHSA-xv3r-6x54-766h)
- opencode's provider routing is ad-hoc fetch() calls with no quota
  tracking, no health state, no fallback precedence

## Approach: no fork, no bun PR, no opencode split

literbike runs as a sidecar at localhost:8888.
opencode's existing literbike provider stub already points there.
No changes to bun. No fork of opencode required.
Upstream opencode gets a PR only for the snapshot retention fix
(GHSA-xv3r-6x54-766h) — scope-limited, mergeable standalone.

## Status

- [x] keymux::dsel: route(), discover_providers(), get_provider(),
      is_real_key_pub(), track_tokens(), all_provider_quotas()
- [x] ModelRegistry::register_builtin_providers() delegates to dsel
      (single source of truth, no duplicate URL tables)
- [x] proxy: Anthropic x-api-key header + /v1/messages routing
- [x] src/bin/modelmux.rs — daemon at :8888, auto-discovers keys from env
- [x] branch: claude/modelmux-keymux-wiring pushed to origin

## Remaining

- [ ] Streaming: server-sent events passthrough for long completions
- [ ] macOS menubar icon (keymux replaces cc-switch, needs NSStatusItem)
- [ ] LaunchAgent plist for auto-start at login
- [ ] @literbike/ai-sdk-provider npm shim (Vercel AI SDK compat for opencode)
- [ ] Test: ANTHROPIC_API_KEY set → curl localhost:8888/v1/chat/completions

## Bun relationship (read-only, no PR needed)

bun issue #28138: Gigacage reserves ~485GB VSZ per process on Apple Silicon.
Fix exists at WebKit level (hasCapacityToUseLargeGigacage flag) but requires
rebuilding JSC. GIGACAGE_ENABLED=0 reduces VSZ at cost of security feature.
Mitigation: fewer Bun processes. modelmux removes the need for Bun in the
model routing path entirely — one fewer JSC instance per session.

Track the issue, do not open a bun PR until WebKit patch is ready.
