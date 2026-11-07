use super::git_workbench_store::{directory, read, write};
use super::git_workflow::{checked, git, output, require_idle, validate_files, HISTORY_WRITE};
use crate::app_error::AppCommandError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Serialize, Deserialize)]
struct Tools {
    messages: Vec<String>,
    shelves: BTreeMap<String, ShelfMetadata>,
}
#[derive(Default, Serialize, Deserialize)]
struct ShelfMetadata {
    name: String,
    group: String,
}
pub(crate) fn remember_message(path: &str, message: &str) {
    let result = (|| -> Result<(), AppCommandError> {
        let file = directory(path)?.join("commit-tools.json");
        let mut state: Tools = read(&file)?;
        state.messages.retain(|s| s != message);
        state
            .messages
            .insert(0, message.chars().take(10000).collect());
        state.messages.truncate(50);
        write(&file, &state)
    })();
    if let Err(err) = result {
        tracing::warn!("Cannot save commit message history: {err}");
    }
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_commit_tools(
    path: String,
    operation: String,
    files: Option<Vec<String>>,
    shelf_ref: Option<String>,
    name: Option<String>,
    group: Option<String>,
) -> Result<serde_json::Value, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    let file = directory(&path)?.join("commit-tools.json");
    let mut state: Tools = read(&file)?;
    if operation == "state" {
        return serde_json::to_value(state)
            .map_err(|e| AppCommandError::invalid_input(e.to_string()));
    }
    if operation == "shelf-meta" {
        let reference =
            shelf_ref.ok_or_else(|| AppCommandError::invalid_input("Select a shelf"))?;
        super::git_shelf::validate_ref(&reference)?;
        git(&path, &["rev-parse", "--verify", &reference]).await?;
        let name = name.unwrap_or_default().trim().to_string();
        let group = group.unwrap_or_default().trim().to_string();
        if name.is_empty()
            || name.len() > 200
            || group.len() > 100
            || name.contains(['\n', '\r', '\0'])
            || group.contains(['\n', '\r', '\0'])
        {
            return Err(AppCommandError::invalid_input("Invalid shelf name/group"));
        }
        state
            .shelves
            .insert(reference, ShelfMetadata { name, group });
        write(&file, &state)?;
        return Ok(serde_json::Value::Null);
    }
    if operation == "shelf-paths" {
        let reference =
            shelf_ref.ok_or_else(|| AppCommandError::invalid_input("Select a shelf"))?;
        super::git_shelf::validate_ref(&reference)?;
        let raw = git(
            &path,
            &[
                "stash",
                "show",
                "--include-untracked",
                "--no-renames",
                "--name-only",
                "-z",
                &reference,
            ],
        )
        .await?;
        return Ok(serde_json::json!(raw
            .split('\0')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()));
    }
    let files = files.unwrap_or_default();
    validate_files(&files)?;
    if files.is_empty() {
        return Err(AppCommandError::invalid_input("Select files"));
    }
    require_idle(&path).await?;
    let specs: Vec<String> = files.iter().map(|f| format!(":(literal){f}")).collect();
    match operation.as_str() {
        "ignore" => {
            let root = std::fs::canonicalize(&path).map_err(AppCommandError::io)?;
            let ignore = super::folders::resolve_tree_path(&root, ".gitignore")?;
            if std::fs::symlink_metadata(&ignore).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(AppCommandError::invalid_input(
                    ".gitignore must not be a symbolic link",
                ));
            }
            let mut text = match std::fs::read_to_string(&ignore) {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(AppCommandError::io(e)),
            };
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            for file in files {
                if file.contains(['\n', '\r']) {
                    return Err(AppCommandError::invalid_input(
                        "Newline paths cannot be added to ignore rules",
                    ));
                }
                let escaped: String = file
                    .chars()
                    .flat_map(|c| {
                        if "\\*?[]#! ".contains(c) {
                            vec!['\\', c]
                        } else {
                            vec![c]
                        }
                    })
                    .collect();
                text.push('/');
                text.push_str(&escaped);
                text.push('\n');
            }
            super::folders::atomic_write_text(&ignore, text.as_bytes())?;
        }
        "untrack" => {
            let mut args = vec!["rm", "--cached", "--"];
            args.extend(specs.iter().map(String::as_str));
            git(&path, &args).await?;
        }
        "shelf-files" | "shelf-restore" => {
            let reference =
                shelf_ref.ok_or_else(|| AppCommandError::invalid_input("Select a shelf"))?;
            super::git_shelf::validate_ref(&reference)?;
            let hash = git(&path, &["rev-parse", "--verify", &reference]).await?;
            let base = format!("{hash}^1");
            let mut args = vec![
                "diff",
                "--binary",
                "--no-ext-diff",
                "--no-textconv",
                &base,
                &hash,
                "--",
            ];
            args.extend(specs.iter().map(String::as_str));
            let result = output(&path, &args).await?;
            let mut patch = String::from_utf8_lossy(&result.stdout).into_owned();
            checked(result)?;
            let untracked = format!("{hash}^3");
            if output(&path, &["rev-parse", "--verify", &untracked])
                .await?
                .status
                .success()
            {
                let empty = git(&path, &["hash-object", "-t", "tree", "--stdin"]).await?;
                let mut args = vec![
                    "diff",
                    "--binary",
                    "--no-ext-diff",
                    "--no-textconv",
                    &empty,
                    &untracked,
                    "--",
                ];
                args.extend(specs.iter().map(String::as_str));
                let result = output(&path, &args).await?;
                let extra = String::from_utf8_lossy(&result.stdout).into_owned();
                checked(result)?;
                if !extra.is_empty() {
                    patch.push('\n');
                    patch.push_str(&extra);
                }
            }
            if operation == "shelf-files" {
                return Ok(serde_json::Value::String(patch));
            }
            if patch.trim().is_empty() {
                return Err(AppCommandError::invalid_input(
                    "No shelved changes for these files",
                ));
            }
            // Git checks every hunk before touching any file; keep the shelf ref.
            checked(
                super::git_patch::apply_command(&path, &patch, &["--check", "--whitespace=nowarn"])
                    .await?,
            )?;
            checked(
                super::git_patch::apply_command(&path, &patch, &["--whitespace=nowarn"]).await?,
            )?;
        }
        _ => return Err(AppCommandError::invalid_input("Unknown operation")),
    }
    Ok(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn manage(
        path: &str,
        op: &str,
        files: Option<Vec<String>>,
        reference: Option<String>,
        name: Option<String>,
        group: Option<String>,
    ) -> Result<serde_json::Value, AppCommandError> {
        git_commit_tools(path.into(), op.into(), files, reference, name, group).await
    }
    #[tokio::test]
    async fn partial_restore_metadata_ignore_and_untrack_preserve_work() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        git(path, &["init", "-b", "main"]).await.unwrap();
        git(path, &["config", "user.name", "Test"]).await.unwrap();
        git(path, &["config", "user.email", "test@example.invalid"])
            .await
            .unwrap();
        for f in ["a", "b"] {
            std::fs::write(dir.path().join(f), "base\n").unwrap();
        }
        git(path, &["add", "."]).await.unwrap();
        git(path, &["commit", "-m", "root"]).await.unwrap();
        std::fs::write(dir.path().join("a"), "shelved\n").unwrap();
        std::fs::write(dir.path().join("b"), "other shelf\n").unwrap();
        std::fs::write(dir.path().join("[new]"), [0, 255, 1]).unwrap();
        super::super::git_shelf::git_shelf_save(path.into(), "Original".into(), None)
            .await
            .unwrap();
        let reference = super::super::git_shelf::git_shelf_list(path.into())
            .await
            .unwrap()[0]
            .ref_name
            .clone();
        manage(
            path,
            "shelf-meta",
            None,
            Some(reference.clone()),
            Some("Renamed".into()),
            Some("Feature".into()),
        )
        .await
        .unwrap();
        let state = manage(path, "state", None, None, None, None).await.unwrap();
        assert_eq!(state["shelves"][&reference]["name"], "Renamed");
        std::fs::write(dir.path().join("b"), "staged\n").unwrap();
        git(path, &["add", "b"]).await.unwrap();
        let index = git(path, &["ls-files", "--stage"]).await.unwrap();
        manage(
            path,
            "shelf-restore",
            Some(vec!["a".into(), "[new]".into()]),
            Some(reference.clone()),
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("[new]")).unwrap(),
            vec![0, 255, 1]
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a")).unwrap(),
            "shelved\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("b")).unwrap(),
            "staged\n"
        );
        assert_eq!(git(path, &["ls-files", "--stage"]).await.unwrap(), index);
        assert!(manage(
            path,
            "shelf-restore",
            Some(vec!["a".into()]),
            Some(reference.clone()),
            None,
            None
        )
        .await
        .is_err());
        assert_eq!(
            super::super::git_shelf::git_shelf_list(path.into())
                .await
                .unwrap()
                .len(),
            1
        );
        manage(path, "ignore", Some(vec!["[new]".into()]), None, None, None)
            .await
            .unwrap();
        assert!(git(path, &["check-ignore", "[new]"]).await.is_ok());
        assert!(
            manage(path, "untrack", Some(vec!["b".into()]), None, None, None)
                .await
                .is_ok()
        );
        assert!(dir.path().join("b").exists());
        assert!(manage(
            path,
            "ignore",
            Some(vec!["../escape".into()]),
            None,
            None,
            None
        )
        .await
        .is_err());
    }
    #[test]
    fn message_history_is_deduplicated_and_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        for i in 0..60 {
            remember_message(path, &format!("message {i}"));
        }
        remember_message(path, "message 59");
        let state: Tools = read(&directory(path).unwrap().join("commit-tools.json")).unwrap();
        assert_eq!(state.messages.len(), 50);
        assert_eq!(state.messages[0], "message 59");
    }
}

