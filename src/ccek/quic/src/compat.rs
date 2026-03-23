//! Compatibility layer for types that live in the main literbike crate.
//!
//! When compiled as a standalone crate (default), this module provides minimal
//! stubs so the code compiles. When compiled with `feature = "literbike-full"`,
//! these are expected to be supplied by the parent crate instead.
//!
//! TODO: wire these to real implementations when ccek-quic is integrated back.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// ============================================================================
// concurrency::ccek — CoroutineContext + ContextElement
// ============================================================================

/// Trait for context elements (mirrors crate::compat::ContextElement)
pub trait ContextElement: Send + Sync + 'static {
    fn key(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
}

/// Minimal CoroutineContext stub
#[derive(Clone)]
pub struct CoroutineContext {
    elements: Arc<RwLock<HashMap<&'static str, Arc<dyn ContextElement>>>>,
}

impl CoroutineContext {
    pub fn new() -> Self {
        Self {
            elements: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_element<E: ContextElement + Clone>(element: E) -> Self {
        let ctx = Self::new();
        ctx.install(element);
        ctx
    }

    pub fn get(&self, key: &str) -> Option<Arc<dyn ContextElement>> {
        self.elements.read().unwrap().get(key).cloned()
    }

    pub fn get_typed<T: ContextElement + Clone + 'static>(&self, key: &str) -> Option<T> {
        self.get(key)
            .and_then(|e| e.as_any().downcast_ref::<T>().cloned())
    }

    pub fn install<E: ContextElement + Clone>(&self, element: E) {
        self.elements
            .write()
            .unwrap()
            .insert(element.key(), Arc::new(element));
    }

    pub fn merge(&self, other: &Self) -> Self {
        let merged = self.clone();
        let other_elements = other.elements.read().unwrap();
        let mut guard = merged.elements.write().unwrap();
        for (k, v) in other_elements.iter() {
            guard.entry(k).or_insert_with(|| v.clone());
        }
        drop(guard);
        merged
    }

    pub fn keys(&self) -> Vec<&'static str> {
        self.elements.read().unwrap().keys().copied().collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.elements.read().unwrap().contains_key(key)
    }
}

