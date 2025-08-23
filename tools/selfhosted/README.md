literbike-selfhosted

A tiny helper tool to demonstrate a self-hosted mirroring flow: local Git repo + CouchDB attachments + remote Git push.

Usage

- Set environment variables as needed:
  - COUCHDB_URL (default: http://127.0.0.1:5984)
  - COUCHDB_DB (default: literbike)

- Initialize a git repo in the directory you want to watch and add a remote:

```bash
cd /path/to/project
git init
git remote add origin <git-remote-url>
```

- Run the watcher:

```bash
cargo run --manifest-path tools/selfhosted/Cargo.toml -- watch /path/to/project
```

- Or upload a single file as a CouchDB attachment:

```bash
cargo run --manifest-path tools/selfhosted/Cargo.toml -- upload literbike_repo file.txt
```

Notes

- This is intentionally minimal: it demonstrates the three-way mirroring and file-watching pattern.
- CouchDB must be reachable and the database created or the tool will attempt to PUT the document.
- For production-grade behavior add retries, robust conflict handling, and secure auth (Basic/Token) for CouchDB.
