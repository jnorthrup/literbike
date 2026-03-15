# Track: ModelMux session storage via CouchDB

Replace stateless per-request ModelMux with CouchDB-backed session store.
Each conversation = CouchDB document with _rev for optimistic concurrency.
Enables RequestFactory differential sync for UI clients.

## Scope
- `src/modelmux/proxy.rs` — write assistant/user turns to session doc
- `src/couchdb/` — session document schema
- `_changes` feed exposes live session updates

## Status
- [ ] Session document schema defined
- [ ] Proxy writes turns to session doc on each request
- [ ] _changes feed streams live updates
