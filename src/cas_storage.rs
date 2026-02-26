// Content-Addressable Storage — block storage keyed by hash/CID

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockId(pub Vec<u8>);

impl BlockId {
    pub fn from_bytes(b: impl Into<Vec<u8>>) -> Self { Self(b.into()) }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    pub data: Vec<u8>,
}

impl Block {
    pub fn new(id: BlockId, data: Vec<u8>) -> Self { Self { id, data } }
}

#[derive(Debug)]
pub enum CasError {
    NotFound,
    StorageError(String),
}

pub trait CasStorage: Send + Sync {
    fn store(&self, block: Block) -> Result<(), CasError>;
    fn retrieve(&self, id: &BlockId) -> Result<Option<Block>, CasError>;
    fn has(&self, id: &BlockId) -> bool;
    fn remove(&self, id: &BlockId) -> Result<(), CasError>;
}

/// In-memory CAS implementation
pub struct MemoryCasStorage {
    blocks: Arc<RwLock<HashMap<BlockId, Block>>>,
}

impl MemoryCasStorage {
    pub fn new() -> Self {
        Self { blocks: Arc::new(RwLock::new(HashMap::new())) }
    }
}

impl Default for MemoryCasStorage {
    fn default() -> Self { Self::new() }
}

impl CasStorage for MemoryCasStorage {
    fn store(&self, block: Block) -> Result<(), CasError> {
        self.blocks.write().insert(block.id.clone(), block);
        Ok(())
    }
    fn retrieve(&self, id: &BlockId) -> Result<Option<Block>, CasError> {
        Ok(self.blocks.read().get(id).cloned())
    }
    fn has(&self, id: &BlockId) -> bool {
        self.blocks.read().contains_key(id)
    }
    fn remove(&self, id: &BlockId) -> Result<(), CasError> {
        self.blocks.write().remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn store_retrieve_remove() {
        let cas = MemoryCasStorage::new();
        let id = BlockId::from_bytes(b"abc".to_vec());
        let block = Block::new(id.clone(), b"data".to_vec());
        cas.store(block).unwrap();
        assert!(cas.has(&id));
        let got = cas.retrieve(&id).unwrap().unwrap();
        assert_eq!(got.data, b"data");
        cas.remove(&id).unwrap();
        assert!(!cas.has(&id));
    }
}
