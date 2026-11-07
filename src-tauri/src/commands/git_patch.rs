use super::git_history::resolve_commit;
use super::git_workflow::{checked, output, require_idle, validate_files, HISTORY_WRITE};
use crate::app_error::AppCommandError;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

#[derive(Serialize)]
pub struct GitFileComparison {
    pub original: String,
    pub modified: String,
    pub original_label: String,
    pub modified_label: String,
}

async fn file_at(path: &str, file: &str, revision: &str) -> Result<String, AppCommandError> {
    if revision == "WORKTREE" {
        let root = std::fs::canonicalize(path).map_err(AppCommandError::io)?;
        let target = super::folders::resolve_tree_path(&root, file)?;
        if !target.exists() {
            return Ok(String::new());
        }
        let canonical = std::fs::canonicalize(&target).map_err(AppCommandError::io)?;
        if !canonical.starts_with(&root) {
            return Err(AppCommandError::invalid_input(
                "File must stay inside the workspace",
            ));
        }
        let bytes = std::fs::read(canonical).map_err(AppCommandError::io)?;
        return text(bytes);
    }
    let commit = resolve_commit(path, revision).await?;
    let spec = format!("{commit}:{file}");
    let exists = output(path, &["cat-file", "-e", &spec]).await?;
    if !exists.status.success() {
        return Ok(String::new());
    }
    let result = output(path, &["show", &spec]).await?;
    if !result.status.success() {
        checked(result)?;
        return Ok(String::new());
    }
    text(result.stdout)
}
fn text(bytes: Vec<u8>) -> Result<String, AppCommandError> {
    if bytes.len() > 5 * 1024 * 1024 || bytes.contains(&0) {
        return Err(AppCommandError::invalid_input(
            "Text comparison supports non-binary files up to 5 MiB",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| AppCommandError::invalid_input("Text comparison requires UTF-8"))
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_compare_file(
    path: String,
    file: String,
    base: String,
    target: String,
) -> Result<GitFileComparison, AppCommandError> {
    validate_files(std::slice::from_ref(&file))?;
    let original = file_at(&path, &file, &base).await?;
    let modified = file_at(&path, &file, &target).await?;
    Ok(GitFileComparison {
        original,
        modified,
        original_label: base,
        modified_label: target,
    })
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_patch_create(path: String, files: Vec<String>) -> Result<String, AppCommandError> {
    validate_files(&files)?;
    if files.is_empty() {
        return Err(AppCommandError::invalid_input("Select files to export"));
    }
    let temp = tempfile::tempdir().map_err(AppCommandError::io)?;
    let index = temp.path().join("index");
    let head = output(&path, &["rev-parse", "--verify", "HEAD"]).await?;
    let tree = if head.status.success() {
        String::from_utf8_lossy(&head.stdout).trim().to_string()
    } else {
        super::git_workflow::git(&path, &["mktree"]).await?
    };
    let read = crate::commands::git_console::output(
        crate::process::tokio_command("git")
            .args(["read-tree", &tree])
            .current_dir(&path)
            .env("GIT_INDEX_FILE", &index),
    )
    .await
    .map_err(AppCommandError::io)?;
    checked(read)?;
    let added = crate::commands::git_console::output(
        crate::process::tokio_command("git")
            .args(["add", "-A", "--"])
            .args(&files)
            .current_dir(&path)
            .env("GIT_INDEX_FILE", &index)
            .env("GIT_LITERAL_PATHSPECS", "1"),
    )
    .await
    .map_err(AppCommandError::io)?;
    checked(added)?;
    let diff = crate::commands::git_console::output(
        crate::process::tokio_command("git")
            .args([
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-color",
                "--binary",
                "--full-index",
                &tree,
            ])
            .current_dir(&path)
            .env("GIT_INDEX_FILE", &index),
    )
    .await
    .map_err(AppCommandError::io)?;
    if !diff.status.success() {
        checked(diff)?;
        return Ok(String::new());
    }
    Ok(String::from_utf8_lossy(&diff.stdout).into_owned())
}

pub(crate) async fn apply_command(
    path: &str,
    patch: &str,
    args: &[&str],
) -> Result<std::process::Output, AppCommandError> {
    if patch.len() > 10 * 1024 * 1024 || patch.trim().is_empty() {
        return Err(AppCommandError::invalid_input(
            "Provide a patch up to 10 MiB",
        ));
    }
    let mut command = crate::process::tokio_command("git");
    command
        .arg("apply")
        .args(args)
        .current_dir(path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let started = std::time::Instant::now();
    let mut child = command.spawn().map_err(AppCommandError::io)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppCommandError::invalid_input("Cannot send patch to Git"))?;
    let bytes = patch.as_bytes().to_vec();
    let writer = tokio::spawn(async move { stdin.write_all(&bytes).await });
    let result = child.wait_with_output().await;
    super::git_console::record(&command, started, &result);
    let result = result.map_err(AppCommandError::io)?;
    let _ = writer.await;
    Ok(result)
}
#[derive(Serialize)]
pub struct GitPatchPreview {
    pub applicable: bool,
    pub summary: String,
    pub error: Option<String>,
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_patch_preview(
    path: String,
    patch: String,
    reverse: bool,
    stage: bool,
) -> Result<GitPatchPreview, AppCommandError> {
    let mut args = vec!["--check", "--whitespace=nowarn"];
    if reverse {
        args.push("--reverse");
    }
    if stage {
        args.push("--index");
    }
    let check = apply_command(&path, &patch, &args).await?;
    let summary = checked(apply_command(&path, &patch, &["--stat"]).await?)?;
    Ok(GitPatchPreview {
        applicable: check.status.success(),
        summary,
        error: (!check.status.success())
            .then(|| String::from_utf8_lossy(&check.stderr).trim().into()),
    })
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_patch_apply(
    path: String,
    patch: String,
    reverse: bool,
    stage: bool,
) -> Result<String, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    require_idle(&path).await?;
    let mut args = vec!["--whitespace=nowarn"];
    if reverse {
        args.push("--reverse");
    }
    if stage {
        args.push("--index");
    }
    let mut check = args.clone();
    check.push("--check");
    checked(apply_command(&path, &patch, &check).await?)?;
    checked(apply_command(&path, &patch, &args).await?)
}

#[cfg(test)]
mod tests {
    use super::super::git_workflow::git;
    use super::*;
    async fn repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        git(&path, &["init", "-b", "main"]).await.unwrap();
        git(&path, &["config", "user.name", "Patch Test"])
            .await
            .unwrap();
        git(&path, &["config", "user.email", "test@example.invalid"])
            .await
            .unwrap();
        std::fs::write(dir.path().join("a"), "base\n").unwrap();
        std::fs::write(dir.path().join("b"), "base\n").unwrap();
        git(&path, &["add", "."]).await.unwrap();
        git(&path, &["commit", "-m", "initial"]).await.unwrap();
        (dir, path)
    }
    #[tokio::test]
    async fn selected_patch_preserves_index_and_round_trips_binary_untracked() {
        let (dir, path) = repo().await;
        std::fs::write(dir.path().join("a"), "selected\n").unwrap();
        std::fs::write(dir.path().join("b"), "unchecked\n").unwrap();
        git(&path, &["add", "b"]).await.unwrap();
        std::fs::write(dir.path().join("[binary]"), [0, 255, 1, 2]).unwrap();
        let index = git(&path, &["ls-files", "--stage"]).await.unwrap();
        let patch = git_patch_create(path.clone(), vec!["a".into(), "[binary]".into()])
            .await
            .unwrap();
        assert!(patch.contains("GIT binary patch"));
        assert!(!patch.contains("b/b"));
        assert_eq!(git(&path, &["ls-files", "--stage"]).await.unwrap(), index);
        git(&path, &["restore", "a"]).await.unwrap();
        std::fs::remove_file(dir.path().join("[binary]")).unwrap();
        assert!(
            git_patch_preview(path.clone(), patch.clone(), false, false)
                .await
                .unwrap()
                .applicable
        );
        git_patch_apply(path.clone(), patch.clone(), false, false)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("[binary]")).unwrap(),
            vec![0, 255, 1, 2]
        );
        assert_eq!(git(&path, &["ls-files", "--stage"]).await.unwrap(), index);
        assert!(
            !git_patch_preview(path.clone(), patch.clone(), false, false)
                .await
                .unwrap()
                .applicable
        );
        git_patch_apply(path.clone(), patch, true, false)
            .await
            .unwrap();
        assert!(!dir.path().join("[binary]").exists());
        let comparison =
            git_compare_file(path.clone(), "b".into(), "HEAD".into(), "WORKTREE".into())
                .await
                .unwrap();
        assert_eq!(comparison.original, "base\n");
        assert_eq!(comparison.modified, "unchecked\n");
        assert!(
            git_compare_file(path, "../outside".into(), "HEAD".into(), "WORKTREE".into())
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn initial_repository_patch_works_and_bad_patch_does_not_mutate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        git(&path, &["init"]).await.unwrap();
        std::fs::write(dir.path().join("new"), "first\n").unwrap();
        let patch = git_patch_create(path.clone(), vec!["new".into()])
            .await
            .unwrap();
        assert!(patch.contains("first"));
        assert!(git(&path, &["ls-files"]).await.unwrap().is_empty());
        assert!(git_patch_apply(path, "not a patch".into(), false, false)
            .await
            .is_err());
    }
}
