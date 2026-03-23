// Minimal CouchDB 1.7.2-like emulator for local testing
// Features:
// - in-memory document store (kv)
// - attachments stored as base64 blobs
// - simple design doc 'views' via filter functions
// - simulated IPFS add/get (stores blobs by multihash-like key)
// - a tiny HTTP-like Swagger JSON stub for the API surface

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Document {
    pub id: String,
    pub rev: u64,
    pub content: Vec<u8>,
    pub attachments: HashMap<String, Vec<u8>>,
}

#[derive(Clone, Default)]
pub struct CouchDbEmulator {
    store: Arc<Mutex<HashMap<String, Document>>>,
    ipfs: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl CouchDbEmulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put_doc(&self, id: &str, content: &[u8]) {
        let mut s = self.store.lock().unwrap();
        let rev = s.get(id).map(|d| d.rev + 1).unwrap_or(1);
        s.insert(id.to_string(), Document { id: id.to_string(), rev, content: content.to_vec(), attachments: HashMap::new() });
    }

    pub fn get_doc(&self, id: &str) -> Option<Document> {
        let s = self.store.lock().unwrap();
        s.get(id).cloned()
    }

    pub fn put_attachment(&self, id: &str, name: &str, data: &[u8]) -> Result<(), String> {
        let mut s = self.store.lock().unwrap();
        if let Some(doc) = s.get_mut(id) {
            doc.attachments.insert(name.to_string(), data.to_vec());
            doc.rev += 1;
            Ok(())
        } else {
            Err("not_found".into())
        }
    }

    pub fn get_attachment(&self, id: &str, name: &str) -> Option<Vec<u8>> {
        let s = self.store.lock().unwrap();
        s.get(id).and_then(|d| d.attachments.get(name).cloned())
    }

    // very small view: return all docs where predicate returns true
    pub fn view_filter<F>(&self, f: F) -> Vec<Document>
    where
        F: Fn(&Document) -> bool,
    {
        let s = self.store.lock().unwrap();
        s.values().filter(|d| f(d)).cloned().collect()
    }

    // IPFS-like add: returns a fake multihash key (hex sha256)
    pub fn ipfs_add(&self, data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let res = hasher.finalize();
        let key = hex::encode(res);
        let mut ipfs = self.ipfs.lock().unwrap();
        ipfs.insert(key.clone(), data.to_vec());
        key
    }

    pub fn ipfs_get(&self, key: &str) -> Option<Vec<u8>> {
        let ipfs = self.ipfs.lock().unwrap();
        ipfs.get(key).cloned()
    }

    // minimal swagger stub describing the basic endpoints
    pub fn swagger_json() -> &'static str {
        r#"{"info":{"title":"CouchDB Emulator","version":"1.7.2-emulated"},"paths":{"/db/{doc}":{"put":{},"get":{}},"/db/{doc}/attachments/{name}":{"put":{},"get":{}},"/ipfs/add":{"post":{}},"/ipfs/get":{"get":{}}}}"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_doc_roundtrip() {
        let e = CouchDbEmulator::new();
        e.put_doc("d1", b"hello");
        let got = e.get_doc("d1").expect("doc");
        assert_eq!(got.content, b"hello".to_vec());
    }

    #[test]
    fn attachments_roundtrip() {
        let e = CouchDbEmulator::new();
        e.put_doc("d2", b"ok");
        assert!(e.put_attachment("d2", "a.txt", b"data").is_ok());
        let att = e.get_attachment("d2", "a.txt").expect("att");
        assert_eq!(att, b"data".to_vec());
    }

    #[test]
    fn view_filter_basic() {
        let e = CouchDbEmulator::new();
        e.put_doc("odd", b"1");
        e.put_doc("even", b"2");
        let found = e.view_filter(|d| d.content == b"2".to_vec());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "even");
    }

    #[test]
    fn ipfs_add_get() {
        let e = CouchDbEmulator::new();
        let key = e.ipfs_add(b"blob");
        let got = e.ipfs_get(&key).expect("ipfs");
        assert_eq!(got, b"blob".to_vec());
    }

    #[test]
    fn swagger_contains_version() {
        let s = CouchDbEmulator::swagger_json();
        assert!(s.contains("1.7.2-emulated"));
    }
}
