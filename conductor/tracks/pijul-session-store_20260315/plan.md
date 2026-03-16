# Track: pijul as session/snapshot store (replaces git bare repos + CouchDB)

## Objective

Replace opencode's bare git snapshot repos and literbike's CouchDB emulator
with libpijul as the session state substrate. Pijul's patch theory gives us:

- Content-addressed patches (no orphan repo accumulation — GHSA-xv3r-6x54-766h gone)
- Commuting patches (no merge conflicts across concurrent sessions)
- Native differential sync (patch feed replaces `_changes` + RequestFactory)
- Pure Rust (libpijul links directly into literbike — no subprocess, no HTTP)

## Architecture

```
opencode UI (Bun — one process, UI only)
     ↓ HTTP localhost:8888
literbike modelmux
     ↓
keymux (provider routing)
pijul channel per session (libpijul crate, embedded)
     ↓ patch feed
opencode UI differential sync (replaces SQLite drizzle + snapshot git)
```

Each conversation session = a pijul channel.
Each AI turn = a recorded patch on that channel.
Revert = unapply patch. Diff = patch contents. History = patch list.

## libpijul integration path

- `../pijul/libpijul` — source available locally at /Users/jim/work/pijul/libpijul
- Add as path dependency in literbike Cargo.toml
- `src/session/` — new module: pijul-backed session store
  - `open_channel(session_id)` → ChannelRef
  - `record_turn(channel, role, content)` → Hash (patch hash)
  - `patch_feed(channel, from_hash)` → Vec<Patch> (differential sync)
  - `revert_turn(channel, hash)` → applies inverse patch

## Replaces

- `~/.local/share/opencode/snapshot/` bare git repos (entire dir goes away)
- `src/couchdb/` in literbike (CouchDB emulator no longer needed for sessions)
- `src/request_factory/` (RequestFactory RPC replaced by pijul patch feed)

## Status

- [ ] Add libpijul path dep to literbike/Cargo.toml
- [ ] `src/session/mod.rs` — pijul-backed session channel open/record/revert
- [ ] `src/session/feed.rs` — patch feed endpoint (GET /session/:id/patches)
- [ ] Wire modelmux proxy to record turns via session module
- [ ] opencode: replace snapshot/index.ts git calls with pijul patch feed HTTP calls
- [ ] Verify: no bare git repos created after a full opencode session
