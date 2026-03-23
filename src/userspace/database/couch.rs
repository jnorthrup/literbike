use anyhow::Result;
use memmap2::Mmap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub rev: String,
    pub content: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IndexEntry {
    offset: u64,
    len: u64,
    rev: String,
}

pub struct CouchDatabase {
    pub path: PathBuf,
    index: RwLock<HashMap<String, IndexEntry>>,
    data_file: PathBuf,
    index_file: PathBuf,
}

impl CouchDatabase {
    pub fn new(path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&path)?;

        let data_file = path.join("data.bin");
        let index_file = path.join("index.json");

        let idx: HashMap<String, IndexEntry> = if index_file.exists() {
            let mut f = File::open(&index_file)?;
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            serde_json::from_str(&s)?
        } else {
            HashMap::new()
        };

        Ok(Self {
            path,
            index: RwLock::new(idx),
            data_file,
            index_file,
        })
    }

    pub fn put(&self, doc: Document) -> Result<()> {
        let bytes = serde_json::to_vec(&doc)?;
        let len = bytes.len() as u64;

        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.data_file)?;
        let offset = f.seek(SeekFrom::End(0))?;
        f.write_all(&bytes)?;
        f.flush()?;

        let mut idx = self.index.write();
        idx.insert(
            doc.id.clone(),
            IndexEntry {
                offset,
                len,
                rev: doc.rev,
            },
        );

        let idx_clone = idx.clone();
        let mut mf = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.index_file)?;
        let s = serde_json::to_string(&idx_clone)?;
        mf.write_all(s.as_bytes())?;
        mf.flush()?;

        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<Document>> {
        let idx = self.index.read();
        let entry = match idx.get(id) {
            Some(e) => e.clone(),
            None => return Ok(None),
        };
        drop(idx);

        let f = File::open(&self.data_file)?;
        let mmap = unsafe { Mmap::map(&f)? };
        let start = entry.offset as usize;
        let end = start + entry.len as usize;
        if end > mmap.len() {
            anyhow::bail!("index entry out of bounds");
        }
        let slice = &mmap[start..end];
        let doc: Document = serde_json::from_slice(slice)?;
        Ok(Some(doc))
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let mut idx = self.index.write();
        if idx.remove(id).is_some() {
            let idx_clone = idx.clone();
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.index_file)?;
            let s = serde_json::to_string(&idx_clone)?;
            f.write_all(s.as_bytes())?;
            f.flush()?;
        }
        Ok(())
    }
}
