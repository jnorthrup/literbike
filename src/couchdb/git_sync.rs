use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
    collections::HashMap,
};
use tokio::{
    sync::{mpsc, RwLock},
    time::sleep,
    fs,
};
use notify::{Watcher, RecursiveMode, Event, EventKind, event::CreateKind};
use git2::{Repository, Oid, TreeWalkMode, ObjectType, Diff, DiffOptions};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::couchdb::{
    types::*,
    database::CouchDatabase,
    error::CouchError,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDocument {
    pub _id: String,
    pub _rev: Option<String>,
    pub file_path: String,
    pub content: String,
    pub commit_hash: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    pub file_type: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct GitSyncConfig {
    pub repo_path: PathBuf,
    pub database_name: String,
    pub watch_patterns: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub sync_interval: Duration,
    pub auto_commit: bool,
    pub commit_message_template: String,
}

impl Default for GitSyncConfig {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::from("."),
            database_name: "git_sync".to_string(),
            watch_patterns: vec!["**/*.rs".to_string(), "**/*.md".to_string(), "**/*.toml".to_string()],
            ignore_patterns: vec![
                "**/target/**".to_string(),
                "**/.git/**".to_string(),
                "**/node_modules/**".to_string(),
            ],
            sync_interval: Duration::from_secs(30),
            auto_commit: false,
            commit_message_template: "Auto-sync: {files_changed} files changed".to_string(),
        }
    }
}

pub struct GitSyncManager {
    config: GitSyncConfig,
    database: Arc<CouchDatabase>,
    repository: Repository,
    watcher: Option<notify::RecommendedWatcher>,
    file_events: mpsc::UnboundedReceiver<Event>,
    file_sender: mpsc::UnboundedSender<Event>,
    last_sync: Arc<RwLock<SystemTime>>,
    pending_changes: Arc<RwLock<HashMap<PathBuf, SystemTime>>>,
}

impl GitSyncManager {
    pub async fn new(config: GitSyncConfig, database: Arc<CouchDatabase>) -> Result<Self, CouchError> {
        let repository = Repository::open(&config.repo_path)
            .map_err(|e| CouchError::Internal(format!("Failed to open git repository: {}", e)))?;

        let (file_sender, file_events) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            database,
            repository,
            watcher: None,
            file_events,
            file_sender,
            last_sync: Arc::new(RwLock::new(UNIX_EPOCH)),
            pending_changes: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn start_watching(&mut self) -> Result<(), CouchError> {
        let sender = self.file_sender.clone();
        let mut watcher = notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
            if let Ok(event) = result {
                let _ = sender.send(event);
            }
        }).map_err(|e| CouchError::Internal(format!("Failed to create file watcher: {}", e)))?;

        watcher.watch(&self.config.repo_path, RecursiveMode::Recursive)
            .map_err(|e| CouchError::Internal(format!("Failed to start watching: {}", e)))?;

        self.watcher = Some(watcher);

        // Initial sync of existing files
        self.sync_all_files().await?;

