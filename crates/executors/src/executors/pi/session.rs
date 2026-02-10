use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde_json::Value;
use uuid::Uuid;

pub fn fork_session(session_id: &str) -> io::Result<PathBuf> {
    validate_session_id(session_id)?;
    let root = sessions_root()?;
    let source = find_session_file(&root, session_id)?;
    let contents = fs::read_to_string(&source)?;
    let ends_with_newline = contents.ends_with('\n');

    let new_session_id = Uuid::new_v4().to_string();
    let replaced = contents
        .lines()
        .enumerate()
        .map(|(idx, line)| {
            if idx == 0 {
                replace_session_id(line, &new_session_id)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut output = replaced;
    if ends_with_newline {
        output.push('\n');
    }

    let destination = source
        .parent()
        .unwrap_or(root.as_path())
        .join(format!("{new_session_id}.jsonl"));
    fs::write(&destination, output)?;

    // Pi doesn't use separate settings files like Droid, but we'll check anyway
    if let Ok(settings_source) = find_session_file(&root, &format!("{session_id}.settings.json")) {
        let settings_destination = settings_source
            .parent()
            .unwrap_or(root.as_path())
            .join(format!("{new_session_id}.settings.json"));
        let _ = fs::copy(settings_source, settings_destination);
    }

    Ok(destination)
}

fn validate_session_id(session_id: &str) -> io::Result<()> {
    if session_id.contains('/') || session_id.contains('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "session_id contains a path separator",
        ));
    }

    if Uuid::parse_str(session_id).is_err() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "session_id is not a valid UUID",
        ));
    }

    Ok(())
}

/// Discover the session ID from the newest session file in Pi's sessions directory
/// for the given working directory. Pi creates session files named
/// `{timestamp}_{uuid}.jsonl` in a subdirectory derived from the cwd.
///
/// # Concurrency Safety
///
/// To prevent cross-run session mixing in concurrent scenarios, this function
/// filters session files by creation time, only considering files created after
/// `min_creation_time` if provided. This prevents selecting session files from
/// concurrent or previous Pi runs.
pub fn find_latest_session_id(cwd: &Path) -> io::Result<String> {
    find_latest_session_id_with_constraint(cwd, None)
}

/// Internal version of `find_latest_session_id` with optional time constraint
/// to filter out session files created before a specific time.
pub(crate) fn find_latest_session_id_with_constraint(
    cwd: &Path,
    min_creation_time: Option<SystemTime>,
) -> io::Result<String> {
    find_latest_session_id_with_root(cwd, min_creation_time, None)
}

/// Internal version with custom root for testing
fn find_latest_session_id_with_root(
    cwd: &Path,
    min_creation_time: Option<SystemTime>,
    custom_root: Option<PathBuf>,
) -> io::Result<String> {
    let root = custom_root.unwrap_or_else(|| sessions_root().unwrap());
    // Canonicalize the path to resolve symlinks (e.g., /var -> /private/var on macOS)
    let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let subdir_name = encode_cwd_to_dirname(&canonical_cwd);
    let subdir = root.join(&subdir_name);

    tracing::debug!(
        "Looking for Pi session in directory: {} (from cwd: {})",
        subdir.display(),
        cwd.display()
    );

    if !subdir.is_dir() {
        tracing::warn!(
            "Pi sessions directory not found: {} (encoded from: {})",
            subdir.display(),
            cwd.display()
        );
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Pi sessions directory not found: {}", subdir.display()),
        ));
    }

    // Find the most recently modified .jsonl file, filtering by creation time if specified
    let dir_entries: Vec<_> = fs::read_dir(&subdir)?.filter_map(|entry| entry.ok()).collect();
    tracing::debug!("Read {} total entries from {}", dir_entries.len(), subdir.display());

    let files: Vec<_> = dir_entries
        .into_iter()
        .filter(|entry| {
            let has_jsonl_ext = entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "jsonl");
            tracing::trace!("File {:?} has .jsonl extension: {}", entry.file_name(), has_jsonl_ext);
            has_jsonl_ext
        })
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("Failed to get metadata for {:?}: {}", entry.file_name(), e);
                    return None;
                }
            };

            // Get both creation time (for filtering) and modification time (for sorting)
            let created = metadata.created().ok();
            let modified = metadata.modified().ok()?;

            // Filter out files created before the minimum creation time (if specified)
            if let (Some(min_time), Some(file_created)) = (min_creation_time, created) {
                if file_created < min_time {
                    tracing::debug!(
                        "Skipping session file {:?} (created before process start)",
                        entry.file_name()
                    );
                    return None;
                }
            }

            Some((path, modified))
        })
        .collect();

    tracing::debug!("Found {} candidate session files in {}", files.len(), subdir.display());
    for (path, _) in &files {
        tracing::debug!("  - {}", path.display());
    }

    let newest = files.into_iter().max_by_key(|(_, modified)| *modified);

    let path = newest
        .map(|(path, _)| path)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("No session files found in {}", subdir.display()),
            )
        })?;

    tracing::debug!("Using newest session file: {}", path.display());
    let session_id = extract_session_id_from_file(&path)?;
    tracing::info!("Discovered Pi session_id: {}", session_id);

    Ok(session_id)
}

fn sessions_root() -> io::Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".pi").join("agent").join("sessions"))
        .ok_or_else(|| io::Error::other("Unable to determine home directory"))
}

/// Encode a cwd path into the directory name Pi uses for sessions.
/// Pi strips the leading `/`, replaces remaining `/` with `-`, and wraps with `--`.
/// e.g. `/home/user/project` -> `--home-user-project--`
fn encode_cwd_to_dirname(cwd: &Path) -> String {
    let cwd_str = cwd.to_string_lossy();
    let without_leading_slash = cwd_str.trim_start_matches('/');
    let encoded = without_leading_slash.replace('/', "-");
    format!("--{encoded}--")
}

