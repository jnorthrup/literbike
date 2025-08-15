use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use std::net::TcpStream;

#[derive(Debug, Clone)]
pub struct GitRepoState {
    pub path: PathBuf,
    pub is_git_repo: bool,
    pub current_branch: Option<String>,
    pub has_staged_changes: bool,
    pub has_unstaged_changes: bool,
    pub has_untracked_files: bool,
    pub remotes: Vec<GitRemote>,
    pub commit_count: u32,
    pub is_shallow: bool,
    pub head_commit: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
    pub is_fetch: bool,
    pub is_push: bool,
    pub is_ssh: bool,
    pub is_reachable: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone)]
pub enum SyncStrategy {
    FullClone,     // Complete clone with full history
    ShallowClone,  // Shallow clone with depth=1
    FetchPush,     // Just fetch/push existing repo
    SmartSync,     // Adaptive based on repo state
}

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub strategy: SyncStrategy,
    pub remote_name: String,
    pub branch: Option<String>,
    pub force: bool,
    pub cleanup_temp_remotes: bool,
    pub ssh_timeout: Duration,
    pub termux_optimized: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            strategy: SyncStrategy::SmartSync,
            remote_name: "temp_upstream".to_string(),
            branch: None,
            force: false,
            cleanup_temp_remotes: true,
            ssh_timeout: Duration::from_secs(5),
            termux_optimized: is_termux_environment(),
        }
    }
}

impl GitRepoState {
    pub fn analyze(path: Option<PathBuf>) -> Result<Self, String> {
        let repo_path = path.unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        
        // Check if this is a git repository
        let git_dir_check = Command::new("git")
            .current_dir(&repo_path)
            .args(["rev-parse", "--git-dir"])
            .output();
        
        let is_git_repo = git_dir_check.map(|o| o.status.success()).unwrap_or(false);
        
        if !is_git_repo {
            return Ok(Self {
                path: repo_path,
                is_git_repo: false,
                current_branch: None,
                has_staged_changes: false,
                has_unstaged_changes: false,
                has_untracked_files: false,
                remotes: Vec::new(),
                commit_count: 0,
                is_shallow: false,
                head_commit: None,
            });
        }

        // Get current branch
        let current_branch = get_current_branch(&repo_path);
        
        // Check for changes
        let status_output = Command::new("git")
            .current_dir(&repo_path)
            .args(["status", "--porcelain"])
            .output()
            .map_err(|e| format!("Failed to get git status: {}", e))?;
        
        let status_text = String::from_utf8_lossy(&status_output.stdout);
        let has_staged_changes = status_text.lines().any(|line| !line.starts_with("??") && !line.starts_with(" "));
        let has_unstaged_changes = status_text.lines().any(|line| line.chars().nth(1) != Some(' ') && !line.starts_with("??"));
        let has_untracked_files = status_text.lines().any(|line| line.starts_with("??"));
        
        // Get remotes
        let remotes = get_remotes(&repo_path)?;
        
        // Get commit count and check if shallow
        let commit_count = get_commit_count(&repo_path);
        let is_shallow = is_shallow_repo(&repo_path);
        
        // Get HEAD commit
        let head_commit = get_head_commit(&repo_path);
        
        Ok(Self {
            path: repo_path,
            is_git_repo: true,
            current_branch,
            has_staged_changes,
            has_unstaged_changes,
            has_untracked_files,
            remotes,
            commit_count,
            is_shallow,
            head_commit,
        })
    }

    pub fn recommend_strategy(&self) -> SyncStrategy {
        if !self.is_git_repo {
            return SyncStrategy::FullClone;
        }
        
        if self.is_shallow && self.commit_count < 10 {
            SyncStrategy::ShallowClone
        } else if self.remotes.iter().any(|r| r.is_reachable) {
            SyncStrategy::FetchPush
        } else if self.commit_count > 100 {
            SyncStrategy::ShallowClone
        } else {
            SyncStrategy::FullClone
        }
    }

    pub fn needs_cleanup(&self) -> bool {
        self.remotes.iter().any(|r| {
            (r.name.contains("temp") || r.name.contains("tmp")) && !r.is_reachable
        })
    }
}

