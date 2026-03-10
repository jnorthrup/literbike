use crate::impl_context_element;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct SessionEntry {
    pub server_name: String,
    pub session_ticket: Vec<u8>,
    pub alpn: Option<Vec<u8>>,
    pub zero_rtt_params: Option<Vec<u8>>,
    pub inserted_at: Instant,
    pub ttl: Duration,
}

impl SessionEntry {
    pub fn new(server_name: String, session_ticket: Vec<u8>) -> Self {
        Self {
            server_name,
            session_ticket,
            alpn: None,
            zero_rtt_params: None,
            inserted_at: Instant::now(),
            ttl: Duration::from_secs(3600),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

pub trait QuicSessionCache: Send + Sync {
    fn put(&self, key: String, value: SessionEntry);
    fn get(&self, key: &str) -> Option<SessionEntry>;
}

#[derive(Default, Clone)]
pub struct DefaultQuicSessionCache(Arc<RwLock<HashMap<String, SessionEntry>>>);

impl DefaultQuicSessionCache {
    pub fn evict_expired(&self) {
        self.0.write().unwrap().retain(|_, v| !v.is_expired());
    }
}

impl QuicSessionCache for DefaultQuicSessionCache {
    fn put(&self, key: String, value: SessionEntry) {
        self.0.write().unwrap().insert(key, value);
    }

    fn get(&self, key: &str) -> Option<SessionEntry> {
        let map = self.0.read().unwrap();
        map.get(key).filter(|e| !e.is_expired()).cloned()
    }
}

/// CCek-injectable session cache service, following the TlsCcekService pattern.
#[derive(Clone)]
pub struct SessionCacheService {
    pub cache: Arc<dyn QuicSessionCache>,
}

impl_context_element!(SessionCacheService, "SessionCacheService");

impl SessionCacheService {
    pub fn new(cache: Arc<dyn QuicSessionCache>) -> Self {
        Self { cache }
    }

    pub fn with_default() -> Self {
        Self {
            cache: Arc::new(DefaultQuicSessionCache::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_with_ttl(ttl: Duration) -> SessionEntry {
        SessionEntry {
            server_name: "test.example".into(),
            session_ticket: vec![1, 2, 3],
            alpn: Some(b"h3".to_vec()),
            zero_rtt_params: Some(vec![0xde, 0xad]),
            inserted_at: Instant::now(),
            ttl,
        }
    }

    #[test]
    fn test_session_entry_ttl_expiry() {
        let entry = SessionEntry {
            server_name: "example".into(),
            session_ticket: vec![],
            alpn: None,
            zero_rtt_params: None,
            inserted_at: Instant::now() - Duration::from_secs(10),
            ttl: Duration::from_secs(5),
        };
        assert!(entry.is_expired(), "entry with elapsed > ttl must be expired");

        let fresh = entry_with_ttl(Duration::from_secs(3600));
        assert!(!fresh.is_expired(), "fresh entry must not be expired");
    }

    #[test]
    fn test_lazy_eviction_on_get() {
        let cache = DefaultQuicSessionCache::default();

        // Insert already-expired entry
        let stale = SessionEntry {
            server_name: "stale".into(),
            session_ticket: vec![9],
            alpn: None,
            zero_rtt_params: None,
            inserted_at: Instant::now() - Duration::from_secs(10),
            ttl: Duration::from_secs(1),
        };
        cache.put("stale-key".into(), stale);

        // get() must return None for expired entry
        assert!(
            cache.get("stale-key").is_none(),
            "expired entry must be filtered on get"
        );
    }

    #[test]
    fn test_evict_expired_bulk() {
        let cache = DefaultQuicSessionCache::default();

        let stale = SessionEntry {
            server_name: "s".into(),
            session_ticket: vec![],
            alpn: None,
            zero_rtt_params: None,
            inserted_at: Instant::now() - Duration::from_secs(10),
            ttl: Duration::from_secs(1),
        };
        cache.put("stale1".into(), stale.clone());
        cache.put("stale2".into(), stale);

        let live = entry_with_ttl(Duration::from_secs(3600));
        cache.put("live".into(), live);

        assert_eq!(cache.0.read().unwrap().len(), 3);

        cache.evict_expired();

        let map = cache.0.read().unwrap();
        assert_eq!(map.len(), 1, "only live entry should remain after evict_expired");
        assert!(map.contains_key("live"));
    }

    #[test]
    fn test_zero_rtt_params_roundtrip() {
        let cache = DefaultQuicSessionCache::default();
        let params = vec![0x01, 0x02, 0x03, 0x04];
        let entry = SessionEntry {
            server_name: "zrtt".into(),
            session_ticket: vec![0xaa],
            alpn: Some(b"h3".to_vec()),
            zero_rtt_params: Some(params.clone()),
            inserted_at: Instant::now(),
            ttl: Duration::from_secs(3600),
        };
        cache.put("zrtt-key".into(), entry);

        let retrieved = cache.get("zrtt-key").expect("must retrieve stored entry");
        assert_eq!(retrieved.zero_rtt_params.as_deref(), Some(params.as_slice()));
        assert_eq!(retrieved.alpn.as_deref(), Some(b"h3".as_ref()));
        assert_eq!(retrieved.session_ticket, vec![0xaa]);
    }
}