/// Extract the session ID from the first line of a Pi session JSONL file.
fn extract_session_id_from_file(path: &Path) -> io::Result<String> {
    let contents = fs::read_to_string(path)?;
    let first_line = contents.lines().next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Session file is empty")
    })?;

    let meta: Value = serde_json::from_str(first_line).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Invalid JSON: {e}"))
    })?;

    meta.get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Session file first line has no 'id' field",
            )
        })
}

fn replace_session_id(line: &str, new_session_id: &str) -> String {
    if let Ok(mut meta) = serde_json::from_str::<Value>(line)
        && let Some(Value::String(id)) = meta.get_mut("id")
    {
        *id = new_session_id.to_string();
        if let Ok(serialized) = serde_json::to_string(&meta) {
            return serialized;
        }
    }

    line.to_string()
}

/// Find a session file by session_id. Pi names files as `{timestamp}_{uuid}.jsonl`,
/// so we search for files whose name ends with `_{session_id}.jsonl` or matches
/// exactly `{session_id}.jsonl` (for forked sessions).
fn find_session_file(root: &Path, session_id: &str) -> io::Result<PathBuf> {
    let exact_name = format!("{session_id}.jsonl");
    let suffix = format!("_{session_id}.jsonl");

    // Check root directory
    if let Some(found) = find_in_dir(root, &exact_name, &suffix) {
        return Ok(found);
    }

    // Check subdirectories
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_in_dir(&path, &exact_name, &suffix) {
                return Ok(found);
            }
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "Unable to locate session {session_id} in {}",
            root.display()
        ),
    ))
}

/// Search a single directory for a file matching either the exact name or the suffix pattern.
fn find_in_dir(dir: &Path, exact_name: &str, suffix: &str) -> Option<PathBuf> {
    // Try exact match first
    let exact = dir.join(exact_name);
    if exact.exists() {
        return Some(exact);
    }

    // Search for timestamp-prefixed files
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(suffix) {
                return Some(entry.path());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn fork_session_rejects_non_uuid() {
        let err = fork_session("not-a-uuid").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn fork_session_rejects_path_separators() {
        let err = fork_session("123e4567-e89b-12d3-a456-426614174000/evil").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_concurrent_session_discovery_filters_by_time() {
        // Create a temporary directory structure that mimics Pi's session layout
        let temp_dir = std::env::temp_dir().join(format!("pi-test-{}", Uuid::new_v4()));
        let sessions_dir = temp_dir.join("sessions");
        let cwd_dir = temp_dir.join("project");
        fs::create_dir_all(&cwd_dir).unwrap();

        // Encode the project directory name using Pi's convention
        let cwd_encoded = encode_cwd_to_dirname(&cwd_dir.canonicalize().unwrap());
        let session_subdir = sessions_dir.join(&cwd_encoded);
        fs::create_dir_all(&session_subdir).unwrap();

        // Simulate old session files from a previous run
        let old_session_id = Uuid::new_v4().to_string();
        let old_session_file = session_subdir.join(format!("1234567890_{}.jsonl", old_session_id));

        // Create the old session file with proper metadata
        fs::write(
            &old_session_file,
            format!(r#"{{"id":"{}","type":"session_metadata"}}"#, old_session_id),
        )
        .unwrap();

        // Record the current process start time
        let process_start_time = SystemTime::now();

        // Sleep briefly to ensure time separation
        thread::sleep(Duration::from_millis(100));

        // Create a new session file that should be discovered (created after process start)
        let new_session_id = Uuid::new_v4().to_string();
        let new_session_file = session_subdir.join(format!("9999999999_{}.jsonl", new_session_id));
        fs::write(
            &new_session_file,
            format!(r#"{{"id":"{}","type":"session_metadata"}}"#, new_session_id),
        )
        .unwrap();

        // Test 1: Without time constraint, should find the newest file (new_session_id)
        let result = find_latest_session_id_with_root(&cwd_dir, None, Some(sessions_dir.clone()));
        assert!(
            result.is_ok(),
            "Should find a session without time constraint: {:?}",
            result
        );
        let found_id = result.unwrap();
        assert_eq!(
            found_id, new_session_id,
            "Should find the newest session file"
        );

        // Test 2: With time constraint, should still find new_session_id (created after constraint)
        let result = find_latest_session_id_with_root(
            &cwd_dir,
            Some(process_start_time),
            Some(sessions_dir.clone()),
        );
        assert!(
            result.is_ok(),
            "Should find a session with time constraint: {:?}",
            result
        );
        let found_id = result.unwrap();
        assert_eq!(
            found_id, new_session_id,
            "Should find only the session created after process start time"
        );

        // Test 3: With time constraint in the future, should find nothing
        let future_time = SystemTime::now()
            .checked_add(Duration::from_secs(10))
            .unwrap();
        let result = find_latest_session_id_with_root(&cwd_dir, Some(future_time), Some(sessions_dir));
        assert!(
            result.is_err(),
            "Should not find any sessions with future time constraint"
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_encode_cwd_to_dirname() {
        // Test basic path encoding
        let path = Path::new("/home/user/project");
        let encoded = encode_cwd_to_dirname(path);
        assert_eq!(encoded, "--home-user-project--");

        // Test root path
        let path = Path::new("/");
        let encoded = encode_cwd_to_dirname(path);
        assert_eq!(encoded, "----");

        // Test path without leading slash (edge case)
        let path = Path::new("relative/path");
        let encoded = encode_cwd_to_dirname(path);
        assert_eq!(encoded, "--relative-path--");
    }
}