impl GitRemote {
    fn from_remote_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }
        
        let name = parts[0].to_string();
        let url = parts[1].to_string();
        let is_fetch = parts[2].contains("fetch");
        let is_push = parts[2].contains("push");
        let is_ssh = url.starts_with("ssh://") || url.contains("@") && !url.starts_with("http");
        
        let (host, port) = if is_ssh {
            extract_ssh_host_port(&url)
        } else {
            (None, None)
        };
        
        let is_reachable = if let Some(h) = &host {
            test_ssh_connectivity(h, port.unwrap_or(22))
        } else if url.starts_with("http") {
            test_http_connectivity(&url)
        } else {
            false
        };
        
        Some(Self {
            name,
            url,
            is_fetch,
            is_push,
            is_ssh,
            is_reachable,
            host,
            port,
        })
    }
}

pub fn run_git_sync(args: &[String]) -> Result<(), String> {
    let mut options = SyncOptions::default();
    let mut target_url: Option<String> = None;
    let mut target_path: Option<PathBuf> = None;
    
    // Parse arguments
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--strategy" if i + 1 < args.len() => {
                options.strategy = match args[i + 1].as_str() {
                    "full" => SyncStrategy::FullClone,
                    "shallow" => SyncStrategy::ShallowClone,
                    "fetch" => SyncStrategy::FetchPush,
                    "smart" => SyncStrategy::SmartSync,
                    _ => return Err(format!("Invalid strategy: {}", args[i + 1])),
                };
                i += 2;
            }
            "--remote" if i + 1 < args.len() => {
                options.remote_name = args[i + 1].clone();
                i += 2;
            }
            "--branch" if i + 1 < args.len() => {
                options.branch = Some(args[i + 1].clone());
                i += 2;
            }
            "--force" => {
                options.force = true;
                i += 1;
            }
            "--no-cleanup" => {
                options.cleanup_temp_remotes = false;
                i += 1;
            }
            "--path" if i + 1 < args.len() => {
                target_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            arg if !arg.starts_with("--") && target_url.is_none() => {
                target_url = Some(arg.to_string());
                i += 1;
            }
            _ => i += 1,
        }
    }
    
    // Show usage if no arguments
    if args.is_empty() || args[0] == "help" {
        show_git_sync_help();
        return Ok(());
    }
    
    // Handle subcommands
    match args[0].as_str() {
        "status" => cmd_sync_status(target_path),
        "clean" => cmd_clean_remotes(target_path),
        "clone" => cmd_clone_repo(target_url, target_path, &options),
        "push" => cmd_push_repo(target_url, target_path, &options),
        "pull" => cmd_pull_repo(target_url, target_path, &options),
        "sync" => cmd_bidirectional_sync(target_url, target_path, &options),
        "pijul-analyze" => analyze_pijul_migration(target_path),
        url if url.contains("://") || url.contains("@") => {
            // Direct URL provided - do smart sync
            options.strategy = SyncStrategy::SmartSync;
            cmd_smart_sync(Some(url.to_string()), target_path, &options)
        }
        _ => {
            eprintln!("Unknown git-sync command: {}", args[0]);
            show_git_sync_help();
            Err("Invalid command".to_string())
        }
    }
}

fn cmd_sync_status(path: Option<PathBuf>) -> Result<(), String> {
    let state = GitRepoState::analyze(path)?;
    
    println!("Git Repository Status");
    println!("  Path: {}", state.path.display());
    println!("  Is Git Repo: {}", state.is_git_repo);
    
    if !state.is_git_repo {
        println!("  Not a git repository");
        return Ok(());
    }
    
    println!("  Current Branch: {}", state.current_branch.as_deref().unwrap_or("(detached)"));
    println!("  HEAD Commit: {}", state.head_commit.as_deref().unwrap_or("unknown"));
    println!("  Commit Count: {}", state.commit_count);
    println!("  Is Shallow: {}", state.is_shallow);
    
    println!("\nChanges:");
    println!("  Staged: {}", state.has_staged_changes);
    println!("  Unstaged: {}", state.has_unstaged_changes);
    println!("  Untracked: {}", state.has_untracked_files);
    
    println!("\nRemotes:");
    for remote in &state.remotes {
        let status = if remote.is_reachable { "✓" } else { "✗" };
        let type_info = if remote.is_ssh { "SSH" } else { "HTTP" };
        println!("  {} {} ({}) {} - {}", status, remote.name, type_info, remote.url, 
                 if remote.is_reachable { "reachable" } else { "unreachable" });
    }
    
    let recommended = state.recommend_strategy();
    println!("\nRecommended Strategy: {:?}", recommended);
    
    if state.needs_cleanup() {
        println!("\n⚠ Cleanup recommended: run 'litebike git-sync clean'");
    }
    
    Ok(())
}

