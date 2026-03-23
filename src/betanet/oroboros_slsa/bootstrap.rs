use std::process::Command;
use std::path::Path;
use std::fs;
use serde_json::json;
use serde_json::Value;

use crate::oroboros_slsa::canonicalizer;

/// Scan git-tracked files and prepare a bootstrap JSON array for oroboros_couch.
///
/// Behavior:
/// - Runs `git ls-files` in the current working directory to list tracked files.
/// - For every file with extension `.json` or `.jsonl` reads and canonicalizes JSON.
/// - For `.jsonl` each line is treated as a separate JSON document.
/// - Emits a JSON array of objects: { id: "path", row: [ <canonical-json-string> ] }
///
/// Returns the JSON string on success or a String describing the error.
pub fn bootstrap_from_git(cwd: &str) -> Result<String, String> {
    // Run `git ls-files`
    let out = Command::new("git")
        .arg("ls-files")
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("failed to run git: {}", e))?;

    if !out.status.success() {
        return Err(format!("git ls-files failed: {}", String::from_utf8_lossy(&out.stderr)));
    }

    let ls = String::from_utf8_lossy(&out.stdout);
    let mut docs: Vec<Value> = Vec::new();

    for line in ls.lines() {
        let p = Path::new(line);
        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            if ext.eq_ignore_ascii_case("json") {
                if let Ok(src) = fs::read_to_string(p) {
                    // Try parse as a single JSON value; if that fails, wrap lines
                    let canonical = canonicalizer::canonicalize(&src);
                    if !canonical.is_empty() {
                        let doc = json!({"id": line.to_string(), "row": [serde_json::Value::String(canonical)]});
                        docs.push(doc);
                    }
                }
            } else if ext.eq_ignore_ascii_case("jsonl") || ext.eq_ignore_ascii_case("ndjson") {
                if let Ok(src) = fs::read_to_string(p) {
                    for (i, l) in src.lines().enumerate() {
                        if l.trim().is_empty() { continue; }
                        let canonical = canonicalizer::canonicalize(l);
                        if canonical.is_empty() { continue; }
                        let id = format!("{}:{}", line, i);
                        let doc = json!({"id": id, "row": [serde_json::Value::String(canonical)]});
                        docs.push(doc);
                    }
                }
            }
        }
    }

    serde_json::to_string(&docs).map_err(|e| format!("serialize error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_bootstrap_from_git_empty() {
        // Create a temp dir and init an empty git repository
        let td = tempfile::tempdir().expect("tempdir");
        let td_path = td.path().to_str().unwrap();
        // init git
        let _ = Command::new("git").arg("init").current_dir(td_path).output();
        // create a json file and add it
        let file_path = td.path().join("a.json");
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "{\"foo\": 1, \"bar\": [2,3]}").unwrap();
        let _ = Command::new("git").arg("add").arg("a.json").current_dir(td_path).output();
        let _ = Command::new("git").arg("commit").arg("-m").arg("add").current_dir(td_path).output();

        let s = bootstrap_from_git(td_path).expect("bootstrap should succeed");
        assert!(s.contains("a.json"));
    }
}
