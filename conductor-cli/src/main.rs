//! Conductor CLI - Development Track Management for Literbike
//!
//! A command-line tool for managing development tracks, tasks, and progress
//! in the Literbike project.

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};
use colored::Colorize;
use git2::{Repository, ObjectType};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Conductor - Development Track Management CLI
#[derive(Parser)]
#[command(name = "conductor")]
#[command(author = "Literbike Team")]
#[command(version = "0.1.0")]
#[command(about = "Development track management for Literbike", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the conductor directory
    #[arg(long, default_value = "conductor")]
    conductor_path: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// List all development tracks
    List {
        /// Filter by status (pending, in_progress, complete)
        #[arg(short, long)]
        status: Option<String>,

        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Show detailed information about a specific track
    Show {
        /// Track ID or name
        track: String,
    },

    /// Show overall progress across all tracks
    Status {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Track progress visualization
    Progress {
        /// Track ID to show progress for
        track: Option<String>,
    },

    /// Initialize a new track
    Init {
        /// Track name (e.g., "my-feature_20260309")
        name: String,

        /// Track title
        #[arg(short, long)]
        title: Option<String>,
    },

    /// Update task status in a track
    Update {
        /// Track ID
        track: String,

        /// Task pattern to match
        task: String,

        /// New status (pending, in_progress, complete)
        #[arg(short, long)]
        status: String,
    },

    /// Validate track structure and content
    Validate {
        /// Track ID to validate (or all if not specified)
        track: Option<String>,
    },

    /// Generate summary report
    Report {
        /// Output format (markdown, json)
        #[arg(short, long, default_value = "markdown")]
        format: String,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show git integration status
    Git {
        /// Show commits for a specific track's files
        #[arg(short, long)]
        track: Option<String>,
    },
}

/// Represents a development track
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Track {
    id: String,
    name: String,
    title: String,
    status: TrackStatus,
    created_date: String,
    plan_path: PathBuf,
    spec_path: PathBuf,
    tasks: Vec<Task>,
    metadata: TrackMetadata,
}

/// Track status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum TrackStatus {
    Pending,
    InProgress,
    Complete,
    Blocked,
    Archived,
}

impl std::fmt::Display for TrackStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackStatus::Pending => write!(f, "Pending"),
            TrackStatus::InProgress => write!(f, "In Progress"),
            TrackStatus::Complete => write!(f, "Complete"),
            TrackStatus::Blocked => write!(f, "Blocked"),
            TrackStatus::Archived => write!(f, "Archived"),
        }
    }
}

/// Individual task within a track
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: String,
    description: String,
    status: TaskStatus,
    commit_sha: Option<String>,
    notes: Option<String>,
}

/// Task status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum TaskStatus {
    Pending,
    InProgress,
    Complete,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "[ ]"),
            TaskStatus::InProgress => write!(f, "[~]"),
            TaskStatus::Complete => write!(f, "[x]"),
        }
    }
}

/// Track metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackMetadata {
    owner: Option<String>,
    priority: Option<String>,
    estimated_hours: Option<f32>,
    actual_hours: Option<f32>,
    dependencies: Vec<String>,
    tags: Vec<String>,
}

impl Default for TrackMetadata {
    fn default() -> Self {
        Self {
            owner: None,
            priority: None,
            estimated_hours: None,
            actual_hours: None,
            dependencies: Vec::new(),
            tags: Vec::new(),
        }
    }
}

/// Git integration for commit tracking
struct GitIntegration {
    repo: Option<Repository>,
}

impl GitIntegration {
    fn new(project_root: &Path) -> Result<Self> {
        let repo = Repository::discover(project_root)
            .ok()
            .or_else(|| {
                // Try current directory as fallback
                Repository::discover(".").ok()
            });

        Ok(Self { repo })
    }

    /// Get commit information for a SHA
    fn get_commit(&self, sha: &str) -> Option<CommitInfo> {
        let repo = self.repo.as_ref()?;
        let oid = repo.revparse_single(sha).ok()?.id();
        let obj = repo.find_object(oid, Some(ObjectType::Commit)).ok()?;
        let commit = obj.into_commit().ok()?;

        Some(CommitInfo {
            sha: commit.id().to_string()[..7].to_string(),
            message: commit.message().unwrap_or("").to_string(),
            date: commit.time().seconds(),
        })
    }