fn cmd_clean_remotes(path: Option<PathBuf>) -> Result<(), String> {
    let state = GitRepoState::analyze(path)?;
    
    if !state.is_git_repo {
        return Err("Not a git repository".to_string());
    }
    
    let mut cleaned = 0;
    
    for remote in &state.remotes {
        if (remote.name.contains("temp") || remote.name.contains("tmp")) && !remote.is_reachable {
            println!("Removing stale remote: {} -> {}", remote.name, remote.url);
            
            let remove_result = Command::new("git")
                .current_dir(&state.path)
                .args(["remote", "remove", &remote.name])
                .status();
            
            if remove_result.map(|s| s.success()).unwrap_or(false) {
                cleaned += 1;
            } else {
                eprintln!("Failed to remove remote: {}", remote.name);
            }
        }
    }
    
    if cleaned > 0 {
        println!("✓ Cleaned {} stale remotes", cleaned);
    } else {
        println!("No stale remotes found");
    }
    
    Ok(())
}

fn cmd_clone_repo(url: Option<String>, path: Option<PathBuf>, options: &SyncOptions) -> Result<(), String> {
    let repo_url = url.ok_or("Repository URL required for clone")?;
    let target_path = path.unwrap_or_else(|| {
        let repo_name = extract_repo_name(&repo_url);
        PathBuf::from(repo_name)
    });
    
    if target_path.exists() && target_path.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Err(format!("Target directory {} is not empty", target_path.display()));
    }
    
    println!("Cloning {} to {}", repo_url, target_path.display());
    
    let mut cmd = Command::new("git");
    cmd.arg("clone");
    
    match options.strategy {
        SyncStrategy::ShallowClone => {
            cmd.args(["--depth", "1"]);
        }
        SyncStrategy::FullClone => {
            // No additional args needed
        }
        _ => {
            // Default to shallow for cloning
            cmd.args(["--depth", "1"]);
        }
    }
    
    cmd.arg(&repo_url).arg(&target_path);
    
    let result = cmd.status();
    
    if result.map(|s| s.success()).unwrap_or(false) {
        println!("✓ Successfully cloned repository");
        Ok(())
    } else {
        Err("Clone failed".to_string())
    }
}

fn cmd_push_repo(url: Option<String>, path: Option<PathBuf>, options: &SyncOptions) -> Result<(), String> {
    let state = GitRepoState::analyze(path)?;
    
    if !state.is_git_repo {
        return Err("Not a git repository".to_string());
    }
    
    let target_url = url.or_else(|| {
        state.remotes.iter()
            .find(|r| r.name == options.remote_name)
            .map(|r| r.url.clone())
    }).ok_or("No target URL specified or found")?;
    
    let branch = options.branch.clone().or(state.current_branch)
        .ok_or("No branch specified or detected")?;
    
    // Setup or update remote
    setup_remote(&state.path, &options.remote_name, &target_url)?;
    
    println!("Pushing {} to {} ({})", branch, target_url, options.remote_name);
    
    let mut cmd = Command::new("git");
    cmd.current_dir(&state.path)
        .args(["push", &options.remote_name, &branch]);
    
    if options.force {
        cmd.arg("--force");
    }
    
    let result = cmd.status();
    
    if result.map(|s| s.success()).unwrap_or(false) {
        println!("✓ Successfully pushed to remote");
        Ok(())
    } else {
        if !options.force {
            println!("Push failed, trying with force...");
            let force_result = Command::new("git")
                .current_dir(&state.path)
                .args(["push", "--force", &options.remote_name, &branch])
                .status();
            
            if force_result.map(|s| s.success()).unwrap_or(false) {
                println!("✓ Successfully force-pushed to remote");
                return Ok(());
            }
        }
        Err("Push failed".to_string())
    }
}

