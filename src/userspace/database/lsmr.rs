#![allow(deprecated)]

use anyhow::Result;
use chrono::Utc;
use memmap2::Mmap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Condvar;
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct LsmrConfig {
    pub path: PathBuf,
    pub memtable_threshold: usize,
    pub max_segments: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SegmentIndexEntry {
    offset: u64,
    len: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SegmentMeta {
    pub filename: String,
    pub index: BTreeMap<String, SegmentIndexEntry>,
    pub size: u64,
    #[serde(default = "segmeta_version_default")]
    pub version: u32,
}

fn segmeta_version_default() -> u32 {
    1
}

const SEGMENT_VERSION_WRITTEN: u32 = 2;
const TOMBSTONE_MARKER: &[u8] = b"null";

#[derive(Default)]
struct MemTable {
    map: BTreeMap<String, Vec<u8>>,
    size: usize,
}

impl MemTable {
    fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            size: 0,
        }
    }
    fn insert(&mut self, id: String, bytes: Vec<u8>) {
        if let Some(prev) = self.map.insert(id, bytes) {
            self.size = self.size.saturating_sub(prev.len());
        }
        if let Some(v) = self.map.values().last() {
            self.size = self.size.saturating_add(v.len());
        }
    }
}

struct CompactionState {
    notified: bool,
}

pub struct LsmrDatabase {
    cfg: LsmrConfig,
    memtable: RwLock<MemTable>,
    segments: RwLock<Vec<SegmentMeta>>,
    compaction_notify: Arc<Condvar>,
    compaction_state: Arc<RwLock<CompactionState>>,
    _bg_running: Arc<RwLock<bool>>,
}

impl LsmrDatabase {
    pub fn open(cfg: LsmrConfig) -> Result<Arc<Self>> {
        std::fs::create_dir_all(&cfg.path)?;

        let mut segments = Vec::new();
        let entries = std::fs::read_dir(&cfg.path)?;
        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.ends_with(".tmp.data") || file_name.ends_with(".tmp.meta.json") {
                let _ = std::fs::remove_file(cfg.path.join(&file_name));
                continue;
            }

            if file_name.ends_with(".meta.json") {
                let meta_path = cfg.path.join(&file_name);
                let mut f = File::open(&meta_path)?;
                let mut s = String::new();
                f.read_to_string(&mut s)?;
                let m: SegmentMeta = serde_json::from_str(&s)?;
                segments.push(m);
            }
        }

