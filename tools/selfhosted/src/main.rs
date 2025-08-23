use std::env;
use std::path::PathBuf;
use std::process::Command;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tiny_http::{Server as TinyServer, Response};
use std::sync::{Arc, Mutex};
use serde_json::Value;
use notify::{RecursiveMode, Watcher, Event};
use tokio::sync::mpsc;
use std::fs::File;
use std::io::Write;
use walkdir::WalkDir;
use tar::Builder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use memmap2::MmapMut;
mod densifier;
mod couchdb_emulator;
mod config;

#[derive(Parser)]
#[command(author, version, about="Minimal self-hosted mirror: git + CouchDB attachments + remote git push", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Watch a directory and mirror changes
    Watch { path: PathBuf },
    /// Run an HTTP middle-tier server for status and manual triggers
    Serve { bind: Option<String>, watch_path: Option<PathBuf> },
    /// Run a tiny in-process CouchDB emulator (for tests/dev)
    Emu { bind: Option<String> },
    /// Mirror a git repo: create tar.gz, upload to CouchDB and write mmap index
    Mirror { path: PathBuf, doc_id: String },
    /// Upload a single file to CouchDB as attachment
    Upload { doc_id: String, file: PathBuf },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
    Commands::Watch { path } => watch_and_mirror(path, None).await?,
    Commands::Serve { bind, watch_path } => {
            // Shared in-memory app state
            let app_state: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));

            // Optional background watcher if watch_path given
            if let Some(p) = watch_path {
                let p_clone = p.clone();
                let s = Arc::clone(&app_state);
                tokio::spawn(async move { let _ = watch_and_mirror(p_clone, Some(s)).await; });
            }

            let cfg = config::Config::from_env();
            let listen = bind.or(cfg.http_bind).unwrap_or_else(|| "127.0.0.1:3000".to_string());
            println!("Starting HTTP middle-tier at http://{}", listen);

            let server = TinyServer::http(&listen).map_err(|e| anyhow::anyhow!("start tiny server: {}", e))?;
            for request in server.incoming_requests() {
                let url = request.url().to_string();
                match url.as_str() {
                    "/status" => {
                        let hdr = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
                        let resp = Response::from_string("{\"status\":\"ok\"}").with_header(hdr);
                        let _ = request.respond(resp);
                    }
                    "/sync" => {
                        // trigger a sync (best-effort)
                        let hdr = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
                        let _ = request.respond(Response::from_string("{\"result\":\"triggered\"}").with_header(hdr));
                    }
                    "/appstate" => {
                        let hdr = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
                        let current = {
                            let lock = app_state.lock().unwrap();
                            lock.clone()
                        };
                        let body = if let Some(v) = current { v.to_string() } else { "null".to_string() };
                        let _ = request.respond(Response::from_string(body).with_header(hdr));
                    }
                    _ => {
                        let _ = request.respond(Response::from_string("Not found").with_status_code(404));
                    }
                }
            }
        }
        Commands::Emu { bind } => {
            let listen = bind.unwrap_or_else(|| std::env::var("COUCHDB_EMU_ADDR").unwrap_or_else(|_| "127.0.0.1:15984".to_string()));
            println!("Starting CouchDB emulator at http://{}", listen);
            couchdb_emulator::start(&listen);
            // block the main thread to keep emulator running
            loop { std::thread::park(); }
        }
        Commands::Mirror { path, doc_id } => {
            // create tar.gz of repo
            let idx = create_repo_tar_and_index(&path).context("create tar/index")?;
            // upload tar as attachment
            let tar_path = path.join("repo.tar.gz");
            upload_attachment(&doc_id, &tar_path).await?;
            println!("Uploaded mirror for {} with index: {} entries", doc_id, idx);
        }
        Commands::Upload { doc_id, file } => upload_attachment(&doc_id, &file).await?,
    }

    Ok(())
}

