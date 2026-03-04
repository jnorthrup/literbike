//! Lazy N-way CAS projection gateway.
//!
//! The gateway stores canonical content once and materializes backend-specific
//! projections only when explicitly requested.

use crate::cas_storage::{ContentBlob, ContentHash, ContentAddressedStore};
use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Backends supported by the lazy projection gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectionBackend {
    Git,
    Torrent,
    Ipfs,
    S3Blobs,
    Kv,
}

impl ProjectionBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectionBackend::Git => "git",
            ProjectionBackend::Torrent => "torrent",
            ProjectionBackend::Ipfs => "ipfs",
            ProjectionBackend::S3Blobs => "s3-blobs",
            ProjectionBackend::Kv => "kv",
        }
    }
}

/// Canonical CAS metadata envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasEnvelope {
    pub algorithm: &'static str,
    pub size: u64,
    pub media_type: String,
}

/// Returned after storing canonical bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PutResult {
    pub hash: ContentHash,
    pub envelope: CasEnvelope,
}

/// Projection result for a backend materialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRecord {
    pub backend: ProjectionBackend,
    pub hash: ContentHash,
    pub locator: String,
}

/// Adapter contract for individual backends.
pub trait ProjectionAdapter: Send + Sync {
    fn backend(&self) -> ProjectionBackend;
    fn deterministic_locator(&self, hash: &ContentHash) -> String;
    fn project(&self, hash: &ContentHash, bytes: &[u8]) -> Result<String>;
    fn fetch(&self, locator: &str) -> Result<Option<Vec<u8>>>;
}

/// In-memory adapter used for deterministic tests and local simulation.
pub struct InMemoryProjectionAdapter {
    backend: ProjectionBackend,
    namespace: &'static str,
    objects: RwLock<HashMap<String, Vec<u8>>>,
    writes: AtomicUsize,
}

impl InMemoryProjectionAdapter {
    pub fn new(backend: ProjectionBackend, namespace: &'static str) -> Self {
        Self {
            backend,
            namespace,
            objects: RwLock::new(HashMap::new()),
            writes: AtomicUsize::new(0),
        }
    }

    pub fn write_count(&self) -> usize {
        self.writes.load(Ordering::Relaxed)
    }
}

impl ProjectionAdapter for InMemoryProjectionAdapter {
    fn backend(&self) -> ProjectionBackend {
        self.backend
    }

    fn deterministic_locator(&self, hash: &ContentHash) -> String {
        format!("{}/{}", self.namespace, hex::encode(hash))
    }

    fn project(&self, hash: &ContentHash, bytes: &[u8]) -> Result<String> {
        let locator = self.deterministic_locator(hash);
        let mut objects = self.objects.write();
        if !objects.contains_key(&locator) {
            objects.insert(locator.clone(), bytes.to_vec());
            self.writes.fetch_add(1, Ordering::Relaxed);
        }
        Ok(locator)
    }

    fn fetch(&self, locator: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.objects.read().get(locator).cloned())
    }
}

/// Canonical CAS + lazy backend projection gateway.
pub struct LazyProjectionGateway {
    canonical: ContentAddressedStore,
    envelopes: RwLock<HashMap<ContentHash, CasEnvelope>>,
    adapters: RwLock<HashMap<ProjectionBackend, Arc<dyn ProjectionAdapter>>>,
    projection_index: RwLock<HashMap<(ContentHash, ProjectionBackend), String>>,
}

impl Default for LazyProjectionGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl LazyProjectionGateway {
    pub fn new() -> Self {
        Self {
            canonical: ContentAddressedStore::new(),
            envelopes: RwLock::new(HashMap::new()),
            adapters: RwLock::new(HashMap::new()),
            projection_index: RwLock::new(HashMap::new()),
        }
    }

    /// Register or replace a backend adapter.
    pub fn register_adapter(&self, adapter: Arc<dyn ProjectionAdapter>) {
        self.adapters.write().insert(adapter.backend(), adapter);
    }

    /// Store bytes once in canonical CAS.
    pub fn put(&self, bytes: Vec<u8>, media_type: impl Into<String>) -> Result<PutResult> {
        let blob = ContentBlob::new(bytes);
        self.canonical.store(&blob)?;

        let envelope = CasEnvelope {
            algorithm: "sha2-256",
            size: blob.size as u64,
            media_type: media_type.into(),
        };
        self.envelopes.write().insert(blob.hash, envelope.clone());

        Ok(PutResult {
            hash: blob.hash,
            envelope,
        })
    }

