//! Bounded, reviewable interactive rebase plans. Only native Git operations are
//! accepted; user input can never add an arbitrary todo command or shell text.
use super::folders::GitConflictInfo;
use super::git_history::resolve_commit;
use super::git_workflow::{checked, git, require_idle, GitWorkflowResult, HISTORY_WRITE};
use crate::app_error::AppCommandError;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
pub struct GitEditCommit {
    pub hash: String,
    pub message: String,
}
#[derive(Serialize)]
pub struct GitHistoryPlan {
    pub head: String,
    pub base: String,
    pub parent: Option<String>,
    pub published: bool,
    pub commits: Vec<GitEditCommit>,
}
#[derive(Deserialize, Serialize)]
pub struct GitHistoryStep {
    pub hash: String,
    pub action: String,
    pub message: Option<String>,
}
#[derive(Serialize)]
pub struct GitHistoryEditResult {
    pub recovery_ref: String,
    pub conflict: Option<GitConflictInfo>,
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_history_plan(
    path: String,
    base: String,
) -> Result<GitHistoryPlan, AppCommandError> {
    let head = resolve_commit(&path, "HEAD").await?;
    let base = resolve_commit(&path, &base).await?;
    git(&path, &["merge-base", "--is-ancestor", &base, &head]).await?;
    let parents = git(&path, &["rev-list", "--parents", "-1", &base]).await?;
    let parent = parents.split_whitespace().nth(1).map(str::to_string);
    let range = parent
        .as_ref()
        .map(|p| format!("{p}..{head}"))
        .unwrap_or_else(|| head.clone());
    let raw = git(
        &path,
        &[
            "log",
            "--reverse",
            "--topo-order",
            "--max-count=101",
            "--format=%H%x00%P%x00%B%x00",
            &range,
            "--",
        ],
    )
    .await?;
    let fields: Vec<_> = raw.split('\0').collect();
    let mut commits = Vec::new();
    let mut previous = parent.clone();
    for fields in fields.chunks(3) {
        if fields.len() < 3 {
            break;
        }
        let hash = fields[0].trim();
        if hash.is_empty() {
            continue;
        }
        let parents: Vec<_> = fields[1].split_whitespace().collect();
        if parents.len() > 1 || parents.first().copied() != previous.as_deref() {
            return Err(AppCommandError::invalid_input(
                "Select a linear history without merge commits (up to 100 commits)",
            ));
        }
        previous = Some(hash.to_string());
        commits.push(GitEditCommit {
            hash: hash.into(),
            message: fields[2].trim_end().into(),
        });
    }
    if commits.is_empty() || commits.len() > 100 || commits[0].hash != base {
        return Err(AppCommandError::invalid_input(
            "Select at most 100 commits beginning at the selected commit",
        ));
    }
    let published = !git(
        &path,
        &[
            "for-each-ref",
            "--count=1",
            "--format=%(refname)",
            &format!("--contains={base}"),
            "refs/remotes",
        ],
    )
    .await?
    .is_empty();
    Ok(GitHistoryPlan {
        head,
        base,
        parent,
        published,
        commits,
    })
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) async fn edit_dir(path: &str) -> Result<PathBuf, AppCommandError> {
    Ok(
        Path::new(&git(path, &["rev-parse", "--absolute-git-dir"]).await?)
            .join("codeg-history-edit"),
    )
}

pub(crate) async fn cleanup(path: &str) -> Result<(), AppCommandError> {
    let dir = edit_dir(path).await?;
    let git_dir = dir
        .parent()
        .ok_or_else(|| AppCommandError::invalid_input("Missing Git directory"))?;
    if !git_dir.join("rebase-merge").exists()
        && !git_dir.join("rebase-apply").exists()
        && dir.exists()
    {
        std::fs::remove_dir_all(dir).map_err(AppCommandError::io)?;
    }
    Ok(())
}

async fn outcome(
    path: &str,
    recovery_ref: String,
    result: std::process::Output,
) -> Result<GitHistoryEditResult, AppCommandError> {
    let pending = super::git_workflow::git_operation_state(path.to_string()).await?;
    if pending
        .as_ref()
        .is_some_and(|state| state.operation == "rebase")
    {
        return Ok(GitHistoryEditResult {
            recovery_ref,
            conflict: pending,
        });
    }
    cleanup(path).await?;
    checked(result)?;
    Ok(GitHistoryEditResult {
        recovery_ref,
        conflict: None,
    })
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_history_edit(
    path: String,
    base: String,
    expected_head: String,
    steps: Vec<GitHistoryStep>,
    allow_published: bool,
) -> Result<GitHistoryEditResult, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    require_idle(&path).await?;
    if !git(&path, &["status", "--porcelain"]).await?.is_empty() {
        return Err(AppCommandError::invalid_input(
            "Commit, shelve or stash all changes first",
        ));
    }
    let plan = git_history_plan(path.clone(), base).await?;
    if plan.head != expected_head {
        return Err(AppCommandError::invalid_input(
            "HEAD changed; reopen the history editor",
        ));
    }
    if plan.published && !allow_published {
        return Err(AppCommandError::invalid_input(
            "Explicitly confirm rewriting published commits",
        ));
    }
    let hashes: HashSet<_> = plan.commits.iter().map(|c| c.hash.as_str()).collect();
    let requested: HashSet<_> = steps.iter().map(|s| s.hash.as_str()).collect();
    if steps.len() != hashes.len() || requested != hashes {
        return Err(AppCommandError::invalid_input(
            "The plan must include each original commit exactly once",
        ));
    }
    let mut kept = false;
    for step in &steps {
        match step.action.as_str() {
            "pick" | "reword" => kept = true,
            "squash" | "fixup" if kept => {}
            "drop" => {}
            _ => {
                return Err(AppCommandError::invalid_input(
                    "Unsupported action or no preceding commit to combine with",
                ))
            }
        }
        if step.action == "reword"
            && step
                .message
                .as_ref()
                .is_none_or(|m| m.trim().is_empty() || m.contains('\0'))
        {
            return Err(AppCommandError::invalid_input(
                "Enter a message for each reworded commit",
            ));
        }
    }
    if !kept {
        return Err(AppCommandError::invalid_input("Keep at least one commit"));
    }
    // Save the old HEAD under a durable ref before history can change.
    let recovery_ref = format!("refs/codeg/history-backups/{}", uuid::Uuid::new_v4());
    git(&path, &["update-ref", &recovery_ref, &plan.head]).await?;
    cleanup(&path).await?;
    let directory = edit_dir(&path).await?;
    std::fs::create_dir_all(&directory).map_err(AppCommandError::io)?;
    let mut todo = String::new();
    for (i, step) in steps.iter().enumerate() {
        let action = if step.action == "reword" {
            "pick"
        } else {
            &step.action
        };
        todo.push_str(&format!("{action} {}\n", step.hash));
        if step.action == "reword" {
            let file = directory.join(format!("message-{i}.txt"));
            std::fs::write(&file, step.message.as_deref().unwrap_or(""))
                .map_err(AppCommandError::io)?;
            todo.push_str(&format!(
                "exec git commit --amend -F {}\n",
                shell_quote(&file.to_string_lossy())
            ));
        }
    }
    let todo_file = directory.join("todo");
    std::fs::write(&todo_file, todo).map_err(AppCommandError::io)?;
    std::fs::write(directory.join("recovery-ref"), &recovery_ref).map_err(AppCommandError::io)?;
    let mut command = crate::process::tokio_command("git");
    command.args([
        "-c",
        "rebase.updateRefs=false",
        "-c",
        "rebase.rescheduleFailedExec=true",
        "-c",
        "rebase.abbreviateCommands=false",
        "-c",
        "rebase.missingCommitsCheck=error",
        "rebase",
        "--interactive",
        "--no-autosquash",
        "--no-autostash",
        "--keep-empty",
    ]);
    if let Some(parent) = &plan.parent {
        command.arg(parent);
    } else {
        command.arg("--root");
    }
    let result = crate::commands::git_console::output(
        command
            .current_dir(&path)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_EDITOR", "true")
            .env(
                "GIT_SEQUENCE_EDITOR",
                "sh -c 'cat \"$CODEG_HISTORY_TODO\" > \"$1\"' --",
            )
            .env("CODEG_HISTORY_TODO", &todo_file),
    )
    .await
    .map_err(AppCommandError::io)?;
    outcome(&path, recovery_ref, result).await
}

pub(crate) async fn continue_rebase(path: &str) -> Result<(), AppCommandError> {
    let result = crate::commands::git_console::output(
        crate::process::tokio_command("git")
            .args(["rebase", "--continue"])
            .current_dir(path)
            .env("GIT_EDITOR", "true"),
    )
    .await
    .map_err(AppCommandError::io)?;
    if result.status.success() {
        cleanup(path).await?;
        super::git_smart::after_git_operation(path).await?;
    }
    checked(result).map(|_| ())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_undo_commit(
    path: String,
    expected_head: String,
    allow_published: bool,
) -> Result<GitWorkflowResult, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    require_idle(&path).await?;
    let head = resolve_commit(&path, "HEAD").await?;
    if head != expected_head {
        return Err(AppCommandError::invalid_input(
            "HEAD changed; refresh first",
        ));
    }
    let plan = git_history_plan(path.clone(), head.clone()).await?;
    if plan.published && !allow_published {
        return Err(AppCommandError::invalid_input(
            "Explicitly confirm rewriting published commits",
        ));
    }
    let parent = resolve_commit(&path, "HEAD^").await?;
    let backup = format!("refs/codeg/history-backups/{}", uuid::Uuid::new_v4());
    git(&path, &["update-ref", &backup, &head]).await?;
    let message = git(&path, &["reset", "--soft", &parent]).await?;
    Ok(GitWorkflowResult {
        message: format!("{message}\nRecovery: {backup}"),
        conflict: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::Builder::new()
            .prefix("codeg history ' ")
            .tempdir()
            .unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        git(&path, &["init", "-b", "main"]).await.unwrap();
        git(&path, &["config", "user.name", "History Test"])
            .await
            .unwrap();
        git(&path, &["config", "user.email", "test@example.invalid"])
            .await
            .unwrap();
        (dir, path)
    }
    async fn commit(path: &str, file: &str, content: &str, message: &str) -> String {
        std::fs::write(Path::new(path).join(file), content).unwrap();
        git(path, &["add", "--", file]).await.unwrap();
        git(path, &["commit", "-m", message]).await.unwrap();
        resolve_commit(path, "HEAD").await.unwrap()
    }
    fn steps(plan: &GitHistoryPlan) -> Vec<GitHistoryStep> {
        plan.commits
            .iter()
            .map(|c| GitHistoryStep {
                hash: c.hash.clone(),
                action: "pick".into(),
                message: Some(c.message.clone()),
            })
            .collect()
    }
    #[tokio::test]
    async fn root_reword_squash_fixup_retains_tree_and_recovery_ref() {
        let (_dir, path) = repo().await;
        let root = commit(&path, "a", "base", "initial").await;
        commit(&path, "b", "second", "second").await;
        commit(&path, "c", "third", "third").await;
        let tree = git(&path, &["rev-parse", "HEAD^{tree}"]).await.unwrap();
        let plan = git_history_plan(path.clone(), root.clone()).await.unwrap();
        let mut actions = steps(&plan);
        actions[0].action = "reword".into();
        actions[0].message = Some("new message; $(not executed) ' \"".into());
        actions[1].action = "squash".into();
        actions[2].action = "fixup".into();
        let result = git_history_edit(path.clone(), root, plan.head.clone(), actions, false)
            .await
            .unwrap();
        assert!(result.conflict.is_none());
        assert_eq!(
            git(&path, &["rev-list", "--count", "HEAD"]).await.unwrap(),
            "1"
        );
        assert_eq!(
            git(&path, &["rev-parse", "HEAD^{tree}"]).await.unwrap(),
            tree
        );
        assert!(git(&path, &["log", "-1", "--format=%B"])
            .await
            .unwrap()
            .starts_with("new message; $(not executed)"));
        assert_eq!(
            resolve_commit(&path, &result.recovery_ref).await.unwrap(),
            plan.head
        );
        assert!(!edit_dir(&path).await.unwrap().exists());
    }
    #[tokio::test]
    async fn drop_then_soft_undo_preserves_local_work() {
        let (dir, path) = repo().await;
        commit(&path, "a", "base", "initial").await;
        let base = commit(&path, "b", "remove", "remove").await;
        let head = commit(&path, "c", "keep", "keep").await;
        let plan = git_history_plan(path.clone(), base.clone()).await.unwrap();
        let mut actions = steps(&plan);
        actions[0].action = "drop".into();
        git_history_edit(path.clone(), base, head, actions, false)
            .await
            .unwrap();
        assert!(!dir.path().join("b").exists());
        assert!(dir.path().join("c").exists());
        std::fs::write(dir.path().join("a"), "staged").unwrap();
        git(&path, &["add", "a"]).await.unwrap();
        std::fs::write(dir.path().join("a"), "unstaged").unwrap();
        let head = resolve_commit(&path, "HEAD").await.unwrap();
        git_undo_commit(path.clone(), head, false).await.unwrap();
        assert_eq!(git(&path, &["show", ":a"]).await.unwrap(), "staged");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a")).unwrap(),
            "unstaged"
        );
        assert!(git(&path, &["show", ":c"]).await.is_ok());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn refused_reword_hook_stays_recoverable_without_conflict_files() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, path) = repo().await;
        let root = commit(&path, "a", "one", "initial").await;
        let plan = git_history_plan(path.clone(), root.clone()).await.unwrap();
        let hook = dir.path().join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut actions = steps(&plan);
        actions[0].action = "reword".into();
        actions[0].message = Some("refused".into());
        let result = git_history_edit(path.clone(), root, plan.head.clone(), actions, false)
            .await
            .unwrap();
        let pending = result.conflict.unwrap();
        assert_eq!(pending.operation, "rebase");
        assert!(!pending.has_conflicts);
        super::super::folders::git_abort_operation(path.clone(), "rebase".into())
            .await
            .unwrap();
        assert_eq!(resolve_commit(&path, "HEAD").await.unwrap(), plan.head);
        assert!(!edit_dir(&path).await.unwrap().exists());
    }