impl Default for CoroutineContext {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Add<Arc<dyn ContextElement>> for CoroutineContext {
    type Output = Self;
    fn add(self, rhs: Arc<dyn ContextElement>) -> Self::Output {
        self.elements.write().unwrap().insert(rhs.key(), rhs);
        self
    }
}

/// Macro to implement ContextElement for a type (mirrors crate::impl_context_element)
#[macro_export]
macro_rules! impl_context_element {
    ($type:ty, $key:expr) => {
        impl $crate::compat::ContextElement for $type {
            fn key(&self) -> &'static str {
                $key
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

// ============================================================================
// rbcursive — NetTuple, RbCursor, Signal, etc.
// ============================================================================

use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    Tcp,
    Udp,
    Quic,
    Sctp,
    CustomQuic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetTuple {
    pub local_addr: [u8; 4],
    pub local_port: u16,
    pub remote_addr: [u8; 4],
    pub remote_port: u16,
    pub protocol: Protocol,
}

impl NetTuple {
    pub fn from_socket_addr(addr: SocketAddr, protocol: Protocol) -> Self {
        let (ip, port) = match addr {
            SocketAddr::V4(v4) => (v4.ip().octets(), v4.port()),
            SocketAddr::V6(_v6) => ([0u8; 4], 0u16), // stub: IPv4-only for now
        };
        Self {
            local_addr: [0; 4],
            local_port: 0,
            remote_addr: ip,
            remote_port: port,
            protocol,
        }
    }
}

/// Stub RbCursor for observational classification
pub struct RbCursor;

impl RbCursor {
    pub fn new() -> Self {
        Self
    }

    pub fn recognize(&self, _tuple: NetTuple, _hint: &[u8]) -> Signal {
        Signal::Accept(Protocol::Quic)
    }
}

impl Default for RbCursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Signal stub
#[derive(Debug, Clone)]
pub enum Signal {
    Accept(Protocol),
    Data(Vec<u8>),
    Close,
    Error(String),
}

/// Indexed trait stub
pub trait Indexed {
    fn index(&self) -> u64;
}

/// Join trait stub
pub trait Join {
    fn join(&self) -> Vec<u8>;
}

// ============================================================================
// wam_engine — WAMEngine and related types
// ============================================================================

pub mod wam_engine_compat {
    #[derive(Debug, Clone)]
    pub struct WAMEngine;
    #[derive(Debug, Clone)]
    pub struct WAMInstruction;
    #[derive(Debug, Clone)]
    pub struct WAMResult;
    #[derive(Debug, Clone)]
    pub struct Register(pub usize);
    #[derive(Debug, Clone)]
    pub struct Label(pub String);
    #[derive(Debug, Clone)]
    pub struct Functor(pub String, pub usize);
    #[derive(Debug, Clone)]
    pub struct Constant(pub String);
    #[derive(Debug, Clone)]
    pub struct Predicate(pub String);
    #[derive(Debug, Clone)]
    pub enum ProtocolType {
        Quic,
        Tcp,
        Udp,
    }

    impl WAMEngine {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for WAMEngine {
        fn default() -> Self {
            Self::new()
        }
    }
}

// ============================================================================
// cas_storage — ContentAddressedStore and related types
// ============================================================================

pub mod cas_storage_compat {
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    // Using blake3 for key hashing in retrieve_ref

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct ContentHash(pub Vec<u8>);

    impl From<[u8; 32]> for ContentHash {
        fn from(bytes: [u8; 32]) -> Self {
            ContentHash(bytes.to_vec())
        }
    }

    #[derive(Debug, Clone)]
    pub struct ContentBlob {
        pub data: Vec<u8>,
        pub hash: ContentHash,
    }

    impl ContentBlob {
        pub fn with_hash(data: Vec<u8>, hash: ContentHash) -> Self {
            Self { data, hash }
        }
    }

    #[derive(Debug, Clone)]
    pub struct MerkleNode {
        pub hash: ContentHash,
        pub children: Vec<ContentHash>,
    }

    impl MerkleNode {
        pub fn build_tree(hashes: &[ContentHash]) -> Option<MerkleNode> {
            if hashes.is_empty() {
                return None;
            }
            // Simple stub: single parent with all hashes as children
            let mut combined = Vec::new();
            for h in hashes {
                combined.extend_from_slice(&h.0);
            }
            let root_hash = blake3::hash(&combined);
            Some(MerkleNode {
                hash: ContentHash(root_hash.as_bytes().to_vec()),
                children: hashes.to_vec(),
            })
        }

        pub fn root(&self) -> [u8; 32] {
            let mut out = [0u8; 32];
            let h = &self.hash.0;
            let len = h.len().min(32);
            out[..len].copy_from_slice(&h[..len]);
            out
        }
    }

    pub struct ContentAddressedStore {
        store: Arc<RwLock<HashMap<ContentHash, ContentBlob>>>,
    }

    /// Statistics for the content-addressed store
    #[derive(Debug, Clone, Default)]
    pub struct StoreStats {
        pub total_blobs: u64,
        pub total_bytes: u64,
    }

    impl ContentAddressedStore {
        pub fn new() -> Self {
            Self {
                store: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        pub fn put(&self, blob: ContentBlob) -> ContentHash {
            let hash = blob.hash.clone();
            self.store.write().unwrap().insert(hash.clone(), blob);
            hash
        }

        pub fn store(&self, blob: &ContentBlob) -> Result<ContentHash, String> {
            let hash = blob.hash.clone();
            self.store
                .write()
                .map_err(|e| e.to_string())?
                .insert(hash.clone(), blob.clone());
            Ok(hash)
        }

        pub fn store_ref(
            &self,
            key: &str,
            _typ: &str,
            blob: &ContentBlob,
        ) -> Result<ContentHash, String> {
            let hash_bytes = blake3::hash(key.as_bytes()).as_bytes().to_vec();
            let hash = ContentHash(hash_bytes);
            self.store
                .write()
                .map_err(|e| e.to_string())?
                .insert(hash.clone(), blob.clone());
            Ok(hash)
        }

        pub fn store_merkle_root(&self, _root: &[u8; 32], _count: usize) -> Result<(), String> {
            Ok(())
        }

        pub fn get(&self, hash: &ContentHash) -> Option<ContentBlob> {
            self.store.read().unwrap().get(hash).cloned()
        }

        /// Retrieve a reference by string key (for packet recovery)
        pub fn retrieve_ref(&self, key: &str) -> Result<Option<ContentBlob>, String> {
            // Convert string key to ContentHash (using a simple hash for lookup)
            let hash_bytes = blake3::hash(key.as_bytes()).as_bytes().to_vec();
            let hash = ContentHash(hash_bytes);
            Ok(self.get(&hash))
        }

        /// Get store statistics
        pub fn stats(&self) -> Result<StoreStats, String> {
            let store = self.store.read().map_err(|e| e.to_string())?;
            let total_blobs = store.len() as u64;
            let total_bytes = store.values().map(|b| b.data.len() as u64).sum();
            Ok(StoreStats {
                total_blobs,
                total_bytes,
            })
        }
    }

    impl Default for ContentAddressedStore {
        fn default() -> Self {
            Self::new()
        }
    }
}

// ============================================================================
// liburing_facade — LibUringFacade stub
// ============================================================================

pub mod liburing_facade_compat {
    use std::io;

    pub struct LibUringFacade;

    impl LibUringFacade {
        pub fn new(_queue_depth: u32) -> io::Result<Self> {
            Ok(Self)
        }
    }
}

// ============================================================================
// uring_facade — UringFacade stub
// ============================================================================

pub mod uring_facade_compat {
    use std::io;

    #[derive(Debug, Clone, Copy)]
    pub enum OpCode {
        Read,
        Write,
        Accept,
        Connect,
        Close,
        Nop,
    }

    pub struct UringFacade;

    impl UringFacade {
        pub fn new(_queue_depth: u32) -> io::Result<Self> {
            Ok(Self)
        }
    }
}