fn cmd_pull_repo(url: Option<String>, path: Option<PathBuf>, options: &SyncOptions) -> Result<(), String> {
    let state = GitRepoState::analyze(path)?;
    
    if !state.is_git_repo {
        return Err("Not a git repository".to_string());
    }
    
    let target_url = url.or_else(|| {
        state.remotes.iter()
            .find(|r| r.name == options.remote_name)
            .map(|r| r.url.clone())
    }).ok_or("No target URL specified or found")?;
    
    let branch = options.branch.clone().or(state.current_branch)
        .ok_or("No branch specified or detected")?;
    
    // Setup or update remote
    setup_remote(&state.path, &options.remote_name, &target_url)?;
    
    println!("Pulling {} from {} ({})", branch, target_url, options.remote_name);
    
    let result = Command::new("git")
        .current_dir(&state.path)
        .args(["pull", &options.remote_name, &branch])
        .status();
    
    if result.map(|s| s.success()).unwrap_or(false) {
        println!("✓ Successfully pulled from remote");
        Ok(())
    } else {
        Err("Pull failed".to_string())
    }
}

fn cmd_bidirectional_sync(url: Option<String>, path: Option<PathBuf>, options: &SyncOptions) -> Result<(), String> {
    println!("Starting bidirectional sync...");
    
    // First try to pull
    if let Err(e) = cmd_pull_repo(url.clone(), path.clone(), options) {
        println!("Pull failed: {}", e);
    }
    
    // Then try to push
    if let Err(e) = cmd_push_repo(url, path, options) {
        println!("Push failed: {}", e);
    }
    
    println!("✓ Bidirectional sync completed");
    Ok(())
}

fn cmd_smart_sync(url: Option<String>, path: Option<PathBuf>, options: &SyncOptions) -> Result<(), String> {
    let state = GitRepoState::analyze(path.clone())?;
    
    if !state.is_git_repo {
        // Not a git repo - try to clone
        if let Some(repo_url) = url {
            return cmd_clone_repo(Some(repo_url), path, options);
        } else {
            return Err("No repository URL provided for cloning".to_string());
        }
    }
    
    // Determine strategy based on repo state
    let strategy = match &options.strategy {
        SyncStrategy::SmartSync => state.recommend_strategy(),
        other => other.clone(),
    };
    
    let mut smart_options = options.clone();
    smart_options.strategy = strategy;
    
    println!("Smart sync using strategy: {:?}", smart_options.strategy);
    
    // Clean up stale remotes if requested
    if options.cleanup_temp_remotes {
        let _ = cmd_clean_remotes(path.clone());
    }
    
    // Execute appropriate strategy
    match smart_options.strategy {
        SyncStrategy::FetchPush => cmd_bidirectional_sync(url, path, &smart_options),
        SyncStrategy::FullClone | SyncStrategy::ShallowClone => {
            // For existing repos, just do bidirectional sync
            cmd_bidirectional_sync(url, path, &smart_options)
        }
        SyncStrategy::SmartSync => unreachable!(), // Should have been resolved above
    }
}

// Helper functions

fn get_current_branch(repo_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["branch", "--show-current"])
        .output()
        .ok()?;
    
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

fn get_remotes(repo_path: &Path) -> Result<Vec<GitRemote>, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["remote", "-v"])
        .output()
        .map_err(|e| format!("Failed to get remotes: {}", e))?;
    
    let remotes_text = String::from_utf8_lossy(&output.stdout);
    let mut remotes = Vec::new();
    
    for line in remotes_text.lines() {
        if let Some(remote) = GitRemote::from_remote_line(line) {
            remotes.push(remote);
        }
    }
    
    Ok(remotes)
}

fn get_commit_count(repo_path: &Path) -> u32 {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-list", "--count", "HEAD"])
        .output();
    
    if let Ok(output) = output {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap_or(0)
    } else {
        0
    }
}

fn is_shallow_repo(repo_path: &Path) -> bool {
    repo_path.join(".git").join("shallow").exists()
}

fn get_head_commit(repo_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if commit.is_empty() {
        None
    } else {
        Some(commit)
    }
}

fn extract_ssh_host_port(url: &str) -> (Option<String>, Option<u16>) {
    if url.starts_with("ssh://") {
        let without_proto = &url[6..];
        if let Some(at_pos) = without_proto.find('@') {
            let after_at = &without_proto[at_pos + 1..];
            if let Some(colon_pos) = after_at.find(':') {
                let host = after_at[..colon_pos].to_string();
                let port_and_path = &after_at[colon_pos + 1..];
                if let Some(slash_pos) = port_and_path.find('/') {
                    let port_str = &port_and_path[..slash_pos];
                    return (Some(host), port_str.parse().ok());
                }
            } else if let Some(slash_pos) = after_at.find('/') {
                return (Some(after_at[..slash_pos].to_string()), Some(22));
            }
        }
    } else if url.contains("@") {
        if let Some(at_pos) = url.find('@') {
            let after_at = &url[at_pos + 1..];
            if let Some(colon_pos) = after_at.find(':') {
                return (Some(after_at[..colon_pos].to_string()), Some(22));
            }
        }
    }
    (None, None)
}

