use super::git_history::resolve_commit;
use super::git_workbench_store::{directory, read, write};
use super::git_workflow::{
    git, operation_result, output, require_idle, GitWorkflowResult, HISTORY_WRITE,
};
use crate::app_error::AppCommandError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct GitSmartState {
    pub operation: String,
    pub target: String,
    pub saved_ref: String,
    pub stage: String,
    pub branch: Option<String>,
    pub message: String,
}
#[derive(Serialize)]
pub struct GitSmartResult {
    pub message: String,
    pub pending: bool,
    pub saved_ref: Option<String>,
    pub conflict: Option<super::folders::GitConflictInfo>,
    pub operation_failed: bool,
}
fn state_file(path: &str) -> Result<std::path::PathBuf, AppCommandError> {
    Ok(directory(path)?.join("smart.json"))
}
fn store(path: &str, state: &Option<GitSmartState>) -> Result<(), AppCommandError> {
    write(&state_file(path)?, state)
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_smart_state(path: String) -> Result<Option<GitSmartState>, AppCommandError> {
    read(&state_file(&path)?)
}

fn result(
    state: &GitSmartState,
    workflow: GitWorkflowResult,
    pending: bool,
    failed: bool,
) -> GitSmartResult {
    GitSmartResult {
        message: workflow.message,
        pending,
        saved_ref: Some(state.saved_ref.clone()),
        conflict: workflow.conflict,
        operation_failed: failed,
    }
}

pub(crate) async fn resume_core(path: &str) -> Result<GitSmartResult, AppCommandError> {
    let mut state = git_smart_state(path.to_string())
        .await?
        .ok_or_else(|| AppCommandError::not_found("No saved smart operation"))?;
    if super::git_workflow::git_operation_state(path.to_string())
        .await?
        .is_some()
    {
        return Err(AppCommandError::invalid_input(
            "Finish or abort the current Git operation first",
        ));
    }
    if state.stage != "operation" {
        return Err(AppCommandError::invalid_input(
            "Restoration needs manual review. The saved copy remains in Shelf.",
        ));
    }
    let branch = git(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .await
        .ok();
    if branch != state.branch {
        return Err(AppCommandError::invalid_input(
            "The branch changed; return to the operation's branch before restoring",
        ));
    }
    state.stage = "restoring".into();
    store(path, &Some(state.clone()))?;
    let mut applied = output(path, &["stash", "apply", "--index", &state.saved_ref]).await?;
    if !applied.status.success() && git(path, &["status", "--porcelain"]).await?.is_empty() {
        // Index patching can fail before Git attempts the content merge. Retry
        // without --index only when it provably left no changes behind.
        applied = output(path, &["stash", "apply", &state.saved_ref]).await?;
    }
    match operation_result(path, "stash", applied).await {
        Ok(workflow) if workflow.conflict.is_none() => {
            store(path, &None)?;
            Ok(result(&state, workflow, false, false))
        }
        Ok(workflow) => {
            state.stage = "conflicts".into();
            state.message = workflow.message.clone();
            store(path, &Some(state.clone()))?;
            Ok(result(&state, workflow, true, false))
        }
        Err(error) => {
            state.stage = "manual".into();
            state.message = error.to_string();
            store(path, &Some(state.clone()))?;
            Err(error)
        }
    }
}

pub(crate) async fn after_git_operation(path: &str) -> Result<(), AppCommandError> {
    if let Some(state) = git_smart_state(path.to_string()).await? {
        if state.stage == "operation"
            && super::git_workflow::git_operation_state(path.to_string())
                .await?
                .is_none()
        {
            resume_core(path).await?;
        }
    }
    Ok(())
}
pub(crate) fn finished_restore(path: &str) -> Result<(), AppCommandError> {
    let file = state_file(path)?;
    if let Some(state) = read::<Option<GitSmartState>>(&file)? {
        if state.stage == "conflicts" {
            write(&file, &Option::<GitSmartState>::None)?;
        }
    }
    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_smart_resume(
    path: String,
    finish_manual: bool,
) -> Result<GitSmartResult, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    if finish_manual {
        let state = git_smart_state(path.clone())
            .await?
            .ok_or_else(|| AppCommandError::not_found("No pending smart operation"))?;
        if super::git_workflow::git_operation_state(path.clone())
            .await?
            .is_some()
        {
            return Err(AppCommandError::invalid_input(
                "Resolve the active Git operation first",
            ));
        }
        store(&path, &None)?;
        return Ok(GitSmartResult {
            message: "Saved copy retained in Shelf".into(),
            pending: false,
            saved_ref: Some(state.saved_ref),
            conflict: None,
            operation_failed: false,
        });
    }
    resume_core(&path).await
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_smart_run(
    path: String,
    operation: String,
    target: String,
) -> Result<GitSmartResult, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    require_idle(&path).await?;
    if git_smart_state(path.clone()).await?.is_some() {
        return Err(AppCommandError::invalid_input(
            "Finish the pending smart operation first",
        ));
    }
    if !matches!(operation.as_str(), "checkout" | "merge" | "rebase" | "pull") {
        return Err(AppCommandError::invalid_input(
            "Unsupported smart operation",
        ));
    }
    let original_branch = git(&path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .await
        .ok();
    let resolved = if operation == "pull" {
        String::new()
    } else {
        resolve_commit(&path, &target).await?
    };
    if operation == "checkout" {
        if target.starts_with('-') || target.is_empty() {
            return Err(AppCommandError::invalid_input("Select a local branch"));
        }
        git(
            &path,
            &["show-ref", "--verify", &format!("refs/heads/{target}")],
        )
        .await?;
    }
    let mut saved = None;
    if !git(&path, &["status", "--porcelain"]).await?.is_empty() {
        let id = uuid::Uuid::new_v4();
        let saved_ref = format!("refs/codeg/shelves/{id}");
        let message = format!("codeg-shelf-{id}: Smart {operation} {target}");
        git(
            &path,
            &["stash", "push", "--include-untracked", "-m", &message],
        )
        .await?;
        let oid = git(&path, &["rev-parse", "refs/stash"]).await?;
        git(&path, &["update-ref", &saved_ref, &oid]).await?;
        let state = GitSmartState {
            operation: operation.clone(),
            target: target.clone(),
            saved_ref,
            stage: "operation".into(),
            branch: if operation == "checkout" {
                Some(target.clone())
            } else {
                original_branch
            },
            message: String::new(),
        };
        store(&path, &Some(state.clone()))?;
        // Remove only the stash we just created; concurrent external stashes are
        // never dropped. The durable Shelf ref is already in place.
        if git(&path, &["rev-parse", "refs/stash"]).await? == oid {
            git(&path, &["stash", "drop", "stash@{0}"]).await?;
        }
        saved = Some(state);
    }
    let args = match operation.as_str() {
        "checkout" => vec!["checkout", "--no-guess", &target],
        "merge" => vec!["merge", "--no-edit", &resolved],
        "rebase" => vec!["rebase", &resolved],
        _ => vec!["pull", "--no-rebase"],
    };
    let performed = output(&path, &args).await?;
    let failed = !performed.status.success();
    let mut message = String::from_utf8_lossy(if failed {
        &performed.stderr
    } else {
        &performed.stdout
    })
    .trim()
    .to_string();
    if let Some(mut state) = saved {
        if let Some(conflict) = super::git_workflow::git_operation_state(path.clone()).await? {
            return Ok(GitSmartResult {
                message,
                pending: true,
                saved_ref: Some(state.saved_ref),
                conflict: Some(conflict),
                operation_failed: false,
            });
        }
        // A failed checkout normally leaves us on the original branch; record
        // that branch before putting the saved changes back.
        state.branch = git(&path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .await
            .ok();
        store(&path, &Some(state))?;
        let mut restored = resume_core(&path).await?;
        if !restored.message.is_empty() {
            message.push('\n');
            message.push_str(&restored.message);
        }
        restored.message = message;
        restored.operation_failed = failed;
        Ok(restored)
    } else {
        let workflow = operation_result(
            &path,
            if operation == "pull" {
                "merge"
            } else {
                &operation
            },
            performed,
        )
        .await?;
        Ok(GitSmartResult {
            message: workflow.message,
            pending: workflow.conflict.is_some(),
            conflict: workflow.conflict,
            saved_ref: None,
            operation_failed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        git(&path, &["init", "-b", "main"]).await.unwrap();
        git(&path, &["config", "user.name", "Smart Test"])
            .await
            .unwrap();
        git(&path, &["config", "user.email", "test@example.invalid"])
            .await
            .unwrap();
        std::fs::write(dir.path().join("a"), "base\n").unwrap();
        std::fs::write(dir.path().join("b"), "base\n").unwrap();
        git(&path, &["add", "."]).await.unwrap();
        git(&path, &["commit", "-m", "initial"]).await.unwrap();
        git(&path, &["branch", "other"]).await.unwrap();
        (dir, path)
    }
    #[tokio::test]
    async fn checkout_preserves_staged_unstaged_and_untracked_work() {
        let (dir, path) = repo().await;
        std::fs::write(dir.path().join("a"), "staged\n").unwrap();
        git(&path, &["add", "a"]).await.unwrap();
        std::fs::write(dir.path().join("a"), "unstaged\n").unwrap();
        std::fs::write(dir.path().join("new"), "untracked").unwrap();
        let result = git_smart_run(path.clone(), "checkout".into(), "other".into())
            .await
            .unwrap();
        assert!(!result.pending);
        assert_eq!(
            git(&path, &["branch", "--show-current"]).await.unwrap(),
            "other"
        );
        assert_eq!(git(&path, &["show", ":a"]).await.unwrap(), "staged");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a")).unwrap(),
            "unstaged\n"
        );
        assert!(dir.path().join("new").exists());
        assert!(
            git(&path, &["rev-parse", result.saved_ref.as_ref().unwrap()])
                .await
                .is_ok()
        );
        assert!(git_smart_state(path).await.unwrap().is_none());
    }
    #[tokio::test]
    async fn restore_conflict_is_discoverable_and_copy_retained() {
        let (dir, path) = repo().await;
        git(&path, &["checkout", "other"]).await.unwrap();
        std::fs::write(dir.path().join("a"), "other\n").unwrap();
        git(&path, &["commit", "-am", "other"]).await.unwrap();
        git(&path, &["checkout", "main"]).await.unwrap();
        std::fs::write(dir.path().join("a"), "local\n").unwrap();
        let result = git_smart_run(path.clone(), "checkout".into(), "other".into())
            .await
            .unwrap();
        assert!(result.pending);
        assert!(result.conflict.unwrap().has_conflicts);
        assert!(git_smart_state(path.clone()).await.unwrap().is_some());
        super::super::folders::git_resolve_conflict(path.clone(), "a".into(), "resolved\n".into())
            .await
            .unwrap();
        super::super::folders::git_continue_operation(path.clone(), "stash".into())
            .await
            .unwrap();
        assert!(git_smart_state(path.clone()).await.unwrap().is_none());
        assert!(
            git(&path, &["rev-parse", result.saved_ref.as_ref().unwrap()])
                .await
                .is_ok()
        );
    }
    #[tokio::test]
    async fn failed_merge_restores_work_and_reports_failure() {
        let (dir, path) = repo().await;
        std::fs::write(dir.path().join("b"), "saved\n").unwrap();
        let head = git(&path, &["rev-parse", "HEAD"]).await.unwrap();
        let invalid = git_smart_run(path.clone(), "merge".into(), "--all".into()).await;
        assert!(invalid.is_err());
        assert_eq!(git(&path, &["rev-parse", "HEAD"]).await.unwrap(), head);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("b")).unwrap(),
            "saved\n"
        );
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[tokio::test]
    async fn merge_abort_and_continue_restore_saved_index_only_after_operation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        git(&path, &["init", "-b", "main"]).await.unwrap();
        git(&path, &["config", "user.name", "Recovery"])
            .await
            .unwrap();
        git(&path, &["config", "user.email", "test@example.invalid"])
            .await
            .unwrap();
        std::fs::write(dir.path().join("a"), "base\n").unwrap();
        std::fs::write(dir.path().join("b"), "base\n").unwrap();
        git(&path, &["add", "."]).await.unwrap();
        git(&path, &["commit", "-m", "base"]).await.unwrap();
        git(&path, &["checkout", "-b", "other"]).await.unwrap();
        std::fs::write(dir.path().join("a"), "other\n").unwrap();
        git(&path, &["commit", "-am", "other"]).await.unwrap();
        git(&path, &["checkout", "main"]).await.unwrap();
        std::fs::write(dir.path().join("a"), "main\n").unwrap();
        git(&path, &["commit", "-am", "main"]).await.unwrap();
        let old = git(&path, &["rev-parse", "HEAD"]).await.unwrap();
        std::fs::write(dir.path().join("b"), "staged work\n").unwrap();
        git(&path, &["add", "b"]).await.unwrap();
        let result = git_smart_run(path.clone(), "merge".into(), "other".into())
            .await
            .unwrap();
        assert!(result.pending);
        assert_eq!(
            git_smart_state(path.clone()).await.unwrap().unwrap().stage,
            "operation"
        );
        assert_eq!(git(&path, &["show", ":b"]).await.unwrap(), "base");
        super::super::folders::git_abort_operation(path.clone(), "merge".into())
            .await
            .unwrap();
        assert_eq!(git(&path, &["rev-parse", "HEAD"]).await.unwrap(), old);
        assert_eq!(git(&path, &["show", ":b"]).await.unwrap(), "staged work");
        assert!(git_smart_state(path.clone()).await.unwrap().is_none());
        let result = git_smart_run(path.clone(), "merge".into(), "other".into())
            .await
            .unwrap();
        assert!(result.pending);
        super::super::folders::git_resolve_conflict(path.clone(), "a".into(), "resolved\n".into())
            .await
            .unwrap();
        super::super::folders::git_continue_operation(path.clone(), "merge".into())
            .await
            .unwrap();
        assert!(git_smart_state(path.clone()).await.unwrap().is_none());
        assert_eq!(git(&path, &["show", "HEAD:b"]).await.unwrap(), "base");
        assert_eq!(git(&path, &["show", ":b"]).await.unwrap(), "staged work");
    }
}