    /// Get recent commits for a file path
    fn get_file_commits(&self, file_path: &Path, limit: usize) -> Vec<CommitInfo> {
        let mut commits = Vec::new();
        
        if let Some(repo) = &self.repo {
            if let Ok(mut revwalk) = repo.revwalk() {
                if revwalk.push_head().is_ok() {
                    revwalk.set_sorting(git2::Sort::TIME).ok();

                    for oid in revwalk.take(limit) {
                        if let Ok(oid) = oid {
                            if let Ok(obj) = repo.find_object(oid, None) {
                                if let Ok(commit) = obj.into_commit() {
                                    // Check if commit touches the file
                                    if let Ok(parent) = commit.parent(0) {
                                        if let Ok(diff) = repo.diff_tree_to_tree(
                                            parent.tree().ok().as_ref(),
                                            commit.tree().ok().as_ref(),
                                            None,
                                        ) {
                                            let mut touches_file = false;
                                            diff.foreach(
                                                &mut |delta, _| {
                                                    if let Some(path) = delta.new_file().path() {
                                                        if path == file_path {
                                                            touches_file = true;
                                                            false // stop iteration
                                                        } else {
                                                            true // continue
                                                        }
                                                    } else {
                                                        true
                                                    }
                                                },
                                                None,
                                                None,
                                                None,
                                            ).ok();

                                            if touches_file {
                                                commits.push(CommitInfo {
                                                    sha: commit.id().to_string()[..7].to_string(),
                                                    message: commit.message().unwrap_or("").to_string(),
                                                    date: commit.time().seconds(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        commits
    }

    /// Get the most recent commit
    fn get_latest_commit(&self) -> Option<CommitInfo> {
        let repo = self.repo.as_ref()?;
        let head = repo.head().ok()?;
        let oid = head.target()?;
        let obj = repo.find_object(oid, None).ok()?;
        let commit = obj.into_commit().ok()?;

        Some(CommitInfo {
            sha: commit.id().to_string()[..7].to_string(),
            message: commit.message().unwrap_or("").to_string(),
            date: commit.time().seconds(),
        })
    }

    /// Get current branch name
    fn get_current_branch(&self) -> Option<String> {
        let repo = self.repo.as_ref()?;
        let head = repo.head().ok()?;
        let branch_name = head.shorthand()?.to_string();
        Some(branch_name)
    }

    /// Check if working directory is clean
    fn is_workdir_clean(&self) -> bool {
        if let Some(repo) = &self.repo {
            repo.statuses(None).map_or(false, |statuses| {
                statuses.is_empty() || statuses.iter().all(|s| {
                    s.status().is_empty() || s.status().is_ignored()
                })
            })
        } else {
            false
        }
    }
}

/// Commit information
#[derive(Debug, Clone)]
struct CommitInfo {
    sha: String,
    message: String,
    date: i64,
}

impl CommitInfo {
    fn formatted_date(&self) -> String {
        let datetime = DateTime::from_timestamp(self.date, 0)
            .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap());
        datetime.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// Track manager for loading and managing tracks
struct TrackManager {
    #[allow(dead_code)]
    conductor_path: PathBuf,
    tracks_path: PathBuf,
    git: GitIntegration,
}

impl TrackManager {
    fn new(conductor_path: PathBuf) -> Result<Self> {
        let tracks_path = conductor_path.join("tracks");
        
        // Find project root (parent of conductor directory)
        let project_root = conductor_path.parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        
        let git = GitIntegration::new(&project_root)?;
        
        if !tracks_path.exists() {
            anyhow::bail!(
                "Tracks directory not found at {}. Run 'conductor init' first.",
                tracks_path.display()
            );
        }

        Ok(Self {
            conductor_path,
            tracks_path,
            git,
        })
    }

    /// Discover all tracks in the tracks directory
    fn discover_tracks(&self) -> Result<Vec<Track>> {
        let mut tracks = Vec::new();

        for entry in fs::read_dir(&self.tracks_path)
            .with_context(|| format!("Failed to read tracks directory"))?
        {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Ok(track) = self.load_track(&path) {
                    tracks.push(track);
                }
            }
        }

        // Sort by date (newest first)
        tracks.sort_by(|a, b| b.created_date.cmp(&a.created_date));

        Ok(tracks)
    }

    /// Load a single track from its directory
    fn load_track(&self, track_dir: &Path) -> Result<Track> {
        let track_id = track_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid track directory name"))?
            .to_string();

        // Extract date from track ID (format: name_YYYYMMDD)
        let date_regex = Regex::new(r"_([0-9]{8})$")?;
        let created_date = date_regex
            .captures(&track_id)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Load plan.md
        let plan_path = track_dir.join("plan.md");
        let plan_content = fs::read_to_string(&plan_path).unwrap_or_default();

        // Parse tasks from plan.md
        let tasks = self.parse_tasks(&plan_content);

        // Load spec.md
        let spec_path = track_dir.join("spec.md");

        // Extract title from spec.md or use track ID
        let title = self.extract_title(&track_dir.join("spec.md"))
            .unwrap_or_else(|| track_id.clone());

        // Determine track status from tasks
        let status = self.determine_track_status(&tasks);

        // Load metadata.json if it exists
        let metadata_path = track_dir.join("metadata.json");
        let metadata = if metadata_path.exists() {
            let content = fs::read_to_string(&metadata_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            TrackMetadata::default()
        };

        Ok(Track {
            id: track_id.clone(),
            name: track_id.clone(),
            title,
            status,
            created_date,
            plan_path,
            spec_path,
            tasks,
            metadata,
        })
    }

    /// Parse tasks from plan.md content
    fn parse_tasks(&self, content: &str) -> Vec<Task> {
        let mut tasks = Vec::new();
        let task_regex = Regex::new(r"(?m)^[-*]\s+\[([ ~x])\]\s+(.+?)(?:\s+\(([^)]+)\))?(?:\s+-\s+(.+))?$").unwrap();

        for (i, cap) in task_regex.captures_iter(content).enumerate() {
            let status_char = cap.get(1).map(|m| m.as_str()).unwrap_or(" ");
            let description = cap.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
            
            let status = match status_char {
                " " => TaskStatus::Pending,
                "~" => TaskStatus::InProgress,
                "x" => TaskStatus::Complete,
                _ => TaskStatus::Pending,
            };

            // Extract commit SHA if present in description
            let commit_sha = cap.get(3)
                .map(|m| m.as_str())
                .and_then(|s| {
                    if s.len() == 7 && s.chars().all(|c| c.is_ascii_hexdigit()) {
                        Some(s.to_string())
                    } else {
                        None
                    }
                });

            tasks.push(Task {
                id: format!("task-{}", i + 1),
                description,
                status,
                commit_sha,
                notes: cap.get(4).map(|m| m.as_str()).map(String::from),
            });
        }

        tasks
    }

    /// Extract title from spec.md
    fn extract_title(&self, spec_path: &Path) -> Option<String> {
        if !spec_path.exists() {
            return None;
        }

        let content = fs::read_to_string(spec_path).ok()?;
        
        // Look for first heading
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                return Some(line.trim_start_matches('#').trim().to_string());
            }
        }

        None
    }

    /// Determine overall track status from tasks
    fn determine_track_status(&self, tasks: &[Task]) -> TrackStatus {
        if tasks.is_empty() {
            return TrackStatus::Pending;
        }

        let total = tasks.len() as f32;
        let complete = tasks.iter().filter(|t| t.status == TaskStatus::Complete).count() as f32;
        let in_progress = tasks.iter().filter(|t| t.status == TaskStatus::InProgress).count();

        let completion_pct = (complete / total) * 100.0;

        if completion_pct == 100.0 {
            TrackStatus::Complete
        } else if in_progress > 0 {
            TrackStatus::InProgress
        } else if completion_pct > 0.0 {
            TrackStatus::InProgress
        } else {
            TrackStatus::Pending
        }
    }

    /// Get a specific track by ID
    fn get_track(&self, track_id: &str) -> Result<Track> {
        let tracks = self.discover_tracks()?;
        
        tracks.into_iter()
            .find(|t| t.id == track_id || t.name == track_id)
            .ok_or_else(|| anyhow::anyhow!("Track '{}' not found", track_id))
    }

    /// Calculate overall progress
    fn calculate_progress(&self) -> Result<ProgressStats> {
        let tracks = self.discover_tracks()?;
        
        let total_tracks = tracks.len();
        let complete_tracks = tracks.iter().filter(|t| t.status == TrackStatus::Complete).count();
        let in_progress_tracks = tracks.iter().filter(|t| t.status == TrackStatus::InProgress).count();
        
        let total_tasks: usize = tracks.iter().map(|t| t.tasks.len()).sum();
        let complete_tasks: usize = tracks.iter()
            .flat_map(|t| &t.tasks)
            .filter(|t| t.status == TaskStatus::Complete)
            .count();

        Ok(ProgressStats {
            total_tracks,
            complete_tracks,
            in_progress_tracks,
            pending_tracks: total_tracks - complete_tracks - in_progress_tracks,
            total_tasks,
            complete_tasks,
            tracks,
        })
    }
}

/// Progress statistics
#[derive(Debug)]
struct ProgressStats {
    total_tracks: usize,
    complete_tracks: usize,
    in_progress_tracks: usize,
    pending_tracks: usize,
    total_tasks: usize,
    complete_tasks: usize,
    tracks: Vec<Track>,
}

impl ProgressStats {
    fn track_completion_percentage(&self) -> f32 {
        if self.total_tracks == 0 {
            return 0.0;
        }
        (self.complete_tracks as f32 / self.total_tracks as f32) * 100.0
    }

    fn task_completion_percentage(&self) -> f32 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        (self.complete_tasks as f32 / self.total_tasks as f32) * 100.0
    }
}

/// CLI command handlers
impl Cli {
    fn run(&self) -> Result<()> {
        let manager = TrackManager::new(self.conductor_path.clone())?;

        match &self.command {
            Commands::List { status, detailed } => {
                self.cmd_list(&manager, status.as_ref(), *detailed)
            }
            Commands::Show { track } => self.cmd_show(&manager, track),
            Commands::Status { format } => self.cmd_status(&manager, format),
            Commands::Progress { track } => self.cmd_progress(&manager, track.as_ref()),
            Commands::Init { name, title } => self.cmd_init(name, title.as_ref()),
            Commands::Update { track, task, status } => {
                self.cmd_update(&manager, track, task, status)
            }
            Commands::Validate { track } => self.cmd_validate(&manager, track.as_ref()),
            Commands::Report { format, output } => self.cmd_report(&manager, format, output.as_ref()),
            Commands::Git { track } => self.cmd_git(&manager, track.as_ref()),
        }
    }

    /// List all tracks
    fn cmd_list(&self, manager: &TrackManager, status_filter: Option<&String>, detailed: bool) -> Result<()> {
        let tracks = manager.discover_tracks()?;

        if tracks.is_empty() {
            println!("{}", "No tracks found. Use 'conductor init' to create a new track.".yellow());
            return Ok(());
        }

        println!("\n{}", "Development Tracks".bold().blue());
        println!("{}", "=".repeat(60));

        for track in &tracks {
            // Apply status filter
            if let Some(filter) = status_filter {
                let filter_status = filter.to_lowercase();
                let track_status = match track.status {
                    TrackStatus::Pending => "pending",
                    TrackStatus::InProgress => "in_progress",
                    TrackStatus::Complete => "complete",
                    TrackStatus::Blocked => "blocked",
                    TrackStatus::Archived => "archived",
                };
                
                if track_status != filter_status {
                    continue;
                }
            }

            // Format status with color
            let status_str = match track.status {
                TrackStatus::Complete => format!("{}", "✓".green()),
                TrackStatus::InProgress => format!("{}", "◐".yellow()),
                TrackStatus::Pending => format!("{}", "○".white()),
                TrackStatus::Blocked => format!("{}", "⊘".red()),
                TrackStatus::Archived => format!("{}", "⊗".dimmed()),
            };

            // Calculate task progress
            let total = track.tasks.len();
            let complete = track.tasks.iter().filter(|t| t.status == TaskStatus::Complete).count();
            let progress = if total > 0 {
                format!("{}/{}", complete, total)
            } else {
                "0/0".to_string()
            };

            println!("\n{} {} {}", status_str, track.id.bold(), progress.dimmed());
            println!("   {}", track.title);

            if detailed {
                println!("   Created: {}", track.created_date);
                
                if let Some(owner) = &track.metadata.owner {
                    println!("   Owner: {}", owner);
                }
                
                if let Some(priority) = &track.metadata.priority {
                    println!("   Priority: {}", priority);
                }

                if !track.tasks.is_empty() {
                    println!("   Tasks:");
                    for task in &track.tasks {
                        println!("     {} {}", task.status, task.description);
                    }
                }
            }
        }

        println!("\n{}", "=".repeat(60));
        println!("Total: {} tracks", tracks.len());

        Ok(())
    }

    /// Show detailed track information
    fn cmd_show(&self, manager: &TrackManager, track_id: &str) -> Result<()> {
        let track = manager.get_track(track_id)?;

        println!("\n{}", track.id.bold().blue());
        println!("{}", "=".repeat(60));
        println!("Title: {}", track.title);
        println!("Status: {}", track.status);
        println!("Created: {}", track.created_date);

        if let Some(owner) = &track.metadata.owner {
            println!("Owner: {}", owner);
        }

        if let Some(priority) = &track.metadata.priority {
            println!("Priority: {}", priority);
        }

        // Task breakdown
        let total = track.tasks.len();
        let complete = track.tasks.iter().filter(|t| t.status == TaskStatus::Complete).count();
        let in_progress = track.tasks.iter().filter(|t| t.status == TaskStatus::InProgress).count();
        let pending = total - complete - in_progress;

        println!("\nTasks: {}/{} complete, {} in progress, {} pending", 
                 complete, total, in_progress, pending);

        if !track.tasks.is_empty() {
            println!("\nTask List:");
            for task in &track.tasks {
                let status_icon = match task.status {
                    TaskStatus::Complete => "✓".green(),
                    TaskStatus::InProgress => "◐".yellow(),
                    TaskStatus::Pending => "○".white(),
                };

                println!("  {} {}", status_icon, task.description);
                
                if let Some(sha) = &task.commit_sha {
                    println!("      Commit: {}", sha.dimmed());
                }
            }
        }

        // File paths
        println!("\nFiles:");
        println!("  Plan: {}", track.plan_path.display());
        println!("  Spec: {}", track.spec_path.display());

        if !track.metadata.dependencies.is_empty() {
            println!("\nDependencies:");
            for dep in &track.metadata.dependencies {
                println!("  - {}", dep);
            }
        }

        if !track.metadata.tags.is_empty() {
            println!("\nTags: {}", track.metadata.tags.join(", "));
        }

        println!();

        Ok(())
    }

    /// Show overall status
    fn cmd_status(&self, manager: &TrackManager, format: &str) -> Result<()> {
        let stats = manager.calculate_progress()?;

        match format.to_lowercase().as_str() {
            "json" => {
                let json = serde_json::json!({
                    "tracks": {
                        "total": stats.total_tracks,
                        "complete": stats.complete_tracks,
                        "in_progress": stats.in_progress_tracks,
                        "pending": stats.pending_tracks,
                        "completion_percentage": stats.track_completion_percentage()
                    },
                    "tasks": {
                        "total": stats.total_tasks,
                        "complete": stats.complete_tasks,
                        "completion_percentage": stats.task_completion_percentage()
                    }
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            _ => {
                println!("\n{}", "Literbike Development Status".bold().blue());
                println!("{}", "=".repeat(60));

                // Track progress bar
                let track_pct = stats.track_completion_percentage();
                println!("\nTracks: {}/{} complete ({:.1}%)", 
                         stats.complete_tracks, stats.total_tracks, track_pct);
                self.print_progress_bar(track_pct, 40);

                // Task progress bar
                let task_pct = stats.task_completion_percentage();
                println!("\nTasks: {}/{} complete ({:.1}%)", 
                         stats.complete_tasks, stats.total_tasks, task_pct);
                self.print_progress_bar(task_pct, 40);

                // Breakdown
                println!("\nBreakdown:");
                println!("  {} Complete", stats.complete_tracks.to_string().green());
                println!("  {} In Progress", stats.in_progress_tracks.to_string().yellow());
                println!("  {} Pending", stats.pending_tracks.to_string().white());

                println!("\n{}", "=".repeat(60));
            }
        }

        Ok(())
    }

    /// Show progress visualization
    fn cmd_progress(&self, manager: &TrackManager, track_id: Option<&String>) -> Result<()> {
        if let Some(id) = track_id {
            // Show progress for specific track
            let track = manager.get_track(id)?;
            
            println!("\n{}", track.id.bold());
            println!("{}", "=".repeat(60));

            let total = track.tasks.len();
            let complete = track.tasks.iter().filter(|t| t.status == TaskStatus::Complete).count();
            let pct = if total > 0 {
                (complete as f32 / total as f32) * 100.0
            } else {
                0.0
            };

            println!("Progress: {}/{} tasks", complete, total);
            self.print_progress_bar(pct, 50);
            println!("{:.1}% complete", pct);

        } else {
            // Show progress for all tracks
            let stats = manager.calculate_progress()?;

            println!("\n{}", "Overall Progress".bold().blue());
            println!("{}", "=".repeat(60));

            for track in &stats.tracks {
                let total = track.tasks.len();
                let complete = track.tasks.iter().filter(|t| t.status == TaskStatus::Complete).count();
                let pct = if total > 0 {
                    (complete as f32 / total as f32) * 100.0
                } else {
                    0.0
                };

                let status_icon = match track.status {
                    TrackStatus::Complete => "✓".green(),
                    TrackStatus::InProgress => "◐".yellow(),
                    TrackStatus::Pending => "○".white(),
                    _ => "•".dimmed(),
                };

                println!("\n{} {} ({:.0}%)", status_icon, track.id, pct);
                self.print_progress_bar(pct, 40);
            }

            println!("\n{}", "=".repeat(60));
        }

        Ok(())
    }

    /// Initialize a new track
    fn cmd_init(&self, name: &str, title: Option<&String>) -> Result<()> {
        let track_dir = self.conductor_path.join("tracks").join(name);

        if track_dir.exists() {
            anyhow::bail!("Track '{}' already exists", name);
        }

        // Create progress bar
        let pb = ProgressBar::new(4);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")?
            .progress_chars("=>-"));

        pb.set_message("Creating track directory");
        fs::create_dir_all(&track_dir)?;
        pb.inc(1);

        pb.set_message("Creating spec.md");
        let spec_content = self.generate_spec_template(name, title);
        fs::write(track_dir.join("spec.md"), spec_content)?;
        pb.inc(1);

        pb.set_message("Creating plan.md");
        let plan_content = self.generate_plan_template();
        fs::write(track_dir.join("plan.md"), plan_content)?;
        pb.inc(1);

        pb.set_message("Creating metadata.json");
        let metadata = TrackMetadata::default();
        fs::write(
            track_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?,
        )?;
        pb.inc(1);

        pb.finish_with_message("Track initialized!");

        println!("\n{} Track '{}' created successfully!", "✓".green(), name.bold());
        println!("  Location: {}", track_dir.display());
        println!("\nNext steps:");
        println!("  1. Edit spec.md to define the track scope");
        println!("  2. Edit plan.md to add tasks");
        println!("  3. Run 'conductor show {}' to view progress", name);

        Ok(())
    }

    /// Update task status
    fn cmd_update(&self, manager: &TrackManager, track_id: &str, task_pattern: &str, status: &str) -> Result<()> {
        let track = manager.get_track(track_id)?;
        let plan_content = fs::read_to_string(&track.plan_path)
            .with_context(|| format!("Failed to read plan.md"))?;

        // Parse status
        let status_char = match status.to_lowercase().as_str() {
            "pending" => " ",
            "in_progress" | "progress" => "~",
            "complete" | "done" => "x",
            _ => anyhow::bail!("Invalid status '{}'. Use: pending, in_progress, or complete", status),
        };

        // Update matching tasks
        let task_regex = Regex::new(&regex::escape(task_pattern))?;
        let mut updated = 0;

        let updated_content = plan_content
            .lines()
            .map(|line| {
                if line.contains(&['[', ']', '~', 'x']) && task_regex.is_match(line) {
                    updated += 1;
                    // Replace status marker
                    let new_line = Regex::new(r"\[([ ~x])\]")
                        .unwrap()
                        .replace(line, format!("[{}]", status_char));
                    new_line.to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if updated == 0 {
            println!("{} No tasks matched pattern '{}'", "⚠".yellow(), task_pattern);
            return Ok(());
        }

        // Write updated content
        fs::write(&track.plan_path, updated_content)?;

        println!("{} Updated {} task(s) in track '{}'", "✓".green(), updated, track_id);

        Ok(())
    }

    /// Validate track structure
    fn cmd_validate(&self, manager: &TrackManager, track_id: Option<&String>) -> Result<()> {
        let tracks = if let Some(id) = track_id {
            vec![manager.get_track(id)?]
        } else {
            manager.discover_tracks()?
        };

        let mut all_valid = true;

        for track in &tracks {
            println!("\nValidating: {}", track.id.bold());
            
            let mut errors = Vec::new();
            let mut warnings = Vec::<String>::new();

            // Check required files
            if !track.plan_path.exists() {
                errors.push("Missing plan.md".to_string());
            }

            if !track.spec_path.exists() {
                errors.push("Missing spec.md".to_string());
            }

            // Check task structure
            if track.tasks.is_empty() {
                warnings.push("No tasks defined in plan.md".to_string());
            }

            // Check for commit SHAs in complete tasks
            let complete_without_sha: Vec<_> = track.tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Complete && t.commit_sha.is_none())
                .collect();

            if !complete_without_sha.is_empty() {
                warnings.push(format!(
                    "{} complete task(s) missing commit SHA",
                    complete_without_sha.len()
                ));
            }

            // Report results
            if errors.is_empty() && warnings.is_empty() {
                println!("  {} Valid", "✓".green());
            } else {
                all_valid = false;

                for error in &errors {
                    println!("  {} {}", "✗".red(), error);
                }

                for warning in &warnings {
                    println!("  {} {}", "⚠".yellow(), warning);
                }
            }
        }

        if all_valid {
            println!("\n{} All tracks valid!", "✓".green());
        } else {
            println!("\n{} Validation completed with errors", "⚠".yellow());
        }

        Ok(())
    }

    /// Generate report
    fn cmd_report(&self, manager: &TrackManager, format: &str, output: Option<&PathBuf>) -> Result<()> {
        let stats = manager.calculate_progress()?;

        let report = match format.to_lowercase().as_str() {
            "json" => {
                let json = serde_json::json!({
                    "report_date": Local::now().format("%Y-%m-%d").to_string(),
                    "summary": {
                        "total_tracks": stats.total_tracks,
                        "complete_tracks": stats.complete_tracks,
                        "in_progress_tracks": stats.in_progress_tracks,
                        "pending_tracks": stats.pending_tracks,
                        "track_completion": format!("{:.1}%", stats.track_completion_percentage()),
                        "task_completion": format!("{:.1}%", stats.task_completion_percentage())
                    },
                    "tracks": stats.tracks.iter().map(|t| {
                        serde_json::json!({
                            "id": t.id,
                            "title": t.title,
                            "status": format!("{}", t.status),
                            "tasks": {
                                "total": t.tasks.len(),
                                "complete": t.tasks.iter().filter(|t| t.status == TaskStatus::Complete).count()
                            }
                        })
                    }).collect::<Vec<_>>()
                });
                serde_json::to_string_pretty(&json)?
            }
            _ => {
                // Markdown format
                let mut md = String::new();
                md.push_str("# Literbike Development Report\n\n");
                md.push_str(&format!("**Generated:** {}\n\n", Local::now().format("%Y-%m-%d %H:%M")));
                
                md.push_str("## Summary\n\n");
                md.push_str(&format!("- **Total Tracks:** {}\n", stats.total_tracks));
                md.push_str(&format!("- **Complete:** {}\n", stats.complete_tracks.to_string().green()));
                md.push_str(&format!("- **In Progress:** {}\n", stats.in_progress_tracks.to_string().yellow()));
                md.push_str(&format!("- **Pending:** {}\n", stats.pending_tracks));
                md.push_str(&format!("- **Track Completion:** {:.1}%\n", stats.track_completion_percentage()));
                md.push_str(&format!("- **Task Completion:** {:.1}%\n\n", stats.task_completion_percentage()));

                md.push_str("## Tracks\n\n");
                md.push_str("| Track | Status | Tasks | Progress |\n");
                md.push_str("|-------|--------|-------|----------|\n");

                for track in &stats.tracks {
                    let total = track.tasks.len();
                    let complete = track.tasks.iter().filter(|t| t.status == TaskStatus::Complete).count();
                    let pct = if total > 0 {
                        format!("{:.0}%", (complete as f32 / total as f32) * 100.0)
                    } else {
                        "0%".to_string()
                    };

                    md.push_str(&format!(
                        "| {} | {} | {}/{} | {} |\n",
                        track.id,
                        format!("{}", track.status),
                        complete,
                        total,
                        pct
                    ));
                }

                md
            }
        };

        // Output to file or stdout
        if let Some(output_path) = output {
            fs::write(output_path, &report)?;
            println!("{} Report written to {}", "✓".green(), output_path.display());
        } else {
            println!("{}", report);
        }

        Ok(())
    }

    /// Show git integration status
    fn cmd_git(&self, manager: &TrackManager, track_id: Option<&String>) -> Result<()> {
        println!("\n{}", "Git Integration Status".bold().blue());
        println!("{}", "=".repeat(60));

        // Show current branch
        if let Some(branch) = manager.git.get_current_branch() {
            println!("\nBranch: {}", branch.bold());
        }

        // Show working directory status
        let is_clean = manager.git.is_workdir_clean();
        if is_clean {
            println!("Working Directory: {}", "Clean".green());
        } else {
            println!("Working Directory: {}", "Has uncommitted changes".yellow());
        }

        // Show latest commit
        if let Some(commit) = manager.git.get_latest_commit() {
            println!("\nLatest Commit:");
            println!("  SHA: {}", commit.sha.dimmed());
            println!("  Date: {}", commit.formatted_date());
            println!("  Message: {}", commit.message);
        }

        // If track specified, show commits for track files
        if let Some(id) = track_id {
            let track = manager.get_track(id)?;
            
            println!("\nTrack: {}", track.id.bold());
            
            // Get commits for plan.md
            let plan_commits = manager.git.get_file_commits(&track.plan_path, 5);
            if !plan_commits.is_empty() {
                println!("\nRecent commits to plan.md:");
                for commit in &plan_commits {
                    println!("  {} {} ({})", commit.sha.dimmed(), commit.message, commit.formatted_date());
                }
            }

            // Get commits for spec.md
            let spec_commits = manager.git.get_file_commits(&track.spec_path, 5);
            if !spec_commits.is_empty() {
                println!("\nRecent commits to spec.md:");
                for commit in &spec_commits {
                    println!("  {} {} ({})", commit.sha.dimmed(), commit.message, commit.formatted_date());
                }
            }

            // Show tasks with commit SHAs
            let tasks_with_commits: Vec<_> = track.tasks
                .iter()
                .filter(|t| t.commit_sha.is_some())
                .collect();

            if !tasks_with_commits.is_empty() {
                println!("\nTasks with commits:");
                for task in &tasks_with_commits {
                    if let Some(sha) = &task.commit_sha {
                        // Try to get commit details
                        let commit_info = manager.git.get_commit(sha);
                        
                        if let Some(info) = commit_info {
                            println!("  {} {}", task.status, task.description);
                            println!("    → {} {} ({})", info.sha.dimmed(), info.message, info.formatted_date());
                        } else {
                            println!("  {} {} (commit: {})", task.status, task.description, sha.dimmed());
                        }
                    }
                }
            }
        }

        println!();
        Ok(())
    }

    /// Print a progress bar
    fn print_progress_bar(&self, percentage: f32, width: usize) {
        let filled = ((percentage / 100.0) * width as f32) as usize;
        let empty = width - filled;

        print!("[");
        for _ in 0..filled {
            print!("{}", "█".green());
        }
        for _ in 0..empty {
            print!("{}", "░".dimmed());
        }
        println!("]");
    }

    /// Generate spec.md template
    fn generate_spec_template(&self, name: &str, title: Option<&String>) -> String {
        let title_upper = name.replace('_', " ").to_uppercase();
        let title = title.unwrap_or(&title_upper);
        
        format!(
r#"# {}

**Track:** `{}`
**Status:** Pending
**Priority:** TBD

---

## Overview

<!-- Brief description of what this track aims to accomplish -->

## Goals

<!-- List of specific, measurable goals -->

1. 
2. 
3. 

## Non-Goals

<!-- Explicitly call out what is NOT in scope -->

- 

## Success Criteria

<!-- How will we know this track is complete? -->

- [ ] 
- [ ] 
- [ ] 

## Technical Approach

<!-- High-level technical strategy -->

## Dependencies

<!-- Other tracks or external dependencies -->

- 

## Risks

<!-- Potential risks and mitigation strategies -->

- 

---

**Created:** {date}
**Owner:** TBD
"#,
            title,
            name,
            date = Local::now().format("%Y-%m-%d")
        )
    }

    /// Generate plan.md template
    fn generate_plan_template(&self) -> String {
        r#"# Implementation Plan

## Phase 1: Discovery & Planning

- [ ] Research existing implementations and related code
- [ ] Define clear interfaces and boundaries
- [ ] Create detailed task breakdown

## Phase 2: Foundation

- [ ] Set up module structure and exports
- [ ] Implement core data types
- [ ] Add basic tests for foundation types

## Phase 3: Implementation

- [ ] Implement core functionality
- [ ] Add comprehensive tests
- [ ] Document public APIs

## Phase 4: Integration & Validation

- [ ] Integrate with existing codebase
- [ ] Run full test suite
- [ ] Performance validation (if applicable)
- [ ] Documentation review

## Progress Notes

<!-- Add dated notes as work progresses -->

- YYYY-MM-DD: Track created

"#.to_string()
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    let cli = Cli::parse();
    cli.run()
}
