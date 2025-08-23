use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tiny_http::{Server, Response, Method, Request};
use std::thread;
use std::env;
use std::fs;
use std::path::PathBuf;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;

type Db = HashMap<String, serde_json::Value>;

pub fn start(listen: &str) {
    let addr = listen.to_string();
    let server = Server::http(&addr).expect("start emu server");

    // optional persistence path
    let store_path = env::var("COUCHDB_EMU_STORE").ok().map(PathBuf::from);

    // load existing store if present
    let mut initial: HashMap<String, Db> = HashMap::new();
    if let Some(ref p) = store_path {
        if let Ok(s) = fs::read_to_string(p) {
            if let Ok(v) = serde_json::from_str::<HashMap<String, Db>>(&s) {
                initial = v;
            }
        }
    }

    let store: Arc<Mutex<HashMap<String, Db>>> = Arc::new(Mutex::new(initial));

    // clone persist_path for the spawned thread
    let persist_for_thread = store_path.clone();
    thread::spawn(move || {
        for req in server.incoming_requests() {
            handle_request(req, &store, &persist_for_thread);
        }
    });
}

fn bump_rev(doc_obj: serde_json::Map<String, serde_json::Value>) -> String {
    // simple rev bump semantics: _rev = "<n>-manual"
    let cur = doc_obj.get("_rev").and_then(|v| v.as_str()).unwrap_or("0-0");
    let parts: Vec<&str> = cur.split('-').collect();
    let num = parts.get(0).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let next = num + 1;
    let new_rev = format!("{}-manual", next);
    new_rev
}