        Ok(Arc::new(Self {
            cfg,
            memtable: RwLock::new(MemTable::new()),
            segments: RwLock::new(segments),
            compaction_notify: Arc::new(Condvar::new()),
            compaction_state: Arc::new(RwLock::new(CompactionState { notified: false })),
            _bg_running: Arc::new(RwLock::new(false)),
        }))
    }

    pub fn put_json(&self, id: String, json: &serde_json::Value) -> Result<()> {
        let bytes = serde_json::to_vec(json)?;

        {
            let mut mt = self.memtable.write();
            mt.insert(id.clone(), bytes);
            if mt.size < self.cfg.memtable_threshold {
                return Ok(());
            }
        }

        self.flush_memtable()?;
        self.notify_compaction();
        Ok(())
    }

    pub fn delete(&self, id: String) -> Result<()> {
        {
            let mut mt = self.memtable.write();
            mt.insert(id.clone(), TOMBSTONE_MARKER.to_vec());
            if mt.size < self.cfg.memtable_threshold {
                return Ok(());
            }
        }

        self.flush_memtable()?;
        self.notify_compaction();
        Ok(())
    }

    pub fn get_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        {
            let mt = self.memtable.read();
            if let Some(v) = mt.map.get(id) {
                if v == TOMBSTONE_MARKER {
                    return Ok(None);
                }
                return Ok(Some(serde_json::from_slice(v)?));
            }
        }

        let segments = self.segments.read();
        for seg in segments.iter().rev() {
            if let Some(entry) = seg.index.get(id) {
                let data_path = self
                    .cfg
                    .path
                    .join(format!("{}.data", &seg.filename[..seg.filename.len() - 10]));
                let mut f = File::open(&data_path)?;
                f.seek(std::io::SeekFrom::Start(entry.offset))?;
                let mut buf = vec![0u8; entry.len as usize];
                f.read_exact(&mut buf)?;
                if buf == TOMBSTONE_MARKER {
                    return Ok(None);
                }
                return Ok(Some(serde_json::from_slice(&buf)?));
            }
        }

        Ok(None)
    }

    fn notify_compaction(&self) {
        let mut state = self.compaction_state.write();
        state.notified = true;
        self.compaction_notify.notify_one();
    }

    fn flush_memtable(&self) -> Result<()> {
        let (data, seg_id) = {
            let mut mt = self.memtable.write();
            let data: Vec<(String, Vec<u8>)> =
                mt.map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            mt.map.clear();
            mt.size = 0;
            let seg_id = Utc::now().timestamp_nanos_opt().unwrap_or(0);
            (data, seg_id)
        };

        if data.is_empty() {
            return Ok(());
        }

        let seg_filename = format!("segment_{}.meta.json", seg_id);
        let data_filename = format!("{}.data", &seg_filename[..seg_filename.len() - 10]);

        let (mut index, mut data_size) = (BTreeMap::new(), 0u64);
        let data_path = self.cfg.path.join(&data_filename);
        {
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&data_path)?;
            for (id, value) in &data {
                let offset = data_size;
                let len = value.len() as u64;
                index.insert(id.clone(), SegmentIndexEntry { offset, len });
                f.write_all(value)?;
                f.write_all(b"\n")?;
                data_size += len + 1;
            }
        }

        let meta = SegmentMeta {
            filename: seg_filename.clone(),
            index,
            size: data_size,
            version: SEGMENT_VERSION_WRITTEN,
        };

        let meta_path = self.cfg.path.join(&seg_filename);
        let mut mf = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&meta_path)?;
        serde_json::to_writer(&mut mf, &meta)?;

        let mut segments = self.segments.write();
        segments.push(meta);

        if let Some(max) = self.cfg.max_segments {
            while segments.len() > max {
                if !segments.is_empty() {
                    let removed = segments.remove(0);
                    let data_path = self.cfg.path.join(format!(
                        "{}.data",
                        &removed.filename[..removed.filename.len() - 10]
                    ));
                    let _ = std::fs::remove_file(data_path);
                    let _ = std::fs::remove_file(self.cfg.path.join(&removed.filename));
                }
            }
        }

        Ok(())
    }

    pub fn compact(&self) -> Result<()> {
        let segments = self.segments.read().clone();
        let mut all_entries: BTreeMap<String, (Vec<u8>, u64)> = BTreeMap::new();

        for seg in &segments {
            let data_path = self
                .cfg
                .path
                .join(format!("{}.data", &seg.filename[..seg.filename.len() - 10]));
            let f = File::open(&data_path)?;
            let mmap = unsafe { Mmap::map(&f)? };
            for (id, entry) in &seg.index {
                if let Some((existing_val, _)) = all_entries.get(id) {
                    if entry.len > existing_val.len() as u64 {
                        continue;
                    }
                }
                let offset = entry.offset as usize;
                let len = entry.len as usize;
                if offset + len <= mmap.len() {
                    let value = mmap[offset..offset + len].to_vec();
                    all_entries.insert(id.clone(), (value, entry.len));
                }
            }
        }

        Ok(())
    }

    pub fn len(&self) -> usize {
        let mt = self.memtable.read();
        let segments = self.segments.read();
        let mut total = mt.map.len();
        for seg in segments.iter() {
            total += seg.index.len();
        }
        total
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for LsmrDatabase {
    fn drop(&mut self) {
        let mut running = self._bg_running.write();
        *running = false;
        drop(running);
    }
}