async fn watch_and_mirror(path: PathBuf, app_state: Option<Arc<Mutex<Option<Value>>>>) -> Result<()> {
    println!("Watching: {}", path.display());

    // Channel for events from notify to async tokio
    let (tx, mut rx) = mpsc::channel(100);

    // Create blocking watcher using notify v6 API
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        match res {
            Ok(ev) => {
                // ignore send errors
                let _ = tx.blocking_send(ev);
            }
            Err(e) => eprintln!("watch error: {:?}", e),
        }
    }).context("create watcher")?;

    watcher.watch(&path, RecursiveMode::Recursive).context("watch path")?;

    while let Some(event) = rx.recv().await {
        println!("fs event: {:?}", event);

        // 1) local git commit
        if let Err(e) = git_commit_all(&path) {
            eprintln!("git commit failed: {:?}", e);
        }

        // 2) push to remote
        if let Err(e) = git_push(&path) {
            eprintln!("git push failed: {:?}", e);
        }

        // 3) upload changed paths to CouchDB (best-effort)
        if let Some(paths) = event.paths.get(0) {
            // upload the first changed path for demonstration
            if paths.is_file() {
                if let Err(e) = upload_attachment("literbike_repo", paths).await {
                    eprintln!("upload failed: {:?}", e);
                } else {
                    // update in-memory app state
                    if let Some(ref s) = app_state {
                        let mut lock = s.lock().unwrap();
                        *lock = Some(serde_json::json!({
                            "last_sync_epoch": chrono::Utc::now().timestamp(),
                            "file": paths.to_string_lossy(),
                        }));
                    }
                }
            }
        }
    }

    Ok(())
}

fn git_commit_all(repo_path: &PathBuf) -> Result<()> {
    // git add -A
    let status = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("add")
        .arg("-A")
        .status()
        .context("git add")?;

    if !status.success() {
        anyhow::bail!("git add failed");
    }

    // git commit -m "auto: sync"
    let status = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("commit")
        .arg("-m")
        .arg("auto: sync from literbike-selfhosted watcher")
        .status()
        .context("git commit")?;

    // commit may exit non-zero if there is nothing to commit
    if !status.success() {
        println!("git commit returned non-zero (maybe nothing to commit)");
    }

    Ok(())
}

fn git_push(repo_path: &PathBuf) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("push")
        .status()
        .context("git push")?;

    if !status.success() {
        anyhow::bail!("git push failed");
    }

    Ok(())
}

async fn upload_attachment(doc_id: &str, file: &PathBuf) -> Result<()> {
    let cfg = config::Config::from_env();
    let couch = cfg.couchdb_url.unwrap_or_else(|| env::var("COUCHDB_URL").unwrap_or_else(|_| "http://127.0.0.1:5984".to_string()));
    let db = cfg.couchdb_db.unwrap_or_else(|| env::var("COUCHDB_DB").unwrap_or_else(|_| "literbike".to_string()));

    // Ensure doc exists (create if missing)
    let client = reqwest::Client::new();
    let doc_url = format!("{}/{}/{}", couch.trim_end_matches('/'), db, doc_id);

    // Get current doc to learn _rev
    let mut rev: Option<String> = None;
    let get = client.get(&doc_url).send().await?;
    if get.status().is_success() {
        if let Ok(v) = get.json::<serde_json::Value>().await {
            rev = v.get("_rev").and_then(|r| r.as_str()).map(|s| s.to_string());
        }
    } else {
        // create basic doc
        let put = client.put(&doc_url).json(&serde_json::json!({})).send().await?;
        // attempt to read rev by fetching the doc again
        if put.status().is_success() {
            let get2 = client.get(&doc_url).send().await?;
            if get2.status().is_success() {
                if let Ok(v2) = get2.json::<serde_json::Value>().await {
                    rev = v2.get("_rev").and_then(|r| r.as_str()).map(|s| s.to_string());
                }
            }
        }
    }

    // Read file bytes
    let filename = file.file_name().and_then(|s| s.to_str()).context("file name")?;
    let data = tokio::fs::read(file).await.context("read file")?;

    // Build attachment upload URL: /db/docid/filename?rev=...
    let mut attach_url = format!("{}/{}/{}", couch.trim_end_matches('/'), db, doc_id);
    attach_url.push('/');
    attach_url.push_str(filename);
    if let Some(r) = rev {
        attach_url.push_str("?rev=");
        attach_url.push_str(&r);
    }

    let resp = client.put(&attach_url)
        .body(data)
        .header("Content-Type", "application/octet-stream")
        .send()
        .await?;

    if resp.status().is_success() {
        println!("uploaded {} to {}/{}", filename, db, doc_id);
    } else {
        eprintln!("attachment upload failed: {}", resp.status());
    }

    Ok(())
}