fn handle_request(mut req: Request, store: &Arc<Mutex<HashMap<String, Db>>>, persist_path: &Option<PathBuf>) {
    let method = req.method().clone();
    let url_full = req.url().to_string();

    // split path and query
    let (path, query) = match url_full.find('?') {
        Some(i) => (&url_full[..i], Some(&url_full[i+1..])),
        None => (&url_full[..], None),
    };

    // parse query into a map
    let mut qmap: HashMap<String, String> = HashMap::new();
    if let Some(qs) = query {
        for pair in qs.split('&') {
            if pair.is_empty() { continue; }
            let mut it = pair.splitn(2, '=');
            if let Some(k) = it.next() {
                let v = it.next().unwrap_or("");
                qmap.insert(k.to_string(), v.to_string());
            }
        }
    }

    // expected patterns: /<db>/<docid> or /<db>/<docid>/<filename>
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.len() < 2 {
        let _ = req.respond(Response::from_string("invalid").with_status_code(400));
        return;
    }
    let db = parts[0].to_string();
    let doc = parts[1].to_string();

    match (method, parts.len()) {
        (Method::Get, 2) => {
            let lock = store.lock().unwrap();
            if let Some(d) = lock.get(&db).and_then(|m| m.get(&doc)) {
                let body = d.to_string();
                let _ = req.respond(Response::from_string(body).with_status_code(200));
            } else {
                let _ = req.respond(Response::from_string("not found").with_status_code(404));
            }
        }
        (Method::Get, 3) => {
            // return raw attachment bytes or metadata if requested
            let filename = parts[2];
            let lock = store.lock().unwrap();
            if let Some(docv) = lock.get(&db).and_then(|m| m.get(&doc)) {
                if let Some(attachments) = docv.get("_attachments").and_then(|a| a.as_object()) {
                    if let Some(entry) = attachments.get(filename) {
                        // metadata request ?meta=1
                        if qmap.get("meta").map(|s| s == "1").unwrap_or(false) {
                            // return JSON metadata
                            let data_b64 = entry.get("data").and_then(|d| d.as_str()).unwrap_or("");
                            if let Ok(bytes) = B64.decode(data_b64) {
                                let meta = serde_json::json!({
                                    "filename": filename,
                                    "length": bytes.len(),
                                    "content_type": "application/octet-stream",
                                });
                                let _ = req.respond(Response::from_string(meta.to_string()).with_status_code(200));
                                return;
                            }
                        } else if let Some(entry_data) = entry.get("data").and_then(|d| d.as_str()) {
                            if let Ok(bytes) = B64.decode(entry_data) {
                                let resp = Response::from_data(bytes).with_status_code(200);
                                let _ = req.respond(resp);
                                return;
                            }
                        }
                    }
                }
            }
            let _ = req.respond(Response::from_string("not found").with_status_code(404));
        }
        (Method::Put, 2) => {
            // create or replace doc (empty or with JSON body)
            let mut buf = String::new();
            let _ = req.as_reader().read_to_string(&mut buf);
            let v = if buf.trim().is_empty() { serde_json::json!({}) } else { serde_json::from_str(&buf).unwrap_or(serde_json::json!({})) };
            let mut lock = store.lock().unwrap();
            let dbmap = lock.entry(db.clone()).or_insert_with(HashMap::new);
            // set _rev to 1 if new; if existing, bump
            let mut new_doc = v.as_object().cloned().unwrap_or_default();
            if let Some(existing) = dbmap.get(&doc) {
                if let Some(existing_obj) = existing.as_object() {
                    let mut merged = existing_obj.clone();
                    for (k, val) in &new_doc {
                        merged.insert(k.clone(), val.clone());
                    }
                    let new_rev = bump_rev(merged.clone());
                    merged.insert("_rev".to_string(), serde_json::Value::String(new_rev));
                    dbmap.insert(doc.clone(), serde_json::Value::Object(merged));
                } else {
                    new_doc.insert("_rev".to_string(), serde_json::Value::String("1-manual".to_string()));
                    dbmap.insert(doc.clone(), serde_json::Value::Object(new_doc));
                }
            } else {
                new_doc.insert("_rev".to_string(), serde_json::Value::String("1-manual".to_string()));
                dbmap.insert(doc.clone(), serde_json::Value::Object(new_doc));
            }

            // persist if requested
            if let Some(ref p) = persist_path {
                let snapshot = lock.clone();
                if let Ok(s) = serde_json::to_string_pretty(&snapshot) {
                    let _ = fs::write(p, s);
                }
            }

            let _ = req.respond(Response::from_string("ok").with_status_code(201));
        }
        (Method::Put, 3) => {
            // attachment upload: enforce rev if provided, store as field _attachments.filename = base64
            let filename = parts[2].to_string();
            let mut data = Vec::new();
            let _ = req.as_reader().read_to_end(&mut data);
            let b64 = B64.encode(&data);
            let mut lock = store.lock().unwrap();
            let dbmap = lock.entry(db.clone()).or_insert_with(HashMap::new);

            // rev check: if doc exists, require ?rev=<expected_rev>
            if let Some(existing_doc) = dbmap.get(&doc) {
                let existing_rev = existing_doc.get("_rev").and_then(|r| r.as_str()).unwrap_or("");
                match qmap.get("rev") {
                    Some(given) if given == existing_rev => { /* ok */ }
                    Some(_) => {
                        let _ = req.respond(Response::from_string("{\"error\":\"conflict\"}").with_status_code(409));
                        return;
                    }
                    None => {
                        // require rev to update existing doc
                        let _ = req.respond(Response::from_string("{\"error\":\"conflict_missing_rev\"}").with_status_code(409));
                        return;
                    }
                }
            }

            let doc_entry = dbmap.entry(doc.clone()).or_insert_with(|| serde_json::json!({}));
            // ensure _attachments map
            let attachments = doc_entry.get("_attachments").cloned().unwrap_or(serde_json::json!({}));
            let mut at_map = attachments.as_object().cloned().unwrap_or_default();
            at_map.insert(filename.clone(), serde_json::json!({"data": b64}));
            let mut new_doc = doc_entry.as_object().cloned().unwrap_or_default();
            new_doc.insert("_attachments".to_string(), serde_json::Value::Object(at_map));

            // bump rev
            let new_rev = bump_rev(new_doc.clone());
            new_doc.insert("_rev".to_string(), serde_json::Value::String(new_rev));

            *doc_entry = serde_json::Value::Object(new_doc);

            // persist if requested
            if let Some(ref p) = persist_path {
                let snapshot = lock.clone();
                if let Ok(s) = serde_json::to_string_pretty(&snapshot) {
                    let _ = fs::write(p, s);
                }
            }

            let _ = req.respond(Response::from_string("ok").with_status_code(201));
        }
        _ => {
            let _ = req.respond(Response::from_string("not implemented").with_status_code(501));
        }
    }
}