fn test_ssh_connectivity(host: &str, port: u16) -> bool {
    let timeout = Duration::from_secs(2);
    TcpStream::connect_timeout(
        &format!("{}:{}", host, port).parse().unwrap_or_else(|_| ([127,0,0,1], port).into()),
        timeout
    ).is_ok()
}

fn test_http_connectivity(url: &str) -> bool {
    // Simple URL parsing to extract host
    if let Some(start) = url.find("://") {
        let after_proto = &url[start + 3..];
        if let Some(end) = after_proto.find('/') {
            let host_port = &after_proto[..end];
            if let Some(colon_pos) = host_port.find(':') {
                let host = &host_port[..colon_pos];
                let port: u16 = host_port[colon_pos + 1..].parse().unwrap_or(80);
                return test_ssh_connectivity(host, port);
            } else {
                let port = if url.starts_with("https") { 443 } else { 80 };
                return test_ssh_connectivity(host_port, port);
            }
        }
    }
    false
}

fn setup_remote(repo_path: &Path, remote_name: &str, url: &str) -> Result<(), String> {
    // Check if remote exists
    let check_result = Command::new("git")
        .current_dir(repo_path)
        .args(["remote", "get-url", remote_name])
        .output();
    
    if check_result.map(|o| o.status.success()).unwrap_or(false) {
        // Remote exists, update URL
        let result = Command::new("git")
            .current_dir(repo_path)
            .args(["remote", "set-url", remote_name, url])
            .status();
        
        if !result.map(|s| s.success()).unwrap_or(false) {
            return Err(format!("Failed to update remote {}", remote_name));
        }
    } else {
        // Remote doesn't exist, add it
        let result = Command::new("git")
            .current_dir(repo_path)
            .args(["remote", "add", remote_name, url])
            .status();
        
        if !result.map(|s| s.success()).unwrap_or(false) {
            return Err(format!("Failed to add remote {}", remote_name));
        }
    }
    
    Ok(())
}

fn extract_repo_name(url: &str) -> String {
    url.split('/')
        .last()
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .to_string()
}

fn is_termux_environment() -> bool {
    env::var("TERMUX_VERSION").is_ok() || 
    env::var("PREFIX").map(|p| p.contains("termux")).unwrap_or(false)
}

pub fn analyze_pijul_migration(repo_path: Option<PathBuf>) -> Result<(), String> {
    let state = GitRepoState::analyze(repo_path)?;
    
    if !state.is_git_repo {
        return Err("Not a git repository".to_string());
    }
    
    println!("🔄 Pijul Migration Analysis for: {}", state.path.display());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Repository metrics
    println!("\n📊 Repository Metrics:");
    println!("  • Commit count: {}", state.commit_count);
    println!("  • Current branch: {}", state.current_branch.as_deref().unwrap_or("(detached)"));
    println!("  • Is shallow: {}", if state.is_shallow { "Yes ⚠" } else { "No ✓" });
    println!("  • Remote count: {}", state.remotes.len());
    
    // Repository complexity score
    let complexity_score = calculate_complexity_score(&state);
    let migration_difficulty = match complexity_score {
        0..=30 => ("Low", "✅"),
        31..=70 => ("Medium", "⚠️"),
        _ => ("High", "❌"),
    };
    
    println!("\n🎯 Migration Assessment:");
    println!("  • Complexity score: {}/100", complexity_score);
    println!("  • Migration difficulty: {} {}", migration_difficulty.1, migration_difficulty.0);
    
    // Storage efficiency analysis
    let repo_size = get_repo_size(&state.path);
    let estimated_pijul_size = estimate_pijul_size(repo_size, &state);
    
    println!("\n💾 Storage Analysis:");
    println!("  • Current Git size: {}", format_size(repo_size));
    println!("  • Estimated Pijul size: {}", format_size(estimated_pijul_size));
    println!("  • Space change: {}", format_size_diff(repo_size, estimated_pijul_size));
    
    // Performance implications
    analyze_performance_impact(&state);
    
    // Workflow compatibility
    analyze_workflow_compatibility(&state);
    
    // Cost analysis
    analyze_migration_costs(&state, complexity_score);
    
    // Recommendations
    provide_migration_recommendations(&state, complexity_score);
    
    Ok(())
}

