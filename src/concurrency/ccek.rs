//! CCEK - CoroutineContext Element Key Bundling
//! 
//! Based on Kotlin's CoroutineContext pattern from Betanet:
//! - Services implement ContextElement trait (like CoroutineContext.Element)
//! - Each service has a unique Key for lookup
//! - Contexts are composed using + operator (implemented as Add trait)
//! - Services are retrieved via context[Key] syntax
//!
//! Core pattern from BetanetIntegrationDemo.kt:
//! ```kotlin
//! return EmptyCoroutineContext +
//!     dhtService +
//!     protocolDetector +
//!     crdtStorage +
//!     crdtNetwork +
//!     conflictResolver
//! ```
//!
//! Rust equivalent:
//! ```rust
//! let ctx = EmptyContext
//!     + dht_service
//!     + protocol_detector
//!     + crdt_storage
//!     + crdt_network
//!     + conflict_resolver;
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::Add;
use std::sync::Arc;
use parking_lot::RwLock;

/// Unique key for identifying context elements (like CoroutineContext.Key)
pub trait ContextKey: Send + Sync + 'static {
    /// Returns the type identifier for this key
    fn type_id(&self) -> TypeId;
    
    /// Returns a unique string identifier
    fn name(&self) -> &'static str;
}

/// A context element that can be stored in CoroutineContext
/// Equivalent to Kotlin's CoroutineContext.Element interface
pub trait ContextElement: Send + Sync + 'static {
    /// The key for this element (like companion object Key in Kotlin)
    fn key(&self) -> &'static str;
    
    /// Get the type ID for runtime type checking
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
    
    /// Downcast to Any for type-erased storage
    fn as_any(&self) -> &dyn Any;
}

/// Empty context (like EmptyCoroutineContext in Kotlin)
#[derive(Clone, Debug, Default)]
pub struct EmptyContext;

/// CoroutineContext - composite context holding multiple elements
/// Equivalent to Kotlin's CoroutineContext interface
#[derive(Clone, Default)]
pub struct CoroutineContext {
    elements: Arc<RwLock<HashMap<&'static str, Arc<dyn ContextElement>>>>,
}

impl CoroutineContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            elements: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Create context with a single element
    pub fn with_element<E: ContextElement + Clone>(element: E) -> Self {
        let mut ctx = Self::new();
        ctx = ctx + Arc::new(element);
        ctx
    }
    
    /// Get an element by key (like context[Key] in Kotlin)
    pub fn get(&self, key: &str) -> Option<Arc<dyn ContextElement>> {
        let elements = self.elements.read();
        elements.get(key).cloned()
    }
    
    /// Get an element by key and downcast to specific type
    pub fn get_typed<E: ContextElement + Clone>(&self, key: &str) -> Option<Arc<E>> {
        let element = self.get(key)?;
        element.as_any().downcast_ref::<E>().map(|e| Arc::new(E::clone(e)))
    }
    
    /// Check if context contains an element with the given key
    pub fn contains(&self, key: &str) -> bool {
        let elements = self.elements.read();
        elements.contains_key(key)
    }
    
    /// Get all element keys
    pub fn keys(&self) -> Vec<&'static str> {
        let elements = self.elements.read();
        elements.keys().copied().collect()
    }
    
    /// Get the number of elements
    pub fn len(&self) -> usize {
        let elements = self.elements.read();
        elements.len()
    }
    
    /// Check if context is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Merge another context into this one
    pub fn merge(&self, other: &CoroutineContext) -> Self {
        let mut new_elements = self.elements.read().clone();
        let other_elements = other.elements.read();
        
        for (key, value) in other_elements.iter() {
            new_elements.insert(*key, value.clone());
        }
        
        Self {
            elements: Arc::new(RwLock::new(new_elements)),
        }
    }
}

impl std::fmt::Debug for CoroutineContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let keys = self.keys();
        f.debug_struct("CoroutineContext")
            .field("elements", &keys)
            .finish()
    }
}

/// Add trait for combining context with element (implements Kotlin's + operator)
impl Add<Arc<dyn ContextElement>> for CoroutineContext {
    type Output = Self;
    