    /// Lazily project canonical bytes into a selected backend.
    pub fn project(&self, hash: &ContentHash, backend: ProjectionBackend) -> Result<ProjectionRecord> {
        if let Some(existing) = self.projection_index.read().get(&(*hash, backend)).cloned() {
            return Ok(ProjectionRecord {
                backend,
                hash: *hash,
                locator: existing,
            });
        }

        let blob = self
            .canonical
            .retrieve(hash)?
            .ok_or_else(|| anyhow!("canonical object not found for hash {}", hex::encode(hash)))?;

        let adapter = self
            .adapters
            .read()
            .get(&backend)
            .cloned()
            .ok_or_else(|| anyhow!("projection adapter not registered: {}", backend.as_str()))?;

        let locator = adapter.project(hash, &blob.data)?;
        self.projection_index
            .write()
            .insert((*hash, backend), locator.clone());

        Ok(ProjectionRecord {
            backend,
            hash: *hash,
            locator,
        })
    }

    /// Resolve bytes from canonical storage first, then fall back to projected backends.
    pub fn get(&self, hash: &ContentHash, fallback_order: &[ProjectionBackend]) -> Result<Option<Vec<u8>>> {
        if let Some(blob) = self.canonical.retrieve(hash)? {
            return Ok(Some(blob.data));
        }

        for backend in fallback_order {
            let locator = self
                .projection_index
                .read()
                .get(&(*hash, *backend))
                .cloned();
            let Some(locator) = locator else {
                continue;
            };

            let adapter = self.adapters.read().get(backend).cloned();
            let Some(adapter) = adapter else {
                continue;
            };

            let bytes = adapter.fetch(&locator)?;
            let Some(bytes) = bytes else {
                continue;
            };

            if digest(&bytes) == *hash {
                return Ok(Some(bytes));
            }
        }

        Ok(None)
    }

    pub fn envelope(&self, hash: &ContentHash) -> Option<CasEnvelope> {
        self.envelopes.read().get(hash).cloned()
    }
}

fn digest(bytes: &[u8]) -> ContentHash {
    let hash = Sha256::digest(bytes);
    hash.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register_all(gw: &LazyProjectionGateway) -> HashMap<ProjectionBackend, Arc<InMemoryProjectionAdapter>> {
        let mut adapters = HashMap::new();
        let all = [
            (ProjectionBackend::Git, "git"),
            (ProjectionBackend::Torrent, "torrent"),
            (ProjectionBackend::Ipfs, "ipfs"),
            (ProjectionBackend::S3Blobs, "s3"),
            (ProjectionBackend::Kv, "kv"),
        ];

        for (backend, ns) in all {
            let adapter = Arc::new(InMemoryProjectionAdapter::new(backend, ns));
            gw.register_adapter(adapter.clone());
            adapters.insert(backend, adapter);
        }

        adapters
    }

    #[test]
    fn put_is_lazy_and_does_not_materialize_backends() {
        let gw = LazyProjectionGateway::new();
        let adapters = register_all(&gw);

        let put = gw.put(b"lazy".to_vec(), "text/plain").expect("put");
        assert_eq!(put.envelope.algorithm, "sha2-256");
        assert_eq!(put.envelope.size, 4);
        assert_eq!(put.envelope.media_type, "text/plain");

        for adapter in adapters.values() {
            assert_eq!(adapter.write_count(), 0);
        }
    }

    #[test]
    fn projection_is_deterministic_and_idempotent() {
        let gw = LazyProjectionGateway::new();
        let adapters = register_all(&gw);

        let put = gw.put(b"same bytes".to_vec(), "application/octet-stream").expect("put");
        let first = gw.project(&put.hash, ProjectionBackend::Git).expect("project once");
        let second = gw.project(&put.hash, ProjectionBackend::Git).expect("project twice");

        assert_eq!(first.locator, second.locator);
        assert_eq!(adapters[&ProjectionBackend::Git].write_count(), 1);
        assert_eq!(adapters[&ProjectionBackend::Torrent].write_count(), 0);
    }

    #[test]
    fn get_round_trips_after_projection() {
        let gw = LazyProjectionGateway::new();
        let _adapters = register_all(&gw);

        let bytes = b"projection bytes".to_vec();
        let put = gw.put(bytes.clone(), "application/octet-stream").expect("put");
        let record = gw
            .project(&put.hash, ProjectionBackend::Ipfs)
            .expect("project ipfs");

        assert!(record.locator.contains("ipfs/"));
        let fetched = gw
            .get(&put.hash, &[ProjectionBackend::Ipfs, ProjectionBackend::Git])
            .expect("get");
        assert_eq!(fetched, Some(bytes));
    }
}
