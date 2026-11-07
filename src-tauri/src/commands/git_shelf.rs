//! Repository-local shelves use their own refs, so `git stash clear` cannot
//! discard them. Git's stash machinery preserves binary and untracked files.
use serde::Serialize;

use crate::app_error::AppCommandError;

const PREFIX: &str = "refs/codeg/shelves/";
use super::git_workflow::HISTORY_WRITE;

#[derive(Debug, Serialize)]
pub struct GitShelfEntry {
    pub ref_name: String,
    pub message: String,
    pub date: String,
}

async fn git(path: &str, args: &[&str]) -> Result<String, AppCommandError> {
    let output = crate::commands::git_console::output(
        crate::process::tokio_command("git")
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .current_dir(path),
    )
    .await
    .map_err(AppCommandError::io)?;
    if !output.status.success() {
        return Err(AppCommandError::invalid_input(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn validate_ref(shelf_ref: &str) -> Result<(), AppCommandError> {
    let id = shelf_ref.strip_prefix(PREFIX).unwrap_or_default();
    if uuid::Uuid::parse_str(id).is_err() {
        return Err(AppCommandError::invalid_input("Invalid shelf reference"));
    }
    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_shelf_list(path: String) -> Result<Vec<GitShelfEntry>, AppCommandError> {
    let output = git(
        &path,
        &[
            "for-each-ref",
            "--sort=-creatordate",
            "--format=%(refname)%09%(contents:subject)%09%(creatordate:iso8601)",
            PREFIX,
        ],
    )
    .await?;
    Ok(output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            Some(GitShelfEntry {
                ref_name: parts.next()?.to_string(),
                message: parts
                    .next()?
                    .split_once(": codeg-shelf-")?
                    .1
                    .split_once(": ")?
                    .1
                    .to_string(),
                date: parts.next()?.to_string(),
            })
        })
        .collect())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_shelf_save(
    path: String,
    message: String,
    files: Option<Vec<String>>,
) -> Result<(), AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    if message.trim().is_empty() || message.contains(['\n', '\r', '\t', '\0']) {
        return Err(AppCommandError::invalid_input(
            "A single-line shelf name is required",
        ));
    }
    let id = uuid::Uuid::new_v4();
    let shelf_ref = format!("{PREFIX}{id}");
    // A unique marker distinguishes our entry from other programs' stashes.
    let marker = format!("codeg-shelf-{id}: {}", message.trim());
    super::git_workflow::require_idle(&path).await?;
    let mut args = vec![
        "stash".to_string(),
        "push".to_string(),
        "--include-untracked".to_string(),
        "--message".to_string(),
        marker.clone(),
    ];
    let mut specs = Vec::new();
    let temp = if let Some(files) = &files {
        if files.is_empty() {
            return Err(AppCommandError::invalid_input(
                "Select at least one file to shelve",
            ));
        }
        super::git_workflow::validate_files(files)?;
        specs = files
            .iter()
            .map(|file| super::folders::to_git_literal_pathspec(file))
            .collect();
        args.push("--".to_string());
        args.extend(specs.iter().cloned());
        Some(tempfile::tempdir().map_err(AppCommandError::io)?)
    } else {
        None
    };
    let head = git(&path, &["rev-parse", "--verify", "HEAD"]).await?;
    let mut command = crate::process::tokio_command("git");
    command
        .args(&args)
        .current_dir(&path)
        .env("GIT_TERMINAL_PROMPT", "0");
    if let Some(temp) = &temp {
        // A regular path-scoped stash still snapshots unrelated staged paths.
        // Use an isolated HEAD index so only the requested files enter Shelf.
        let index = temp.path().join("index");
        let result = crate::commands::git_console::output(
            crate::process::tokio_command("git")
                .args(["read-tree", &head])
                .env("GIT_INDEX_FILE", &index)
                .current_dir(&path),
        )
        .await
        .map_err(AppCommandError::io)?;
        super::git_workflow::checked(result)?;
        command.env("GIT_INDEX_FILE", index);
    }
    super::git_workflow::checked(command.output().await.map_err(AppCommandError::io)?)?;
    let list = git(&path, &["stash", "list", "--format=%H%x09%gd%x09%gs"]).await?;
    let entry = list.lines().find_map(|line| {
        let mut parts = line.splitn(3, '\t');
        let oid = parts.next()?;
        let stash_ref = parts.next()?;
        let subject = parts.next()?;
        subject.contains(&marker).then_some((oid, stash_ref))
    });
    let Some((oid, stash_ref)) = entry else {
        return Err(AppCommandError::invalid_input("No changes to shelve"));
    };
    // Persist before removing the stash entry. If either step fails the changes
    // remain recoverable in the stash or the shelf (never delete on failure).
    git(&path, &["update-ref", &shelf_ref, oid, ""]).await?;
    let current = git(&path, &["rev-parse", "--verify", stash_ref]).await?;
    if current == oid {
        git(&path, &["stash", "drop", stash_ref]).await?;
    }
    if !specs.is_empty() {
        let mut args = vec!["reset", "--quiet", &head, "--"];
        args.extend(specs.iter().map(String::as_str));
        git(&path, &args).await?;
    }
    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_shelf_apply(
    path: String,
    shelf_ref: String,
) -> Result<super::git_workflow::GitWorkflowResult, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    validate_ref(&shelf_ref)?;
    super::git_workflow::require_idle(&path).await?;
    // Retain the shelf after applying, including when Git reports conflicts.
    let result = super::git_workflow::output(&path, &["stash", "apply", &shelf_ref]).await?;
    super::git_workflow::operation_result(&path, "shelf", result).await
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_shelf_drop(path: String, shelf_ref: String) -> Result<(), AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    validate_ref(&shelf_ref)?;
    let oid = git(&path, &["rev-parse", "--verify", &shelf_ref]).await?;
    git(&path, &["update-ref", "-d", &shelf_ref, &oid]).await?;
    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_shelf_diff(path: String, shelf_ref: String) -> Result<String, AppCommandError> {
    validate_ref(&shelf_ref)?;
    git(
        &path,
        &[
            "stash",
            "show",
            "--include-untracked",
            "--patch",
            "--no-ext-diff",
            "--no-textconv",
            &shelf_ref,
        ],
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        git(path, &["init"]).await.unwrap();
        git(path, &["config", "user.name", "Shelf Test"])
            .await
            .unwrap();
        git(path, &["config", "user.email", "shelf@example.test"])
            .await
            .unwrap();
        std::fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        git(path, &["add", "."]).await.unwrap();
        git(path, &["commit", "-m", "initial"]).await.unwrap();
        dir
    }

    #[tokio::test]
    async fn shelves_round_trip_binary_untracked_and_staged_files_without_changing_stashes() {
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("tracked.txt"), "ordinary stash\n").unwrap();
        git(&path, &["stash", "push", "-m", "user stash"])
            .await
            .unwrap();
        let original_stashes = git(&path, &["stash", "list", "--format=%H"]).await.unwrap();
        std::fs::write(dir.path().join("tracked.txt"), "shelved\n").unwrap();
        git(&path, &["add", "tracked.txt"]).await.unwrap();
        let binary = [0, 255, 1, 2, 0];
        std::fs::write(dir.path().join("untracked.bin"), binary).unwrap();

        git_shelf_save(path.clone(), "我的工作".into(), None)
            .await
            .unwrap();
        assert!(git(&path, &["status", "--porcelain"])
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            git(&path, &["stash", "list", "--format=%H"]).await.unwrap(),
            original_stashes
        );
        let entries = git_shelf_list(path.clone()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "我的工作");
        let shelf_ref = entries[0].ref_name.clone();
        let diff = git_shelf_diff(path.clone(), shelf_ref.clone())
            .await
            .unwrap();
        assert!(diff.contains("tracked.txt"));
        assert!(diff.contains("untracked.bin"));
        git(&path, &["stash", "clear"]).await.unwrap();
        git_shelf_apply(path.clone(), shelf_ref.clone())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("tracked.txt")).unwrap(),
            "shelved\n"
        );
        assert_eq!(
            std::fs::read(dir.path().join("untracked.bin")).unwrap(),
            binary
        );
        assert_eq!(git_shelf_list(path.clone()).await.unwrap().len(), 1);
        git_shelf_drop(path.clone(), shelf_ref).await.unwrap();
        assert!(git_shelf_list(path).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn clean_worktree_does_not_move_an_existing_stash() {
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("tracked.txt"), "user work\n").unwrap();
        git(&path, &["stash", "push", "-m", "user stash"])
            .await
            .unwrap();
        let before = git(&path, &["stash", "list", "--format=%H"]).await.unwrap();
        assert!(git_shelf_save(path.clone(), "empty".into(), None)
            .await
            .is_err());
        assert_eq!(
            git(&path, &["stash", "list", "--format=%H"]).await.unwrap(),
            before
        );
        assert!(git_shelf_list(path).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_restore_keeps_the_saved_copy() {
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("tracked.txt"), "shelf\n").unwrap();
        git_shelf_save(path.clone(), "work".into(), None)
            .await
            .unwrap();
        let entry = git_shelf_list(path.clone()).await.unwrap().remove(0);
        std::fs::write(dir.path().join("tracked.txt"), "local\n").unwrap();
        assert!(git_shelf_apply(path.clone(), entry.ref_name).await.is_err());
        assert_eq!(git_shelf_list(path).await.unwrap().len(), 1);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("tracked.txt")).unwrap(),
            "local\n"
        );
    }

    #[test]
    fn rejects_refs_outside_the_shelf_namespace() {
        for value in [
            "HEAD",
            "refs/heads/main",
            "refs/codeg/shelves/../../heads/main",
            "--all",
        ] {
            assert!(validate_ref(value).is_err());
        }
        assert!(validate_ref(&format!("{PREFIX}{}", uuid::Uuid::new_v4())).is_ok());
    }

    #[tokio::test]
    async fn selected_shelf_excludes_unchecked_staged_files_and_preserves_their_index() {
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("tracked.txt"), "selected\n").unwrap();
        std::fs::write(dir.path().join("unchecked.txt"), "unchecked staged\n").unwrap();
        git(&path, &["add", "unchecked.txt"]).await.unwrap();
        std::fs::write(dir.path().join("unchecked.txt"), "unchecked newer\n").unwrap();
        std::fs::write(dir.path().join("[literal].txt"), "untracked\n").unwrap();
        git_shelf_save(
            path.clone(),
            "selection".into(),
            Some(vec!["tracked.txt".into(), "[literal].txt".into()]),
        )
        .await
        .unwrap();
        let entry = git_shelf_list(path.clone()).await.unwrap().remove(0);
        let diff = git_shelf_diff(path.clone(), entry.ref_name.clone())
            .await
            .unwrap();
        assert!(diff.contains("tracked.txt"));
        assert!(diff.contains("[literal].txt"));
        assert!(!diff.contains("unchecked.txt"));
        assert_eq!(
            git(&path, &["show", ":unchecked.txt"]).await.unwrap(),
            "unchecked staged"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("unchecked.txt")).unwrap(),
            "unchecked newer\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("tracked.txt")).unwrap(),
            "base\n"
        );
        assert!(!dir.path().join("[literal].txt").exists());
        assert!(git_shelf_save(path.clone(), "empty".into(), Some(vec![]))
            .await
            .is_err());
        git_shelf_apply(path.clone(), entry.ref_name).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("[literal].txt")).unwrap(),
            "untracked\n"
        );
    }

    #[tokio::test]
    async fn restore_conflict_keeps_shelf_and_can_finish_without_committing() {
        let dir = repo().await;
        let path = dir.path().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("tracked.txt"), "shelved\n").unwrap();
        git_shelf_save(path.clone(), "saved".into(), None)
            .await
            .unwrap();
        let entry = git_shelf_list(path.clone()).await.unwrap().remove(0);
        std::fs::write(dir.path().join("tracked.txt"), "branch\n").unwrap();
        git(&path, &["commit", "-am", "branch"]).await.unwrap();
        let head = git(&path, &["rev-parse", "HEAD"]).await.unwrap();
        let result = git_shelf_apply(path.clone(), entry.ref_name).await.unwrap();
        assert_eq!(result.conflict.unwrap().operation, "shelf");
        assert_eq!(git_shelf_list(path.clone()).await.unwrap().len(), 1);
        assert_eq!(
            super::super::git_workflow::git_operation_state(path.clone())
                .await
                .unwrap()
                .unwrap()
                .operation,
            "shelf"
        );
        assert!(
            super::super::folders::git_continue_operation(path.clone(), "shelf".into())
                .await
                .is_err()
        );
        std::fs::write(dir.path().join("tracked.txt"), "resolved\n").unwrap();
        git(&path, &["add", "tracked.txt"]).await.unwrap();
        super::super::folders::git_continue_operation(path.clone(), "shelf".into())
            .await
            .unwrap();
        assert!(
            super::super::git_workflow::git_operation_state(path.clone())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(git(&path, &["rev-parse", "HEAD"]).await.unwrap(), head);
        assert_eq!(git_shelf_list(path).await.unwrap().len(), 1);
    }
}