    #[tokio::test]
    async fn validates_plan_head_publication_and_clean_state() {
        let (dir, path) = repo().await;
        let root = commit(&path, "a", "base", "initial").await;
        let plan = git_history_plan(path.clone(), root.clone()).await.unwrap();
        assert!(git_history_edit(
            path.clone(),
            root.clone(),
            "stale".into(),
            steps(&plan),
            false
        )
        .await
        .is_err());
        let mut invalid = steps(&plan);
        invalid[0].action = "exec arbitrary".into();
        assert!(git_history_edit(
            path.clone(),
            root.clone(),
            plan.head.clone(),
            invalid,
            false
        )
        .await
        .is_err());
        git(
            &path,
            &["update-ref", "refs/remotes/origin/main", &plan.head],
        )
        .await
        .unwrap();
        assert!(git_history_edit(
            path.clone(),
            root.clone(),
            plan.head.clone(),
            steps(&plan),
            false
        )
        .await
        .is_err());
        std::fs::write(dir.path().join("untracked"), "keep").unwrap();
        assert!(
            git_history_edit(path.clone(), root, plan.head.clone(), steps(&plan), true)
                .await
                .is_err()
        );
        assert_eq!(resolve_commit(&path, "HEAD").await.unwrap(), plan.head);
    }
    #[tokio::test]
    async fn reordered_conflict_can_abort_and_restart_then_continue_and_skip() {
        let (_dir, path) = repo().await;
        let root = commit(&path, "a", "one\n", "one").await;
        commit(&path, "a", "two\n", "two").await;
        commit(&path, "a", "three\n", "three").await;
        let plan = git_history_plan(path.clone(), root.clone()).await.unwrap();
        let mut actions = steps(&plan);
        actions.swap(1, 2);
        let result = git_history_edit(
            path.clone(),
            root.clone(),
            plan.head.clone(),
            actions,
            false,
        )
        .await
        .unwrap();
        assert!(result.conflict.unwrap().has_conflicts);
        super::super::folders::git_abort_operation(path.clone(), "rebase".into())
            .await
            .unwrap();
        assert_eq!(resolve_commit(&path, "HEAD").await.unwrap(), plan.head);
        assert!(!edit_dir(&path).await.unwrap().exists());
        let mut actions = steps(&plan);
        actions.swap(1, 2);
        git_history_edit(path.clone(), root, plan.head, actions, false)
            .await
            .unwrap();
        super::super::folders::git_resolve_conflict(path.clone(), "a".into(), "three\n".into())
            .await
            .unwrap();
        let _ = super::super::folders::git_continue_operation(path.clone(), "rebase".into()).await;
        if super::super::git_workflow::git_operation_state(path.clone())
            .await
            .unwrap()
            .is_some()
        {
            super::super::git_workflow::git_skip_operation(path.clone(), "rebase".into())
                .await
                .unwrap();
        }
        assert!(
            super::super::git_workflow::git_operation_state(path.clone())
                .await
                .unwrap()
                .is_none()
        );
        assert!(!edit_dir(&path).await.unwrap().exists());
    }
}