    fn add(self, element: Arc<dyn ContextElement>) -> Self::Output {
        {
            let mut elements = self.elements.write();
            elements.insert(element.key(), element);
        }
        self
    }
}

/// Add trait for combining two contexts
impl Add<CoroutineContext> for CoroutineContext {
    type Output = Self;
    
    fn add(self, other: CoroutineContext) -> Self::Output {
        self.merge(&other)
    }
}

impl<E: ContextElement + Clone> Add<Arc<E>> for CoroutineContext {
    type Output = Self;
    
    fn add(self, element: Arc<E>) -> Self::Output {
        let ctx_element: Arc<dyn ContextElement> = element;
        {
            let mut elements = self.elements.write();
            elements.insert(ctx_element.key(), ctx_element);
        }
        self
    }
}

/// Add trait for EmptyContext + Element
impl Add<Arc<dyn ContextElement>> for EmptyContext {
    type Output = CoroutineContext;
    
    fn add(self, element: Arc<dyn ContextElement>) -> Self::Output {
        let ctx = CoroutineContext::new();
        {
            let mut elements = ctx.elements.write();
            elements.insert(element.key(), element);
        }
        ctx
    }
}

impl<E: ContextElement + Clone> Add<Arc<E>> for EmptyContext {
    type Output = CoroutineContext;
    
    fn add(self, element: Arc<E>) -> Self::Output {
        let ctx_element: Arc<dyn ContextElement> = element;
        let ctx = CoroutineContext::new();
        {
            let mut elements = ctx.elements.write();
            elements.insert(ctx_element.key(), ctx_element);
        }
        ctx
    }
}

/// Add is_empty method for EmptyContext
impl EmptyContext {
    pub fn is_empty(&self) -> bool {
        true
    }
    
    /// Create a channel with this context
    pub fn create_channel<T: Send + 'static>(&self, buffer: usize) 
        -> (async_channel::Sender<T>, async_channel::Receiver<T>) {
        async_channel::bounded(buffer)
    }
}