        Ok(())
    }

    pub async fn run(&mut self) -> Result<(), CouchError> {
        let mut sync_interval = tokio::time::interval(self.config.sync_interval);

        loop {
            tokio::select! {
                _ = sync_interval.tick() => {
                    self.process_pending_changes().await?;
                }
                
                Some(event) = self.file_events.recv() => {
                    self.handle_file_event(event).await?;
                }
                
                else => break,
            }
        }

        Ok(())
    }

    async fn handle_file_event(&self, event: Event) -> Result<(), CouchError> {
        match event.kind {
            EventKind::Create(CreateKind::File) | 
            EventKind::Modify(_) | 
            EventKind::Remove(_) => {
                for path in event.paths {
                    if self.should_sync_file(&path) {
                        let mut pending = self.pending_changes.write().await;
                        pending.insert(path, SystemTime::now());
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn should_sync_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check ignore patterns first
        for pattern in &self.config.ignore_patterns {
            if glob_match(pattern, &path_str) {
                return false;
            }
        }

        // Check watch patterns
        for pattern in &self.config.watch_patterns {
            if glob_match(pattern, &path_str) {
                return true;
            }
        }

        false
    }

    async fn process_pending_changes(&self) -> Result<(), CouchError> {
        let mut pending = self.pending_changes.write().await;
        let now = SystemTime::now();
        let changes_to_process: Vec<PathBuf> = pending
            .iter()
            .filter(|(_, &timestamp)| {
                now.duration_since(timestamp)
                    .unwrap_or(Duration::ZERO) > Duration::from_secs(5) // 5 second delay
            })
            .map(|(path, _)| path.clone())
            .collect();

        for path in changes_to_process {
            self.sync_file(&path).await?;
            pending.remove(&path);
        }

        if !pending.is_empty() && self.config.auto_commit {
            self.auto_commit().await?;
        }

        Ok(())
    }

    async fn sync_file(&self, path: &Path) -> Result<(), CouchError> {
        if !path.exists() {
            // File was deleted, remove from database
            let doc_id = self.path_to_doc_id(path);
            let _ = self.database.delete_document(&self.config.database_name, &doc_id, None).await;
            return Ok(());
        }

        let content = fs::read_to_string(path).await
            .map_err(|e| CouchError::Internal(format!("Failed to read file {}: {}", path.display(), e)))?;

        let file_metadata = fs::metadata(path).await
            .map_err(|e| CouchError::Internal(format!("Failed to get metadata for {}: {}", path.display(), e)))?;

        let current_commit = self.get_current_commit_hash()?;
        let author = self.get_current_author()?;

        let doc_id = self.path_to_doc_id(path);
        let file_type = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown")
            .to_string();

        let git_doc = GitDocument {
            _id: doc_id.clone(),
            _rev: None, // Will be set by database
            file_path: path.to_string_lossy().to_string(),
            content,
            commit_hash: current_commit,
            author,
            timestamp: Utc::now(),
            file_type,
            size: file_metadata.len(),
        };

        let doc_value = serde_json::to_value(&git_doc)
            .map_err(|e| CouchError::Serialization(e.to_string()))?;

        // Try to get existing document to preserve revision
        if let Ok(existing_doc) = self.database.get_document(&self.config.database_name, &doc_id).await {
            if let Some(rev) = existing_doc.get("_rev").and_then(|v| v.as_str()) {
                let mut updated_doc = git_doc;
                updated_doc._rev = Some(rev.to_string());
                let updated_value = serde_json::to_value(&updated_doc)
                    .map_err(|e| CouchError::Serialization(e.to_string()))?;
                self.database.update_document(&self.config.database_name, &doc_id, updated_value).await?;
            }
        } else {
            self.database.create_document(&self.config.database_name, Some(doc_id), doc_value).await?;
        }

        Ok(())
    }

    async fn sync_all_files(&self) -> Result<(), CouchError> {
        let walk_dir = |dir: &Path| -> Result<Vec<PathBuf>, std::io::Error> {
            let mut files = Vec::new();
            let mut stack = vec![dir.to_path_buf()];

            while let Some(current) = stack.pop() {
                if current.is_dir() {
                    for entry in std::fs::read_dir(current)? {
                        let entry = entry?;
                        let path = entry.path();
                        if path.is_dir() {
                            stack.push(path);
                        } else if path.is_file() {
                            files.push(path);
                        }
                    }
                }
            }

            Ok(files)
        };

        let files = walk_dir(&self.config.repo_path)
            .map_err(|e| CouchError::Internal(format!("Failed to walk directory: {}", e)))?;

        for file in files {
            if self.should_sync_file(&file) {
                self.sync_file(&file).await?;
            }
        }

        let mut last_sync = self.last_sync.write().await;
        *last_sync = SystemTime::now();

        Ok(())
    }

    async fn auto_commit(&self) -> Result<(), CouchError> {
        let pending = self.pending_changes.read().await;
        let files_changed = pending.len();
        
        if files_changed == 0 {
            return Ok(());
        }

        let commit_message = self.config.commit_message_template
            .replace("{files_changed}", &files_changed.to_string());

        self.git_commit(&commit_message).await?;

        Ok(())
    }

    async fn git_commit(&self, message: &str) -> Result<(), CouchError> {
        let mut index = self.repository.index()
            .map_err(|e| CouchError::Internal(format!("Failed to get git index: {}", e)))?;

        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| CouchError::Internal(format!("Failed to add files to git index: {}", e)))?;

        index.write()
            .map_err(|e| CouchError::Internal(format!("Failed to write git index: {}", e)))?;

        let tree_id = index.write_tree()
            .map_err(|e| CouchError::Internal(format!("Failed to write git tree: {}", e)))?;

        let tree = self.repository.find_tree(tree_id)
            .map_err(|e| CouchError::Internal(format!("Failed to find git tree: {}", e)))?;

        let parent_commit = self.repository.head()
            .and_then(|head| head.peel_to_commit())
            .ok();

        let signature = self.repository.signature()
            .map_err(|e| CouchError::Internal(format!("Failed to create git signature: {}", e)))?;

        let parents = if let Some(ref parent) = parent_commit {
            vec![parent]
        } else {
            vec![]
        };

        self.repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        ).map_err(|e| CouchError::Internal(format!("Failed to create git commit: {}", e)))?;

        Ok(())
    }

    fn path_to_doc_id(&self, path: &Path) -> String {
        let relative_path = path.strip_prefix(&self.config.repo_path)
            .unwrap_or(path);
        
        format!("file:{}", relative_path.to_string_lossy().replace('/', ":"))
    }

    fn get_current_commit_hash(&self) -> Result<String, CouchError> {
        let head = self.repository.head()
            .map_err(|e| CouchError::Internal(format!("Failed to get HEAD: {}", e)))?;
        
        let commit = head.peel_to_commit()
            .map_err(|e| CouchError::Internal(format!("Failed to get commit: {}", e)))?;

        Ok(commit.id().to_string())
    }

    fn get_current_author(&self) -> Result<String, CouchError> {
        let config = self.repository.config()
            .map_err(|e| CouchError::Internal(format!("Failed to get git config: {}", e)))?;

        let name = config.get_string("user.name").unwrap_or_else(|_| "Unknown".to_string());
        let email = config.get_string("user.email").unwrap_or_else(|_| "unknown@example.com".to_string());

        Ok(format!("{} <{}>", name, email))
    }

    pub async fn get_file_history(&self, file_path: &str) -> Result<Vec<GitDocument>, CouchError> {
        let query = format!(r#"
        {{
            "selector": {{
                "file_path": "{}"
            }},
            "sort": [
                {{"timestamp": "desc"}}
            ]
        }}
        "#, file_path);

        let query_value: serde_json::Value = serde_json::from_str(&query)
            .map_err(|e| CouchError::Serialization(e.to_string()))?;

        let results = self.database.find_documents(&self.config.database_name, query_value).await?;
        
        let mut documents = Vec::new();
        if let Some(docs) = results.get("docs").and_then(|d| d.as_array()) {
            for doc in docs {
                if let Ok(git_doc) = serde_json::from_value::<GitDocument>(doc.clone()) {
                    documents.push(git_doc);
                }
            }
        }

        Ok(documents)
    }

    pub async fn restore_file_version(&self, file_path: &str, commit_hash: &str) -> Result<(), CouchError> {
        let query = format!(r#"
        {{
            "selector": {{
                "file_path": "{}",
                "commit_hash": "{}"
            }}
        }}
        "#, file_path, commit_hash);

        let query_value: serde_json::Value = serde_json::from_str(&query)
            .map_err(|e| CouchError::Serialization(e.to_string()))?;

        let results = self.database.find_documents(&self.config.database_name, query_value).await?;
        
        if let Some(docs) = results.get("docs").and_then(|d| d.as_array()) {
            if let Some(doc) = docs.first() {
                if let Ok(git_doc) = serde_json::from_value::<GitDocument>(doc.clone()) {
                    let full_path = self.config.repo_path.join(&git_doc.file_path);
                    
                    if let Some(parent) = full_path.parent() {
                        fs::create_dir_all(parent).await
                            .map_err(|e| CouchError::Internal(format!("Failed to create directories: {}", e)))?;
                    }
                    
                    fs::write(&full_path, &git_doc.content).await
                        .map_err(|e| CouchError::Internal(format!("Failed to write file: {}", e)))?;
                    
                    return Ok(());
                }
            }
        }

        Err(CouchError::NotFound(format!("File version not found: {} @ {}", file_path, commit_hash)))
    }
}

// Simple glob matching function
fn glob_match(pattern: &str, text: &str) -> bool {
    let regex_pattern = pattern
        .replace("**", ".*")
        .replace("*", "[^/]*")
        .replace("?", ".");
    
    if let Ok(regex) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
        regex.is_match(text)
    } else {
        false
    }
}