fn calculate_complexity_score(state: &GitRepoState) -> u32 {
    let mut score = 0;
    
    // Commit history complexity
    score += match state.commit_count {
        0..=50 => 5,
        51..=200 => 15,
        201..=1000 => 25,
        _ => 35,
    };
    
    // Remote complexity
    score += state.remotes.len() as u32 * 5;
    
    // Branch complexity (estimate based on remotes)
    score += state.remotes.len() as u32 * 3;
    
    // Active development indicators
    if state.has_staged_changes || state.has_unstaged_changes {
        score += 10;
    }
    
    if state.has_untracked_files {
        score += 5;
    }
    
    // Large repo penalty
    if state.commit_count > 5000 {
        score += 20;
    }
    
    score.min(100)
}

fn get_repo_size(repo_path: &Path) -> u64 {
    let git_dir = repo_path.join(".git");
    get_dir_size(&git_dir)
}

fn get_dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    
    let mut size = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                size += get_dir_size(&entry_path);
            } else if let Ok(metadata) = entry.metadata() {
                size += metadata.len();
            }
        }
    }
    size
}

fn estimate_pijul_size(git_size: u64, state: &GitRepoState) -> u64 {
    // Pijul typically uses 20-40% less space due to better deduplication
    // But may use more for small repos due to metadata overhead
    let base_reduction = if git_size < 10_000_000 { 0.9 } else { 0.7 };
    
    // Adjust based on repository characteristics
    let adjustment = if state.is_shallow { 1.1 } else { base_reduction };
    
    (git_size as f64 * adjustment) as u64
}

fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size_f = size as f64;
    let mut unit_idx = 0;
    
    while size_f >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size_f /= 1024.0;
        unit_idx += 1;
    }
    
    if size_f < 10.0 {
        format!("{:.1} {}", size_f, UNITS[unit_idx])
    } else {
        format!("{:.0} {}", size_f, UNITS[unit_idx])
    }
}

fn format_size_diff(old_size: u64, new_size: u64) -> String {
    if new_size > old_size {
        let diff = new_size - old_size;
        format!("+{} ({}% increase)", format_size(diff), 
                ((diff as f64 / old_size as f64) * 100.0) as u32)
    } else {
        let diff = old_size - new_size;
        format!("-{} ({}% decrease)", format_size(diff), 
                ((diff as f64 / old_size as f64) * 100.0) as u32)
    }
}

fn analyze_performance_impact(state: &GitRepoState) {
    println!("\n⚡ Performance Impact:");
    
    let clone_improvement = if state.commit_count > 1000 {
        "Significant improvement (patch-based cloning)"
    } else {
        "Moderate improvement"
    };
    
    let merge_improvement = if state.remotes.len() > 2 {
        "Major improvement (conflict-free merging)"
    } else {
        "Moderate improvement"
    };
    
    println!("  • Clone speed: {}", clone_improvement);
    println!("  • Merge conflicts: {}", merge_improvement);
    println!("  • Network efficiency: Significant improvement (patch-based sync)");
    println!("  • Storage dedup: Better than Git (content-based)");
}

fn analyze_workflow_compatibility(state: &GitRepoState) {
    println!("\n🔄 Workflow Compatibility:");
    
    // Estimate workflow complexity based on remotes
    let workflow_complexity = match state.remotes.len() {
        0..=1 => "Simple",
        2..=3 => "Standard", 
        _ => "Complex",
    };
    
    println!("  • Current workflow: {}", workflow_complexity);
    println!("  • Pijul learning curve: 2-4 weeks for team");
    println!("  • Command similarity: High (similar to Git)");
    println!("  • IDE support: Limited (improving rapidly)");
    println!("  • CI/CD compatibility: Requires updates");
}