/// Macro for implementing ContextElement trait
#[macro_export]
macro_rules! impl_context_element {
    ($type:ty, $key:expr) => {
        impl $crate::concurrency::ccek::ContextElement for $type {
            fn key(&self) -> &'static str {
                $key
            }
            
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

/// Example service: Protocol Detector (from BetanetReactorCore.kt)
#[derive(Clone, Debug)]
pub struct ProtocolDetector {
    pub name: &'static str,
}

impl_context_element!(ProtocolDetector, "ProtocolDetector");

impl ProtocolDetector {
    pub fn new() -> Self {
        Self { name: "DefaultProtocolDetector" }
    }
    
    pub fn detect_protocol(&self, data: &[u8]) -> DetectionResult {
        if data.is_empty() {
            return DetectionResult::Unknown;
        }
        
        // Simple protocol detection based on first bytes
        match data[0] {
            0x16 => DetectionResult::TLS(TLSVersion::TLS13),
            b'G' if data.starts_with(b"GET") => DetectionResult::HTTP(HTTPVersion::HTTP11),
            b'P' if data.starts_with(b"POST") => DetectionResult::HTTP(HTTPVersion::HTTP11),
            _ => DetectionResult::Unknown,
        }
    }
}

impl Default for ProtocolDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detection results (from BetanetReactorCore.kt)
#[derive(Debug, Clone)]
pub enum DetectionResult {
    Unknown,
    HTTP(HTTPVersion),
    QUIC(QUICVersion),
    TLS(TLSVersion),
}

#[derive(Debug, Clone)]
pub enum HTTPVersion {
    HTTP10,
    HTTP11,
    HTTP2,
    HTTP3,
}

#[derive(Debug, Clone)]
pub enum QUICVersion {
    QUICv1,
    QUICv2,
}

#[derive(Debug, Clone)]
pub enum TLSVersion {
    TLS12,
    TLS13,
}

/// Example service: DHT Service (from BetanetIPFSCore.kt)
#[derive(Clone, Debug, Default)]
pub struct DHTService {
    pub node_id: String,
}

impl_context_element!(DHTService, "DHTService");

impl DHTService {
    pub fn new(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
        }
    }
}

/// Example service: CRDT Storage (from BetanetCRDTCore.kt)
#[derive(Clone, Debug, Default)]
pub struct CRDTStorage {
    pub storage_path: String,
}

impl_context_element!(CRDTStorage, "CRDTStorage");

impl CRDTStorage {
    pub fn new(path: &str) -> Self {
        Self {
            storage_path: path.to_string(),
        }
    }
}

/// Example service: CRDT Network
#[derive(Clone, Debug, Default)]
pub struct CRDTNetwork {
    pub peer_id: String,
}

impl_context_element!(CRDTNetwork, "CRDTNetwork");

impl CRDTNetwork {
    pub fn new(peer_id: &str) -> Self {
        Self {
            peer_id: peer_id.to_string(),
        }
    }
}

/// Example service: Conflict Resolver
#[derive(Clone, Debug, Default)]
pub struct ConflictResolver {
    pub strategy: ConflictStrategy,
}

#[derive(Clone, Debug, Default)]
pub enum ConflictStrategy {
    #[default]
    LastWriteWins,
    OperationalTransformation,
    CRDTMerge,
}

impl_context_element!(ConflictResolver, "ConflictResolver");

impl ConflictResolver {
    pub fn new(strategy: ConflictStrategy) -> Self {
        Self { strategy }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_context() {
        let ctx = EmptyContext;
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_context_with_element() {
        let detector = Arc::new(ProtocolDetector::new());
        let ctx = EmptyContext + detector;
        
        assert_eq!(ctx.len(), 1);
        assert!(ctx.contains("ProtocolDetector"));
    }

    #[test]
    fn test_context_composition() {
        // Replicate the Kotlin pattern from BetanetIntegrationDemo.kt
        let ctx = EmptyContext
            + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>
            + Arc::new(DHTService::new("node-1"))
            + Arc::new(CRDTStorage::new("/tmp/crdt"))
            + Arc::new(CRDTNetwork::new("peer-1"))
            + Arc::new(ConflictResolver::default());
        
        assert_eq!(ctx.len(), 5);
        assert!(ctx.contains("ProtocolDetector"));
        assert!(ctx.contains("DHTService"));
        assert!(ctx.contains("CRDTStorage"));
        assert!(ctx.contains("CRDTNetwork"));
        assert!(ctx.contains("ConflictResolver"));
    }

    #[test]
    fn test_context_get_typed() {
        let ctx = EmptyContext
            + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>
            + Arc::new(DHTService::new("node-1"));
        
        let detector = ctx.get_typed::<ProtocolDetector>("ProtocolDetector");
        assert!(detector.is_some());
        assert_eq!(detector.unwrap().name, "DefaultProtocolDetector");
        
        let dht = ctx.get_typed::<DHTService>("DHTService");
        assert!(dht.is_some());
        assert_eq!(dht.unwrap().node_id, "node-1");
    }

    #[test]
    fn test_context_merge() {
        let ctx1 = EmptyContext
            + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>;
        
        let ctx2 = EmptyContext
            + Arc::new(DHTService::new("node-1"));
        
        let merged = ctx1.merge(&ctx2);
        assert_eq!(merged.len(), 2);
        assert!(merged.contains("ProtocolDetector"));
        assert!(merged.contains("DHTService"));
    }

    #[test]
    fn test_protocol_detection() {
        let detector = ProtocolDetector::new();
        
        // Test HTTP detection
        let http_get = b"GET /api/v1/status HTTP/1.1\r\n";
        let result = detector.detect_protocol(http_get);
        assert!(matches!(result, DetectionResult::HTTP(_)));
        
        // Test TLS detection
        let tls_handshake = [0x16, 0x03, 0x01];
        let result = detector.detect_protocol(&tls_handshake);
        assert!(matches!(result, DetectionResult::TLS(_)));
        
        // Test unknown
        let unknown = b"random data";
        let result = detector.detect_protocol(unknown);
        assert!(matches!(result, DetectionResult::Unknown));
    }

    #[test]
    fn test_context_keys() {
        let ctx = EmptyContext
            + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>
            + Arc::new(DHTService::new("node-1"));
        
        let keys = ctx.keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"ProtocolDetector"));
        assert!(keys.contains(&"DHTService"));
    }
}
