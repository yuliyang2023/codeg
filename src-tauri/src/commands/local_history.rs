use super::git_workbench_store::{digest, directory, read, write};
use crate::app_error::AppCommandError;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Mutex,
};
static HISTORY: Mutex<()> = Mutex::new(());
const MAX_FILE: u64 = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 2000;
const MAX_BATCH_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalHistoryEntry {
    pub id: String,
    pub file: String,
    pub time: String,
    pub reason: String,
    pub label: Option<String>,
    pub blob: Option<String>,
    pub size: u64,
}
#[derive(Default, Serialize, Deserialize)]
struct History {
    entries: Vec<LocalHistoryEntry>,
}
#[derive(Serialize)]
pub struct LocalHistoryContent {
    pub entry: LocalHistoryEntry,
    pub original: Option<String>,
    pub current: Option<String>,
    pub current_hash: Option<String>,
    pub binary: bool,
}

fn checked_path(root: &Path, file: &str) -> Result<PathBuf, AppCommandError> {
    super::git_workflow::validate_files(&[file.to_string()])?;
    let mut target = root.to_path_buf();
    for component in Path::new(file).components() {
        if let std::path::Component::Normal(part) = component {
            if [".git", "node_modules", "target", ".next"]
                .iter()
                .any(|excluded| part == *excluded)
            {
                return Err(AppCommandError::invalid_input(
                    "Git metadata is excluded from Local History",
                ));
            }
            target.push(part);
            match std::fs::symlink_metadata(&target) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(AppCommandError::invalid_input(
                        "Local History does not follow symbolic links",
                    ))
                }
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(AppCommandError::io(err)),
            }
        }
    }
    if super::git_workbench_store::is_metadata_path(&target) {
        return Err(AppCommandError::invalid_input(
            "Workbench metadata is excluded from Local History",
        ));
    }
    Ok(target)
}
fn current_bytes(target: &Path) -> Result<Option<Vec<u8>>, AppCommandError> {
    match std::fs::metadata(target) {
        Ok(meta) if meta.is_file() && meta.len() <= MAX_FILE => {
            std::fs::read(target).map(Some).map_err(AppCommandError::io)
        }
        Ok(_) => Err(AppCommandError::invalid_input(
            "Local History supports regular files up to 2 MiB",
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(AppCommandError::io(err)),
    }
}
fn trim_store(_directory: &Path, history: &mut History) -> Result<(), AppCommandError> {
    let age = chrono::Utc::now() - chrono::Duration::days(30);
    history
        .entries
        .retain(|e| chrono::DateTime::parse_from_rfc3339(&e.time).is_ok_and(|t| t >= age));
    if history.entries.len() > MAX_ENTRIES {
        history.entries.drain(..history.entries.len() - MAX_ENTRIES);
    }
    let mut total = history
        .entries
        .iter()
        .filter_map(|e| e.blob.as_ref().map(|b| (b, e.size)))
        .collect::<std::collections::HashMap<_, _>>()
        .values()
        .sum::<u64>();
    while total > 256 * 1024 * 1024 && !history.entries.is_empty() {
        history.entries.remove(0);
        total = history
            .entries
            .iter()
            .filter_map(|e| e.blob.as_ref().map(|b| (b, e.size)))
            .collect::<std::collections::HashMap<_, _>>()
            .values()
            .sum::<u64>();
    }
    Ok(())
}

pub(crate) fn capture_files(
    root: &str,
    files: &[String],
    reason: &str,
    label: Option<String>,
) -> Result<usize, AppCommandError> {
    let _guard = HISTORY
        .lock()
        .map_err(|_| AppCommandError::invalid_input("Local History is busy"))?;
    let canonical = std::fs::canonicalize(root).map_err(AppCommandError::io)?;
    let directory = directory(root)?.join("history");
    std::fs::create_dir_all(directory.join("blobs")).map_err(AppCommandError::io)?;
    let file = directory.join("index.json");
    let mut history: History = read(&file)?;
    let mut count = 0;
    let mut bytes_read = 0;
    for name in files {
        let Ok(target) = checked_path(&canonical, name) else {
            continue;
        };
        let Ok(bytes) = current_bytes(&target) else {
            continue;
        };
        bytes_read += bytes.as_ref().map(|b| b.len() as u64).unwrap_or(0);
        if bytes_read > MAX_BATCH_BYTES {
            break;
        }
        let hash = bytes.as_ref().map(|b| digest(b));
        let previous = history.entries.iter().rev().find(|e| e.file == *name);
        if label.is_none()
            && (previous.is_some_and(|e| e.blob == hash) || (previous.is_none() && bytes.is_none()))
        {
            continue;
        }
        if let (Some(hash), Some(bytes)) = (&hash, &bytes) {
            let blob = directory.join("blobs").join(hash);
            if !blob.exists() {
                super::folders::atomic_write_text(&blob, bytes)?;
            }
        }
        history.entries.push(LocalHistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            file: name.clone(),
            time: chrono::Utc::now().to_rfc3339(),
            reason: reason.into(),
            label: label.clone(),
            blob: hash,
            size: bytes.as_ref().map(|b| b.len() as u64).unwrap_or(0),
        });
        count += 1;
    }
    if count == 0 {
        return Ok(0);
    }
    trim_store(&directory, &mut history)?;
    write(&file, &history)?;
    let referenced: HashSet<_> = history
        .entries
        .iter()
        .filter_map(|e| e.blob.as_deref())
        .collect();
    let blobs = directory.join("blobs");
    for item in std::fs::read_dir(&blobs).map_err(AppCommandError::io)? {
        let item = item.map_err(AppCommandError::io)?;
        if !referenced.contains(item.file_name().to_string_lossy().as_ref()) {
            std::fs::remove_file(item.path()).map_err(AppCommandError::io)?;
        }
    }
    Ok(count)
}

pub(crate) fn capture_project(
    root: &str,
    reason: &str,
    label: Option<String>,
) -> Result<usize, AppCommandError> {
    let root_path = std::fs::canonicalize(root).map_err(AppCommandError::io)?;
    let mut files = Vec::new();
    let mut total = 0;
    let walker = ignore::WalkBuilder::new(&root_path)
        .hidden(false)
        .parents(false)
        .git_global(false)
        .require_git(false)
        .filter_entry(|e| {
            !super::git_workbench_store::is_metadata_path(e.path())
                && e.file_name() != ".git"
                && e.file_name() != "node_modules"
                && e.file_name() != "target"
                && e.file_name() != ".next"
        })
        .build();
    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(MAX_FILE + 1);
        if size > MAX_FILE {
            continue;
        }
        total += size;
        if total > MAX_BATCH_BYTES || files.len() >= MAX_ENTRIES {
            break;
        }
        if let Ok(relative) = entry.path().strip_prefix(&root_path) {
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    // Remember removals even when a watch batch overflowed.
    let old: History = read(&directory(root)?.join("history/index.json"))?;
    for entry in old.entries.iter().rev() {
        if entry.blob.is_some()
            && !root_path.join(&entry.file).exists()
            && !files.contains(&entry.file)
        {
            files.push(entry.file.clone());
        }
    }
    capture_files(root, &files, reason, label)
}

pub(crate) fn automatic(root: &str, files: &[String], reason: &str) {
    if let Err(err) = capture_files(root, files, reason, None) {
        tracing::warn!("Local History capture failed: {}", err);
    }
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn local_history_capture(
    path: String,
    file: Option<String>,
    label: Option<String>,
) -> Result<usize, AppCommandError> {
    let label = label
        .map(|s| s.trim().chars().take(200).collect::<String>())
        .filter(|s| !s.is_empty());
    tokio::task::spawn_blocking(move || match file {
        Some(file) => {
            checked_path(
                &std::fs::canonicalize(&path).map_err(AppCommandError::io)?,
                &file,
            )?;
            capture_files(&path, &[file], "manual", label)
        }
        None => capture_project(&path, "label", label),
    })
    .await
    .map_err(|e| AppCommandError::invalid_input(e.to_string()))?
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn local_history_list(
    path: String,
    file: Option<String>,
) -> Result<Vec<LocalHistoryEntry>, AppCommandError> {
    let _guard = HISTORY
        .lock()
        .map_err(|_| AppCommandError::invalid_input("Local History is busy"))?;
    let history: History = read(&directory(&path)?.join("history/index.json"))?;
    Ok(history
        .entries
        .into_iter()
        .rev()
        .filter(|entry| file.as_ref().is_none_or(|f| entry.file == *f))
        .collect())
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn local_history_content(
    path: String,
    id: String,
) -> Result<LocalHistoryContent, AppCommandError> {
    let _guard = HISTORY
        .lock()
        .map_err(|_| AppCommandError::invalid_input("Local History is busy"))?;
    let dir = directory(&path)?.join("history");
    let history: History = read(&dir.join("index.json"))?;
    let entry = history
        .entries
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| AppCommandError::not_found("Snapshot does not exist"))?;
    let original = entry
        .blob
        .as_ref()
        .map(|hash| std::fs::read(dir.join("blobs").join(hash)).map_err(AppCommandError::io))
        .transpose()?;
    let current = current_bytes(&checked_path(
        &std::fs::canonicalize(&path).map_err(AppCommandError::io)?,
        &entry.file,
    )?)?;
    let binary = original
        .iter()
        .chain(current.iter())
        .any(|bytes| bytes.contains(&0) || std::str::from_utf8(bytes).is_err());
    let original_text = original
        .as_ref()
        .and_then(|b| String::from_utf8(b.clone()).ok());
    let current_text = current
        .as_ref()
        .and_then(|b| String::from_utf8(b.clone()).ok());
    Ok(LocalHistoryContent {
        entry,
        original: original_text,
        current: current_text,
        current_hash: current.as_ref().map(|b| digest(b)),
        binary,
    })
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn local_history_restore(
    path: String,
    id: String,
    expected_hash: Option<String>,
) -> Result<(), AppCommandError> {
    let current = local_history_content(path.clone(), id.clone()).await?;
    if current.current_hash != expected_hash {
        return Err(AppCommandError::invalid_input(
            "File changed; refresh the comparison before restoring",
        ));
    }
    // Retain the selected bytes before capturing the displaced version. That
    // capture may prune the oldest entry at the retention boundary.
    let selected_bytes = {
        let _guard = HISTORY
            .lock()
            .map_err(|_| AppCommandError::invalid_input("Local History is busy"))?;
        current
            .entry
            .blob
            .as_ref()
            .map(|hash| {
                std::fs::read(directory(&path)?.join("history/blobs").join(hash))
                    .map_err(AppCommandError::io)
            })
            .transpose()?
    };
    // Capture the displaced content before touching the working tree.
    capture_files(
        &path,
        std::slice::from_ref(&current.entry.file),
        "before-restore",
        Some("Before restore".into()),
    )?;
    let _guard = HISTORY
        .lock()
        .map_err(|_| AppCommandError::invalid_input("Local History is busy"))?;
    let root = std::fs::canonicalize(&path).map_err(AppCommandError::io)?;
    let target = checked_path(&root, &current.entry.file)?;
    if current_bytes(&target)?.as_ref().map(|b| digest(b)) != expected_hash {
        return Err(AppCommandError::invalid_input(
            "File changed during restoration",
        ));
    }
    if let Some(bytes) = selected_bytes {
        let parent = target
            .parent()
            .ok_or_else(|| AppCommandError::invalid_input("Invalid file path"))?;
        std::fs::create_dir_all(parent).map_err(AppCommandError::io)?;
        super::folders::atomic_write_text(&target, &bytes)?;
    } else if target.exists() {
        std::fs::remove_file(target).map_err(AppCommandError::io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn snapshots_deletion_binary_restore_and_stale_protection() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let file = dir.path().join("a");
        std::fs::write(&file, "one\n").unwrap();
        capture_files(&root, &["a".into()], "open", None).unwrap();
        assert_eq!(
            capture_files(&root, &["a".into()], "duplicate", None).unwrap(),
            0
        );
        let initial = local_history_list(root.clone(), None).await.unwrap()[0]
            .id
            .clone();
        std::fs::write(&file, "two\n").unwrap();
        let shown = local_history_content(root.clone(), initial.clone())
            .await
            .unwrap();
        std::fs::write(&file, "three\n").unwrap();
        assert!(
            local_history_restore(root.clone(), initial.clone(), shown.current_hash)
                .await
                .is_err()
        );
        let current = local_history_content(root.clone(), initial.clone())
            .await
            .unwrap();
        local_history_restore(root.clone(), initial, current.current_hash)
            .await
            .unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "one\n");
        std::fs::remove_file(&file).unwrap();
        capture_files(&root, &["a".into()], "delete", None).unwrap();
        let deleted = local_history_list(root.clone(), None).await.unwrap()[0]
            .id
            .clone();
        std::fs::write(&file, [0, 255, 2]).unwrap();
        capture_files(&root, &["a".into()], "binary", None).unwrap();
        let binary = local_history_list(root.clone(), None).await.unwrap()[0]
            .id
            .clone();
        assert!(
            local_history_content(root.clone(), binary.clone())
                .await
                .unwrap()
                .binary
        );
        std::fs::write(&file, "changed").unwrap();
        let state = local_history_content(root.clone(), binary.clone())
            .await
            .unwrap();
        local_history_restore(root.clone(), binary, state.current_hash)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), vec![0, 255, 2]);
        let state = local_history_content(root.clone(), deleted.clone())
            .await
            .unwrap();
        local_history_restore(root, deleted, state.current_hash)
            .await
            .unwrap();
        assert!(!file.exists());
    }
    #[tokio::test]
    async fn restoring_oldest_snapshot_survives_retention_pruning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("a"), "old").unwrap();
        capture_files(&path, &["a".into()], "old", None).unwrap();
        let index = directory(&path).unwrap().join("history/index.json");
        let mut history: History = read(&index).unwrap();
        let old = history.entries[0].clone();
        std::fs::write(dir.path().join("a"), "new").unwrap();
        capture_files(&path, &["a".into()], "new", None).unwrap();
        let newest: History = read(&index).unwrap();
        let template = newest.entries.last().unwrap().clone();
        while history.entries.len() < MAX_ENTRIES {
            let mut entry = template.clone();
            entry.id = uuid::Uuid::new_v4().to_string();
            history.entries.push(entry);
        }
        write(&index, &history).unwrap();
        let current = local_history_content(path.clone(), old.id.clone())
            .await
            .unwrap();
        local_history_restore(path, old.id, current.current_hash)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a")).unwrap(),
            "old"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlinks_and_parent_paths_cannot_escape() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), "keep").unwrap();
        symlink(outside.path(), root.path().join("link")).unwrap();
        let path = root.path().to_str().unwrap().to_string();
        assert!(
            local_history_capture(path.clone(), Some("link/secret".into()), None)
                .await
                .is_err()
        );
        assert!(local_history_capture(path, Some("../secret".into()), None)
            .await
            .is_err());
    }
}

#[cfg(test)]
mod file_operation_tests {
    use super::*;
    #[tokio::test]
    async fn editor_save_rename_and_delete_keep_before_images() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("a"), "original\n").unwrap();
        super::super::folders::save_file_content(path.clone(), "a".into(), "saved\n".into(), None)
            .await
            .unwrap();
        let entries = local_history_list(path.clone(), Some("a".into()))
            .await
            .unwrap();
        assert!(entries.len() >= 2);
        let oldest = entries.last().unwrap();
        assert_eq!(
            local_history_content(path.clone(), oldest.id.clone())
                .await
                .unwrap()
                .original
                .as_deref(),
            Some("original\n")
        );
        super::super::folders::rename_file_tree_entry(path.clone(), "a".into(), "b".into())
            .await
            .unwrap();
        assert!(local_history_list(path.clone(), Some("a".into()))
            .await
            .unwrap()[0]
            .blob
            .is_none());
        super::super::folders::delete_file_tree_entry(path.clone(), "b".into())
            .await
            .unwrap();
        let deleted = local_history_list(path, Some("b".into())).await.unwrap();
        assert!(deleted[0].blob.is_none());
        assert!(deleted.iter().any(|entry| entry.blob.is_some()));
    }
}
