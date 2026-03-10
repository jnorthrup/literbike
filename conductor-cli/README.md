# Conductor CLI

Development track management CLI for the Literbike project.

## Overview

Conductor is a command-line tool for managing development tracks, tasks, and progress in the Literbike project. It provides visibility into track completion, task status, and git integration for commit tracking.

## Installation

### Build from Source

```bash
# Build debug version
cd conductor-cli
cargo build

# Build release version
cargo build --release

# The binary will be at:
# - Debug: ../target/debug/conductor
# - Release: ../target/release/conductor
```

### Usage

The conductor CLI is included in the Literbike workspace. After building, you can run it from the project root:

```bash
./target/release/conductor --help
```

## Commands

### `list` - List all development tracks

```bash
# List all tracks
conductor list

# Filter by status
conductor list --status complete
conductor list --status in_progress
conductor list --status pending

# Show detailed information
conductor list --detailed
```

### `show` - Show detailed track information

```bash
# Show specific track
conductor show kotlin-quic-packet-processing-port_20260225
```

### `status` - Show overall progress

```bash
# Text format (default)
conductor status

# JSON format
conductor status --format json
```

### `progress` - Track progress visualization

```bash
# Show progress for all tracks
conductor progress

# Show progress for specific track
conductor progress kotlin-quic-packet-processing-port_20260225
```

### `init` - Initialize a new track

```bash
# Create new track with auto-generated title
conductor init my-feature_20260309

# Create new track with custom title
conductor init my-feature_20260309 --title "My Awesome Feature"
```

### `update` - Update task status

```bash
# Mark task as complete
conductor update track-id "task description" --status complete

# Mark task as in progress
conductor update track-id "task pattern" --status in_progress

# Mark task as pending
conductor update track-id "task pattern" --status pending
```

### `validate` - Validate track structure

```bash
# Validate all tracks
conductor validate

# Validate specific track
conductor validate track-id
```

### `report` - Generate summary report

```bash
# Markdown report (default)
conductor report

# JSON report
conductor report --format json

# Write to file
conductor report --output report.md
conductor report --format json --output report.json
```

### `git` - Show git integration status

```bash
# Show git status
conductor git

# Show git status for specific track
conductor git --track track-id
```

## Track Structure

Each development track is stored in `conductor/tracks/<track-id>/` with the following structure:

```
conductor/tracks/
└── track-name_YYYYMMDD/
    ├── spec.md          # Track specification and goals
    ├── plan.md          # Implementation plan with tasks
    └── metadata.json    # Track metadata (owner, priority, etc.)
```

### Track ID Format

Track IDs follow the pattern: `name_YYYYMMDD`

- `name`: Descriptive name (lowercase, hyphens or underscores)
- `YYYYMMDD`: Creation date (e.g., 20260309)

Example: `kotlin-quic-packet-processing-port_20260225`

### Task Status

Tasks in `plan.md` use markdown checkbox syntax:

- `[ ]` - Pending
- `[~]` - In Progress
- `[x]` - Complete

When a task is marked complete, it should include the commit SHA:

```markdown
- [x] Implement feature (abc1234) - Added new functionality
```

## Examples

### Check Overall Progress

```bash
$ conductor status

Literbike Development Status
============================================================

Tracks: 4/8 complete (50.0%)
[████████████████████░░░░░░░░░░░░░░░░░░░░]

Tasks: 62/89 complete (69.7%)
[███████████████████████████░░░░░░░░░░░░░]

Breakdown:
  4 Complete
  3 In Progress
  1 Pending
```

### View Track Details

```bash
$ conductor show kotlin-quic-packet-processing-port_20260225

kotlin-quic-packet-processing-port_20260225
============================================================
Title: Port Kotlin QUIC (Full Packet Processing from Trikeshed)
Status: Complete
Created: 20260225

Tasks: 17/17 complete, 0 in progress, 0 pending

Task List:
  ✓ Read Trikeshed QUIC engine sources
  ✓ Identify overlap with existing QUIC interop foundation
  ✓ Refactor packet processing flow
  ...
```

### Generate Report

```bash
$ conductor report --format json
{
  "report_date": "2026-03-09",
  "summary": {
    "total_tracks": 8,
    "complete_tracks": 4,
    "in_progress_tracks": 3,
    "pending_tracks": 1,
    "track_completion": "50.0%",
    "task_completion": "69.7%"
  },
  "tracks": [...]
}
```

## Development

### Building

```bash
# Build with all dependencies
cargo build

# Run tests (when available)
cargo test

# Check code quality
cargo clippy
cargo fmt --check
```

### Adding Features

1. Create a new track for the feature
2. Update `plan.md` with tasks
3. Implement tasks following TDD workflow
4. Update task status with `conductor update`
5. Commit with task summary

## License

AGPL-3.0 (same as Literbike)

## Contributing

1. Create a new track using `conductor init`
2. Define scope in `spec.md`
3. Break down work in `plan.md`
4. Implement and track progress
5. Mark tasks complete with commit SHAs
