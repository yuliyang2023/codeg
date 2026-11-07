//! Native history operations. Commands return conflict state rather than
//! treating an interrupted operation as a completed commit.
use serde::Serialize;
use std::path::{Component, Path};
use std::process::Output;
use tokio::sync::Mutex;

use super::folders::{detect_conflicts, GitConflictInfo};
use crate::app_error::AppCommandError;

pub(crate) static HISTORY_WRITE: Mutex<()> = Mutex::const_new(());

pub(crate) async fn output(path: &str, args: &[&str]) -> Result<Output, AppCommandError> {
    crate::commands::git_console::output(
        crate::process::tokio_command("git")
            .args(args)
            .current_dir(path)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_EDITOR", "true")
            .env("GIT_SEQUENCE_EDITOR", "true"),
    )
    .await
    .map_err(AppCommandError::io)
}

pub(crate) fn checked(result: Output) -> Result<String, AppCommandError> {
    if !result.status.success() {
        return Err(AppCommandError::external_command(
            "Git operation failed",
            String::from_utf8_lossy(&result.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&result.stdout).trim().to_string())
}

pub(crate) async fn git(path: &str, args: &[&str]) -> Result<String, AppCommandError> {
    checked(output(path, args).await?)
}

pub(crate) fn validate_files(files: &[String]) -> Result<(), AppCommandError> {
    for file in files {
        if file.is_empty()
            || file.contains('\0')
            || Path::new(file)
                .components()
                .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err(AppCommandError::invalid_input(
                "Select repository-relative file paths",
            ));
        }
    }
    Ok(())
}

pub(crate) async fn require_idle(path: &str) -> Result<(), AppCommandError> {
    let git_dir = git(path, &["rev-parse", "--absolute-git-dir"]).await?;
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
        "codeg-restore-operation",
    ] {
        let target = Path::new(&git_dir).join(name);
        if target.exists() {
            return Err(AppCommandError::invalid_input(
                "Finish or abort the current Git operation first",
            ));
        }
    }
    if !detect_conflicts(path).await?.is_empty() {
        return Err(AppCommandError::invalid_input(
            "Resolve existing conflicts first",
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct GitWorkflowResult {
    pub message: String,
    pub conflict: Option<GitConflictInfo>,
}

/// Shared by restore operations as well as cherry-pick/revert. A failed apply
/// with no unmerged paths is a real failure, not an empty conflict dialog.
pub(crate) async fn operation_result(
    path: &str,
    operation: &str,
    result: Output,
) -> Result<GitWorkflowResult, AppCommandError> {
    if result.status.success() {
        return Ok(GitWorkflowResult {
            message: String::from_utf8_lossy(&result.stdout).trim().to_string(),
            conflict: None,
        });
    }
    let files = detect_conflicts(path).await?;
    if !files.is_empty() {
        if matches!(operation, "stash" | "shelf") {
            let marker = git_path(path, "codeg-restore-operation").await?;
            std::fs::write(marker, operation).map_err(AppCommandError::io)?;
        }
        return Ok(GitWorkflowResult {
            message: String::from_utf8_lossy(&result.stderr).trim().to_string(),
            conflict: Some(GitConflictInfo {
                has_conflicts: true,
                conflicted_files: files,
                operation: operation.to_string(),
                upstream_commit: None,
            }),
        });
    }
    checked(result).map(|message| GitWorkflowResult {
        message,
        conflict: None,
    })
}

async fn git_path(path: &str, name: &str) -> Result<std::path::PathBuf, AppCommandError> {
    let location = git(path, &["rev-parse", "--git-path", name]).await?;
    let location = Path::new(&location);
    Ok(if location.is_absolute() {
        location.to_path_buf()
    } else {
        Path::new(path).join(location)
    })
}

pub(crate) async fn finish_restore(path: &str) -> Result<(), AppCommandError> {
    let marker = git_path(path, "codeg-restore-operation").await?;
    if marker.exists() {
        std::fs::remove_file(marker).map_err(AppCommandError::io)?;
    }
    super::git_smart::finished_restore(path)?;
    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_operation_state(path: String) -> Result<Option<GitConflictInfo>, AppCommandError> {
    let git_dir = git(&path, &["rev-parse", "--absolute-git-dir"]).await?;
    let mut operation = None;
    for (name, op) in [
        ("MERGE_HEAD", "merge"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
    ] {
        if Path::new(&git_dir).join(name).exists() {
            operation = Some(op.to_string());
        }
    }
    if operation.is_none() {
        if let Ok(todo) = std::fs::read_to_string(Path::new(&git_dir).join("sequencer/todo")) {
            let command = todo
                .lines()
                .find(|line| !line.trim().is_empty() && !line.starts_with('#'))
                .and_then(|line| line.split_whitespace().next());
            operation = match command {
                Some("pick") => Some("cherry-pick".into()),
                Some("revert") => Some("revert".into()),
                _ => None,
            };
        }
    }
    if operation.is_none() {
        if let Ok(value) =
            std::fs::read_to_string(Path::new(&git_dir).join("codeg-restore-operation"))
        {
            if matches!(value.as_str(), "stash" | "shelf") {
                operation = Some(value);
            }
        }
    }
    if let Some(operation) = operation {
        let files = detect_conflicts(&path).await?;
        return Ok(Some(GitConflictInfo {
            has_conflicts: !files.is_empty(),
            conflicted_files: files,
            operation,
            upstream_commit: None,
        }));
    }
    Ok(None)
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_skip_operation(
    path: String,
    operation: String,
) -> Result<GitWorkflowResult, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    if !matches!(operation.as_str(), "rebase" | "cherry-pick" | "revert") {
        return Err(AppCommandError::invalid_input(
            "This operation cannot be skipped",
        ));
    }
    let state = git_operation_state(path.clone()).await?;
    if state.as_ref().map(|s| s.operation.as_str()) != Some(operation.as_str()) {
        return Err(AppCommandError::invalid_input(
            "The active Git operation changed",
        ));
    }
    let result = output(&path, &[&operation, "--skip"]).await?;
    if operation == "rebase" && result.status.success() {
        super::git_history_edit::cleanup(&path).await?;
    }
    let workflow = operation_result(&path, &operation, result).await?;
    if workflow.conflict.is_none() {
        super::git_smart::after_git_operation(&path).await?;
    }
    Ok(workflow)
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_commit_action(
    path: String,
    commit: String,
    operation: String,
) -> Result<GitWorkflowResult, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    if !matches!(operation.as_str(), "cherry-pick" | "revert") {
        return Err(AppCommandError::invalid_input(
            "Unsupported commit operation",
        ));
    }
    require_idle(&path).await?;
    // Use full object ids only: no options, revision expressions or ranges.
    if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppCommandError::invalid_input(
            "Select one full commit hash",
        ));
    }
    let commit = git(
        &path,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )
    .await?;
    let parents = git(&path, &["rev-list", "--parents", "-n", "1", &commit]).await?;
    if parents.split_whitespace().count() > 2 {
        return Err(AppCommandError::invalid_input(
            "Merge commits require a mainline parent; select a non-merge commit",
        ));
    }
    // Avoid carrying a user's staged work into the newly created commit.
    if !git(&path, &["status", "--porcelain", "--untracked-files=no"])
        .await?
        .is_empty()
    {
        return Err(AppCommandError::invalid_input(
            "Commit or shelve tracked changes before this operation",
        ));
    }
    let result = output(&path, &[&operation, &commit]).await?;
    operation_result(&path, &operation, result).await
}

#[derive(Debug, Serialize)]
pub struct GitAmendInfo {
    pub head: String,
    pub message: String,
    pub published: bool,
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_amend_info(path: String) -> Result<GitAmendInfo, AppCommandError> {
    let head = git(&path, &["rev-parse", "--verify", "HEAD"]).await?;
    let message = git(&path, &["log", "-1", "--format=%B", &head]).await?;
    let published = !git(
        &path,
        &[
            "for-each-ref",
            "--contains",
            &head,
            "--format=%(refname)",
            "refs/remotes/",
        ],
    )
    .await?
    .is_empty();
    Ok(GitAmendInfo {
        head,
        message,
        published,
    })
}

#[derive(Debug, Serialize)]
pub struct GitAmendResult {
    pub head: String,
    pub index_warning: Option<String>,
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_amend(
    path: String,
    message: String,
    files: Vec<String>,
    expected_head: String,
) -> Result<GitAmendResult, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    require_idle(&path).await?;
    validate_files(&files)?;
    if message.trim().is_empty() {
        return Err(AppCommandError::invalid_input("Commit message is required"));
    }
    let verify_head = || async {
        if git(&path, &["rev-parse", "--verify", "HEAD"]).await? != expected_head {
            return Err(AppCommandError::invalid_input(
                "HEAD changed; review the latest commit again",
            ));
        }
        Ok::<(), AppCommandError>(())
    };
    verify_head().await?;
    // Build from HEAD in an isolated index: unchecked staged files must never
    // be swept into an amend, and a failing hook must leave the real index alone.
    let temp = tempfile::tempdir().map_err(AppCommandError::io)?;
    let index = temp.path().join("index");
    let run = |args: Vec<String>| {
        let index = index.clone();
        let path = path.clone();
        async move {
            let result = crate::commands::git_console::output(
                crate::process::tokio_command("git")
                    .args(args)
                    .current_dir(path)
                    .env("GIT_INDEX_FILE", index)
                    .env("GIT_EDITOR", "true")
                    .env("GIT_TERMINAL_PROMPT", "0"),
            )
            .await
            .map_err(AppCommandError::io)?;
            checked(result)
        }
    };
    run(vec!["read-tree".into(), expected_head.clone()]).await?;
    let specs: Vec<String> = files
        .iter()
        .map(|file| super::folders::to_git_literal_pathspec(file))
        .collect();
    if !specs.is_empty() {
        let mut args = vec!["add".into(), "-A".into(), "--".into()];
        args.extend(specs.iter().cloned());
        run(args).await?;
    }
    verify_head().await?;
    run(vec![
        "commit".into(),
        "--amend".into(),
        "-m".into(),
        message,
    ])
    .await?;
    let head = git(&path, &["rev-parse", "--verify", "HEAD"]).await?;
    let mut warning = None;
    if !specs.is_empty() {
        let mut args = vec!["reset", "--quiet", "HEAD", "--"];
        args.extend(specs.iter().map(String::as_str));
        if let Err(error) = git(&path, &args).await {
            warning = Some(error.to_string());
        }
    }
    Ok(GitAmendResult {
        head,
        index_warning: warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        git(path, &["init", "-b", "main"]).await.unwrap();
        git(path, &["config", "user.name", "Workflow Test"])
            .await
            .unwrap();
        git(path, &["config", "user.email", "workflow@example.test"])
            .await
            .unwrap();
        for file in ["a.txt", "b.txt"] {
            std::fs::write(dir.path().join(file), "base\n").unwrap();
        }
        git(path, &["add", "."]).await.unwrap();
        git(path, &["commit", "-m", "initial"]).await.unwrap();
        dir
    }

    #[tokio::test]
    async fn amend_only_takes_checked_files_and_preserves_unchecked_index() {
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("a.txt"), "selected\n").unwrap();
        std::fs::write(dir.path().join("b.txt"), "staged\n").unwrap();
        git(&path, &["add", "b.txt"]).await.unwrap();
        std::fs::write(dir.path().join("b.txt"), "newer unstaged\n").unwrap();
        let index = git(&path, &["show", ":b.txt"]).await.unwrap();
        let info = git_amend_info(path.clone()).await.unwrap();
        let result = git_amend(
            path.clone(),
            "updated".into(),
            vec!["a.txt".into()],
            info.head.clone(),
        )
        .await
        .unwrap();
        assert_ne!(result.head, info.head);
        assert_eq!(
            git(&path, &["show", "HEAD:a.txt"]).await.unwrap(),
            "selected"
        );
        assert_eq!(git(&path, &["show", "HEAD:b.txt"]).await.unwrap(), "base");
        assert_eq!(git(&path, &["show", ":b.txt"]).await.unwrap(), index);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("b.txt")).unwrap(),
            "newer unstaged\n"
        );
        assert!(git(&path, &["diff", "HEAD", "--", "a.txt"])
            .await
            .unwrap()
            .is_empty());
        assert!(git_amend(path, "stale".into(), vec![], info.head)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn message_only_amend_preserves_staged_files() {
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("b.txt"), "staged\n").unwrap();
        git(&path, &["add", "b.txt"]).await.unwrap();
        let info = git_amend_info(path.clone()).await.unwrap();
        let tree = git(&path, &["rev-parse", "HEAD^{tree}"]).await.unwrap();
        git_amend(path.clone(), "message only".into(), vec![], info.head)
            .await
            .unwrap();
        assert_eq!(
            git(&path, &["rev-parse", "HEAD^{tree}"]).await.unwrap(),
            tree
        );
        assert_eq!(git(&path, &["show", ":b.txt"]).await.unwrap(), "staged");
    }

    #[tokio::test]
    async fn cherry_pick_and_revert_round_trip_with_conflict_recovery() {
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        git(&path, &["checkout", "-b", "feature"]).await.unwrap();
        std::fs::write(dir.path().join("a.txt"), "feature\n").unwrap();
        git(&path, &["commit", "-am", "feature"]).await.unwrap();
        let commit = git(&path, &["rev-parse", "HEAD"]).await.unwrap();
        git(&path, &["checkout", "main"]).await.unwrap();
        assert!(
            git_commit_action(path.clone(), commit.clone(), "cherry-pick".into())
                .await
                .unwrap()
                .conflict
                .is_none()
        );
        assert_eq!(
            git(&path, &["show", "HEAD:a.txt"]).await.unwrap(),
            "feature"
        );
        assert!(
            git_commit_action(path.clone(), commit.clone(), "revert".into())
                .await
                .unwrap()
                .conflict
                .is_none()
        );
        assert_eq!(git(&path, &["show", "HEAD:a.txt"]).await.unwrap(), "base");
        std::fs::write(dir.path().join("a.txt"), "different\n").unwrap();
        git(&path, &["commit", "-am", "different"]).await.unwrap();
        let result = git_commit_action(path.clone(), commit, "cherry-pick".into())
            .await
            .unwrap();
        assert_eq!(result.conflict.unwrap().conflicted_files, vec!["a.txt"]);
        assert_eq!(
            git_operation_state(path.clone())
                .await
                .unwrap()
                .unwrap()
                .operation,
            "cherry-pick"
        );
        super::super::folders::git_abort_operation(path.clone(), "cherry-pick".into())
            .await
            .unwrap();
        assert!(git_operation_state(path.clone()).await.unwrap().is_none());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "different\n"
        );
    }

    #[tokio::test]
    async fn history_actions_refuse_dirty_work_and_invalid_revisions() {
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        let head = git(&path, &["rev-parse", "HEAD"]).await.unwrap();
        std::fs::write(dir.path().join("b.txt"), "local\n").unwrap();
        git(&path, &["add", "b.txt"]).await.unwrap();
        assert!(
            git_commit_action(path.clone(), head.clone(), "revert".into())
                .await
                .is_err()
        );
        assert!(
            git_commit_action(path.clone(), "--all".into(), "revert".into())
                .await
                .is_err()
        );
        assert_eq!(git(&path, &["rev-parse", "HEAD"]).await.unwrap(), head);
        assert_eq!(git(&path, &["show", ":b.txt"]).await.unwrap(), "local");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_amend_hook_leaves_head_worktree_and_real_index_intact() {
        use std::os::unix::fs::PermissionsExt;
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("a.txt"), "selected\n").unwrap();
        std::fs::write(dir.path().join("b.txt"), "staged\n").unwrap();
        git(&path, &["add", "b.txt"]).await.unwrap();
        let hook = dir.path().join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        let head = git(&path, &["rev-parse", "HEAD"]).await.unwrap();
        let index = git(&path, &["ls-files", "--stage"]).await.unwrap();
        assert!(git_amend(
            path.clone(),
            "refused".into(),
            vec!["a.txt".into()],
            head.clone()
        )
        .await
        .is_err());
        assert_eq!(git(&path, &["rev-parse", "HEAD"]).await.unwrap(), head);
        assert_eq!(git(&path, &["ls-files", "--stage"]).await.unwrap(), index);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "selected\n"
        );
    }
}