#[cfg(test)]
mod stash_tests {
 use super::*;
 #[tokio::test]
 async fn stash_includes_untracked_and_keeps_index_when_requested(){
  let dir=tempfile::tempdir().unwrap();let path=dir.path().to_str().unwrap();
  git(path,&["init","-b","main"]).await.unwrap();git(path,&["config","user.name","Test"]).await.unwrap();git(path,&["config","user.email","test@example.invalid"]).await.unwrap();
  std::fs::write(dir.path().join("a"),"base\n").unwrap();git(path,&["add","."]).await.unwrap();git(path,&["commit","-m","root"]).await.unwrap();
  std::fs::write(dir.path().join("a"),"staged\n").unwrap();git(path,&["add","a"]).await.unwrap();std::fs::write(dir.path().join("a"),"unstaged\n").unwrap();std::fs::write(dir.path().join("new"),"new\n").unwrap();
  super::super::folders::git_stash_push(path.into(),Some("Options".into()),true,Some(true)).await.unwrap();
  assert_eq!(git(path,&["show",":a"]).await.unwrap(),"staged");assert_eq!(std::fs::read_to_string(dir.path().join("a")).unwrap(),"staged\n");assert!(!dir.path().join("new").exists());assert_eq!(git(path,&["show","stash@{0}^3:new"]).await.unwrap(),"new");
 }
}
