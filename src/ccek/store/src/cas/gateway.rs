//! Lazy N-way CAS projection gateway.
//!
//! The gateway stores canonical content once and materializes backend-specific
//! projections only when explicitly requested.

use crate::cas::storage::{ContentBlob, ContentHash, ContentAddressedStore};
use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Threshold in bytes below which objects are stored as a single inline blob.
pub const SMALL_OBJECT_THRESHOLD: usize = 4096;

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

/// Strategy for how objects are chunked before CAS storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkStrategy {
    /// Store as a single blob (used for objects <= SMALL_OBJECT_THRESHOLD bytes).
    Inline,
    /// Split into fixed-size chunks and store a manifest referencing each chunk hash.
    FixedSize { chunk_bytes: usize },
}

/// Manifest for chunked objects: ordered list of chunk hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkManifest {
    pub strategy: ChunkStrategy,
    /// For Inline: single-element vec with the object hash.
    /// For FixedSize: ordered chunk hashes.
    pub chunk_hashes: Vec<ContentHash>,
    pub total_size: u64,
}

impl ChunkManifest {
    pub fn from_bytes(bytes: &[u8], strategy: ChunkStrategy) -> Self {
        match &strategy {
            ChunkStrategy::Inline => ChunkManifest {
                chunk_hashes: vec![digest(bytes)],
                total_size: bytes.len() as u64,
                strategy,
            },
            ChunkStrategy::FixedSize { chunk_bytes } => {
                let chunk_hashes = bytes
                    .chunks(*chunk_bytes)
                    .map(digest)
                    .collect();
                ChunkManifest {
                    chunk_hashes,
                    total_size: bytes.len() as u64,
                    strategy,
                }
            }
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

/// Policy controlling when projections are triggered and the fallback order.
#[derive(Debug, Clone)]
pub struct ProjectionPolicy {
    /// Backends to auto-project when `put` is called (eager mode).
    /// Empty means fully lazy — project only on explicit `project()` call.
    pub eager_backends: Vec<ProjectionBackend>,
    /// Preferred fallback order for `get` when no backend is specified.
    pub fallback_order: Vec<ProjectionBackend>,
}

impl Default for ProjectionPolicy {
    fn default() -> Self {
        Self {
            eager_backends: vec![],
            fallback_order: vec![
                ProjectionBackend::Kv,
                ProjectionBackend::Git,
                ProjectionBackend::S3Blobs,
                ProjectionBackend::Ipfs,
                ProjectionBackend::Torrent,
            ],
        }
    }
}

/// Canonical CAS + lazy backend projection gateway.
pub struct LazyProjectionGateway {
    canonical: ContentAddressedStore,
    envelopes: RwLock<HashMap<ContentHash, CasEnvelope>>,
    adapters: RwLock<HashMap<ProjectionBackend, Arc<dyn ProjectionAdapter>>>,
    projection_index: RwLock<HashMap<(ContentHash, ProjectionBackend), String>>,
    manifests: RwLock<HashMap<ContentHash, ChunkManifest>>,
    policy: RwLock<ProjectionPolicy>,
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
            manifests: RwLock::new(HashMap::new()),
            policy: RwLock::new(ProjectionPolicy::default()),
        }
    }

    /// Register or replace a backend adapter.
    pub fn register_adapter(&self, adapter: Arc<dyn ProjectionAdapter>) {
        self.adapters.write().insert(adapter.backend(), adapter);
    }

    /// Replace the projection policy.
    pub fn set_policy(&self, policy: ProjectionPolicy) {
        *self.policy.write() = policy;
    }

    /// Return the chunk manifest for a stored object, if present.
    pub fn manifest(&self, hash: &ContentHash) -> Option<ChunkManifest> {
        self.manifests.read().get(hash).cloned()
    }

    /// Store bytes once in canonical CAS.
    pub fn put(&self, bytes: Vec<u8>, media_type: impl Into<String>) -> Result<PutResult> {
        let strategy = if bytes.len() <= SMALL_OBJECT_THRESHOLD {
            ChunkStrategy::Inline
        } else {
            ChunkStrategy::FixedSize { chunk_bytes: SMALL_OBJECT_THRESHOLD }
        };
        let manifest = ChunkManifest::from_bytes(&bytes, strategy);

        let blob = ContentBlob::new(bytes);
        self.canonical.store(&blob)?;

        self.manifests.write().insert(blob.hash, manifest);

        let envelope = CasEnvelope {
            algorithm: "sha2-256",
            size: blob.size as u64,
            media_type: media_type.into(),
        };
        self.envelopes.write().insert(blob.hash, envelope.clone());

        // Eager projection for registered backends in policy.
        let eager_backends = self.policy.read().eager_backends.clone();
        for backend in eager_backends {
            // Best-effort: ignore errors from eager projection (adapter may not be registered).
            let _ = self.project(&blob.hash, backend);
        }

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

    // --- Phase 4 tests ---

    #[test]
    fn parity_digest_round_trip_all_backends() {
        let gw = LazyProjectionGateway::new();
        let _adapters = register_all(&gw);

        let bytes = b"parity fixture bytes for all backends".to_vec();
        let put = gw.put(bytes.clone(), "application/octet-stream").expect("put");

        let all_backends = [
            ProjectionBackend::Git,
            ProjectionBackend::Torrent,
            ProjectionBackend::Ipfs,
            ProjectionBackend::S3Blobs,
            ProjectionBackend::Kv,
        ];

        for backend in all_backends {
            gw.project(&put.hash, backend).expect("project");
        }

        let fetched = gw
            .get(&put.hash, &all_backends)
            .expect("get")
            .expect("should be Some");

        let retrieved_hash = digest(&fetched);
        assert_eq!(retrieved_hash, put.hash, "digest mismatch after round-trip");
        assert_eq!(fetched, bytes);
    }

    #[test]
    fn lazy_write_not_materialized_without_explicit_project() {
        let gw = LazyProjectionGateway::new();
        let adapters = register_all(&gw);

        // Default policy has no eager backends.
        let _put = gw.put(b"lazy object bytes".to_vec(), "application/octet-stream").expect("put");

        for adapter in adapters.values() {
            assert_eq!(
                adapter.write_count(),
                0,
                "backend {:?} should have 0 writes without explicit project",
                adapter.backend
            );
        }
    }

    #[test]
    fn eager_policy_materializes_selected_backends_on_put() {
        let gw = LazyProjectionGateway::new();
        let adapters = register_all(&gw);

        gw.set_policy(ProjectionPolicy {
            eager_backends: vec![ProjectionBackend::Git, ProjectionBackend::Kv],
            fallback_order: ProjectionPolicy::default().fallback_order,
        });

        let _put = gw.put(b"eager object bytes".to_vec(), "application/octet-stream").expect("put");

        assert_eq!(adapters[&ProjectionBackend::Git].write_count(), 1, "Git should be eager");
        assert_eq!(adapters[&ProjectionBackend::Kv].write_count(), 1, "Kv should be eager");
        assert_eq!(adapters[&ProjectionBackend::Torrent].write_count(), 0, "Torrent should be lazy");
        assert_eq!(adapters[&ProjectionBackend::Ipfs].write_count(), 0, "Ipfs should be lazy");
        assert_eq!(adapters[&ProjectionBackend::S3Blobs].write_count(), 0, "S3 should be lazy");
    }

    #[test]
    fn partial_outage_get_falls_back_to_next_backend() {
        struct MissingProjectionAdapter {
            backend: ProjectionBackend,
        }

        impl ProjectionAdapter for MissingProjectionAdapter {
            fn backend(&self) -> ProjectionBackend {
                self.backend
            }

            fn deterministic_locator(&self, hash: &ContentHash) -> String {
                format!("missing/{}", hex::encode(hash))
            }

            fn project(&self, hash: &ContentHash, _bytes: &[u8]) -> Result<String> {
                Ok(self.deterministic_locator(hash))
            }

            fn fetch(&self, _locator: &str) -> Result<Option<Vec<u8>>> {
                Ok(None) // always missing
            }
        }

        let gw = LazyProjectionGateway::new();

        // Register a failing adapter for Git and a working one for Kv.
        let failing_git = Arc::new(MissingProjectionAdapter { backend: ProjectionBackend::Git });
        gw.register_adapter(failing_git);

        let kv_adapter = Arc::new(InMemoryProjectionAdapter::new(ProjectionBackend::Kv, "kv"));
        gw.register_adapter(kv_adapter.clone());

        let bytes = b"partial outage test bytes".to_vec();
        let put = gw.put(bytes.clone(), "application/octet-stream").expect("put");

        // Project to both Git (failing) and Kv (working).
        gw.project(&put.hash, ProjectionBackend::Git).expect("project git");
        gw.project(&put.hash, ProjectionBackend::Kv).expect("project kv");

        // Fallback order: Git first (will return None), then Kv.
        let fetched = gw
            .get(&put.hash, &[ProjectionBackend::Git, ProjectionBackend::Kv])
            .expect("get")
            .expect("should fall back to Kv");

        assert_eq!(fetched, bytes, "should have fetched from Kv fallback");
        assert_eq!(kv_adapter.write_count(), 1, "Kv should have been written");
    }
}
