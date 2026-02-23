use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug, Default)]
pub struct SessionEntry {
    pub server_name: String,
    pub session_ticket: Vec<u8>,
    pub alpn: Option<Vec<u8>>,
}

pub trait QuicSessionCache: Send + Sync {
    fn put(&self, key: String, value: SessionEntry);
    fn get(&self, key: &str) -> Option<SessionEntry>;
}

#[derive(Default, Clone)]
pub struct DefaultQuicSessionCache(Arc<RwLock<HashMap<String, SessionEntry>>>);

impl QuicSessionCache for DefaultQuicSessionCache {
    fn put(&self, key: String, value: SessionEntry) {
        self.0.write().unwrap().insert(key, value);
    }
    fn get(&self, key: &str) -> Option<SessionEntry> {
        self.0.read().unwrap().get(key).cloned()
    }
}