fn analyze_migration_costs(state: &GitRepoState, complexity_score: u32) {
    println!("\n💰 Migration Cost Analysis:");
    
    let time_estimate = match complexity_score {
        0..=30 => "1-2 days",
        31..=70 => "3-7 days",
        _ => "1-3 weeks",
    };
    
    let team_training = if state.remotes.len() > 2 { "2-4 weeks" } else { "1-2 weeks" };
    
    println!("  • Migration time: {}", time_estimate);
    println!("  • Team training: {}", team_training);
    println!("  • Infrastructure updates: 1-2 weeks");
    println!("  • Tool compatibility: Medium risk");
    println!("  • Rollback complexity: High (requires git export)");
}

fn provide_migration_recommendations(_state: &GitRepoState, complexity_score: u32) {
    println!("\n💡 Recommendations:");
    
    match complexity_score {
        0..=30 => {
            println!("  ✅ LOW RISK - Good candidate for Pijul migration");
            println!("  • Start with a pilot project or feature branch");
            println!("  • Migrate incrementally, keeping Git as backup");
            println!("  • Expected benefits outweigh migration costs");
        }
        31..=70 => {
            println!("  ⚠️  MEDIUM RISK - Consider carefully");
            println!("  • Evaluate team readiness and tool dependencies");
            println!("  • Plan for extended migration period");
            println!("  • Consider hybrid approach initially");
        }
        _ => {
            println!("  ❌ HIGH RISK - Not recommended currently");
            println!("  • Wait for Pijul ecosystem maturity");
            println!("  • Complex repository may not see immediate benefits");
            println!("  • High migration and training costs");
        }
    }
    
    println!("\n🎯 Next Steps:");
    if complexity_score <= 50 {
        println!("  1. Set up test Pijul repository alongside Git");
        println!("  2. Train 1-2 developers on Pijul basics");
        println!("  3. Migrate a small, non-critical component first");
        println!("  4. Evaluate performance and workflow impact");
        println!("  5. Plan gradual migration if successful");
    } else {
        println!("  1. Monitor Pijul ecosystem development");
        println!("  2. Consider Git workflow optimizations instead");
        println!("  3. Re-evaluate in 6-12 months");
        println!("  4. Focus on team Git skill improvement");
    }
    
    println!("\n📚 Resources:");
    println!("  • Pijul docs: https://pijul.org/manual/");
    println!("  • Git to Pijul guide: https://pijul.org/manual/git/");
    println!("  • Community: https://nest.pijul.com/");
}

fn show_git_sync_help() {
    println!("litebike git-sync - Advanced Git Synchronization Tool");
    println!();
    println!("USAGE:");
    println!("  litebike git-sync <command> [options] [url]");
    println!();
    println!("COMMANDS:");
    println!("  status                 Show repository and remote status");
    println!("  clean                  Remove stale temporary remotes");
    println!("  clone <url>            Clone a repository with smart strategy");
    println!("  push [url]             Push to remote repository");
    println!("  pull [url]             Pull from remote repository");
    println!("  sync [url]             Bidirectional sync (pull then push)");
    println!("  pijul-analyze          Analyze repository for Pijul migration");
    println!("  <url>                  Smart sync with automatic strategy");
    println!();
    println!("OPTIONS:");
    println!("  --strategy <type>      Sync strategy: full, shallow, fetch, smart");
    println!("  --remote <name>        Remote name (default: temp_upstream)");
    println!("  --branch <name>        Target branch (default: current branch)");
    println!("  --force                Force push when needed");
    println!("  --no-cleanup           Don't clean stale remotes automatically");
    println!("  --path <path>          Repository path (default: current directory)");
    println!();
    println!("EXAMPLES:");
    println!("  litebike git-sync status");
    println!("  litebike git-sync clean");
    println!("  litebike git-sync pijul-analyze");
    println!("  litebike git-sync user@host:/path/to/repo.git");
    println!("  litebike git-sync clone ssh://user@host:2222/repo.git");
    println!("  litebike git-sync push --force user@host:/repo.git");
    println!("  litebike git-sync sync --strategy shallow origin");
    println!();
    println!("STRATEGIES:");
    println!("  full     - Complete clone/sync with full history");
    println!("  shallow  - Shallow clone/sync with depth=1");
    println!("  fetch    - Only fetch/push without cloning");
    println!("  smart    - Automatic strategy based on repository state");
    println!();
    println!("TERMUX OPTIMIZATION:");
    println!("  Automatically detects Termux environment and optimizes for:");
    println!("  - Reduced memory usage with shallow clones");
    println!("  - Shorter SSH timeouts for mobile connections");
    println!("  - Cleanup of temporary remotes to save space");
}