/// Create a repo.tar.gz from `path`, write an index.json as mmap file alongside it, return number of indexed entries.
fn create_repo_tar_and_index(path: &PathBuf) -> Result<usize> {
    // create tar in system temp dir to avoid including it in the tar walk
    let tmp_name = format!("repo-{}-{}.tar.zlib", std::process::id(), chrono::Utc::now().timestamp());
    let tmp_path = std::env::temp_dir().join(&tmp_name);
    let tar_gz = File::create(&tmp_path).context("create tmp tar.zlib file")?;
    let enc = ZlibEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);

    let mut index: Vec<String> = Vec::new();
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() {
            // Add file to tar with a relative path
            let rel = p.strip_prefix(path).unwrap().to_path_buf();
            tar.append_path_with_name(p, rel.clone()).context("append file")?;
            index.push(rel.to_string_lossy().to_string());
        }
    }

    // finish tar
    tar.finish().context("finish tar")?;

    // copy tar into repo directory with stable name
    let tar_gz_path = path.join("repo.tar.zlib");
    std::fs::copy(&tmp_path, &tar_gz_path).context("copy tar to repo")?;

    // write index.json next to tar
    let idx_path = path.join("repo.index.json");
    let idx_json = serde_json::to_vec(&index)?;
    let mut f = File::create(&idx_path).context("create index file")?;
    f.set_len(idx_json.len() as u64).context("set idx len")?;
    f.write_all(&idx_json).context("write idx")?;

    // mmap the index file to validate mapping
    let file = File::options().read(true).write(true).open(&idx_path).context("open idx for mmap")?;
    let mmap = unsafe { MmapMut::map_mut(&file).context("mmap idx")? };
    // for demonstration we simply leave the bytes as-is (already written)
    mmap.flush().context("flush mmap")?;

    Ok(index.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::write;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_create_repo_tar_and_index() {
        let dir = tempdir().unwrap();
        let p = dir.path().to_path_buf();
        // create sample files
        write(p.join("a.txt"), b"hello").unwrap();
        write(p.join("sub/b.txt"), b"world").unwrap_or_else(|_| {
            std::fs::create_dir_all(p.join("sub")).unwrap();
            write(p.join("sub/b.txt"), b"world").unwrap();
        });

        let cnt = create_repo_tar_and_index(&p).expect("create tar/index");
        assert_eq!(cnt, 2);
        // ensure files exist
        assert!(p.join("repo.tar.zlib").exists());
        assert!(p.join("repo.index.json").exists());

        // New user-story driven assertion: index file should be valid JSON array with relative paths
        let idx_raw = std::fs::read_to_string(p.join("repo.index.json")).expect("read index");
        let parsed: serde_json::Value = serde_json::from_str(&idx_raw).expect("index is valid json");
        assert!(parsed.is_array());
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        // ensure entries are relative paths (no leading '/')
        for v in arr {
            if let Some(s) = v.as_str() {
                assert!(!s.starts_with('/'));
            } else { panic!("index entries must be strings"); }
        }
    }

    #[tokio::test]
    async fn emu_flow_upload() {
        // start emulator
        let bind = "127.0.0.1:15984";
        crate::couchdb_emulator::start(bind);
        // allow server to start
        thread::sleep(Duration::from_millis(200));

        std::env::set_var("COUCHDB_URL", format!("http://{}", bind));
        std::env::set_var("COUCHDB_DB", "testdb");

        let dir = tempdir().unwrap();
        let p = dir.path().to_path_buf();
        write(p.join("x.txt"), b"hi").unwrap();

        upload_attachment("doc-emu", &p.join("x.txt")).await.expect("upload");

        // fetch back
        let client = reqwest::Client::new();
        let url = format!("http://{}/testdb/doc-emu", bind);
        let resp = client.get(&url).send().await.expect("get");
        assert!(resp.status().is_success());
        let v: serde_json::Value = resp.json().await.expect("json");
        assert!(v.get("_attachments").is_some());
    }

    #[tokio::test]
    async fn emu_rev_conflict_red_to_green() {
        let bind = "127.0.0.1:15985";
        crate::couchdb_emulator::start(bind);
        thread::sleep(Duration::from_millis(200));

        let client = reqwest::Client::new();
        // create base doc
        let url = format!("http://{}/testdb/doc-conf", bind);
        let _ = client.put(&url).body("{}").send().await.expect("create");

        // upload attachment without rev -> expect 409
        let attach_url = format!("http://{}/testdb/doc-conf/file.txt", bind);
        let resp = client.put(&attach_url).body("hello").send().await.expect("put");
        assert_eq!(resp.status().as_u16(), 409);

        // fetch doc to get rev
        let get = client.get(&url).send().await.expect("get");
        let v: serde_json::Value = get.json().await.expect("json");
        let rev = v.get("_rev").and_then(|r| r.as_str()).unwrap_or("");

        // now upload with rev -> should succeed
        let attach_url_rev = format!("http://{}/testdb/doc-conf/file.txt?rev={}", bind, rev);
        let resp2 = client.put(&attach_url_rev).body("hello").send().await.expect("put2");
        assert!(resp2.status().is_success());
    }
}
