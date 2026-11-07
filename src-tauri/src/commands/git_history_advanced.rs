use super::git_history::resolve_commit;
use super::git_workflow::{checked, git, output, require_idle, validate_files, HISTORY_WRITE};
use crate::app_error::AppCommandError;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRequest {
    pub operation: String,
    pub commits: Vec<String>,
    pub expected_head: Option<String>,
    pub mainline: Option<u32>,
    #[serde(default)]
    pub allow_published: bool,
    #[serde(default)]
    pub messages: HashMap<String, String>,
    pub files: Option<Vec<String>>,
    pub second_message: Option<String>,
}
async fn parents(path: &str, hash: &str) -> Result<Vec<String>, AppCommandError> {
    Ok(git(path, &["show", "-s", "--format=%P", hash])
        .await?
        .split_whitespace()
        .map(str::to_string)
        .collect())
}
async fn new_commit(
    path: &str,
    old: &str,
    tree: &str,
    parents: &[String],
    message: &str,
) -> Result<String, AppCommandError> {
    if message.trim().is_empty() || message.contains('\0') {
        return Err(AppCommandError::invalid_input(
            "Enter a nonempty commit message",
        ));
    }
    let metadata = git(path, &["show", "-s", "--format=%an%x00%ae%x00%aI", old]).await?;
    let fields: Vec<_> = metadata.split('\0').collect();
    if fields.len() != 3 {
        return Err(AppCommandError::invalid_input("Invalid author metadata"));
    }
    let temp = tempfile::NamedTempFile::new().map_err(AppCommandError::io)?;
    std::fs::write(temp.path(), message).map_err(AppCommandError::io)?;
    let mut command = crate::process::tokio_command("git");
    command.args(["commit-tree", tree, "-F"]).arg(temp.path());
    for parent in parents {
        command.args(["-p", parent]);
    }
    command
        .current_dir(path)
        .env("GIT_AUTHOR_NAME", fields[0])
        .env("GIT_AUTHOR_EMAIL", fields[1])
        .env("GIT_AUTHOR_DATE", fields[2]);
    checked(
        super::git_console::output(&mut command)
            .await
            .map_err(AppCommandError::io)?,
    )
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_history_advanced(
    path: String,
    request: HistoryRequest,
) -> Result<serde_json::Value, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    if request.commits.is_empty() || request.commits.len() > 100 {
        return Err(AppCommandError::invalid_input("Select 1–100 commits"));
    }
    let mut commits = Vec::new();
    for hash in &request.commits {
        let full = resolve_commit(&path, hash).await?;
        if commits.contains(&full) {
            return Err(AppCommandError::invalid_input("Duplicate commit"));
        }
        commits.push(full);
    }
    let head = resolve_commit(&path, "HEAD").await?;
    if request.operation == "inspect" {
        let mut rows = Vec::new();
        let mut published = false;
        for hash in commits {
            let ps = parents(&path, &hash).await?;
            let message = git(&path, &["show", "-s", "--format=%B", &hash]).await?;
            let raw = git(
                &path,
                &[
                    "diff-tree",
                    "--root",
                    "--no-commit-id",
                    "--name-only",
                    "--no-renames",
                    "-r",
                    "-z",
                    &hash,
                ],
            )
            .await?;
            published |= !git(
                &path,
                &[
                    "for-each-ref",
                    "--count=1",
                    "--format=%(refname)",
                    &format!("--contains={hash}"),
                    "refs/remotes",
                ],
            )
            .await?
            .is_empty();
            rows.push(serde_json::json!({"hash":hash,"parents":ps,"message":message,"files":raw.split('\0').filter(|s|!s.is_empty()).collect::<Vec<_>>()}));
        }
        return Ok(serde_json::json!({"head":head,"commits":rows,"published":published}));
    }
    require_idle(&path).await?;
    if request.expected_head.as_deref() != Some(head.as_str()) {
        return Err(AppCommandError::invalid_input(
            "HEAD changed; reopen the review",
        ));
    }
    if !git(&path, &["status", "--porcelain"]).await?.is_empty() {
        return Err(AppCommandError::invalid_input(
            "Commit, shelve or stash all changes first",
        ));
    }
    if ["cherry-pick", "revert"].contains(&request.operation.as_str()) {
        let mut has_merge = false;
        let mut has_nonmerge = false;
        let mut has_root = false;
        for hash in &commits {
            let ps = parents(&path, hash).await?;
            has_root |= ps.is_empty();
            if ps.len() <= 1 {
                has_nonmerge = true;
            }
            if ps.len() > 1 {
                has_merge = true;
                if request
                    .mainline
                    .is_none_or(|n| n == 0 || n as usize > ps.len())
                {
                    return Err(AppCommandError::invalid_input(
                        "Choose an existing mainline parent for every selected merge",
                    ));
                }
            }
        }
        if has_merge && has_root {
            return Err(AppCommandError::invalid_input(
                "Apply root commits separately from merge commits",
            ));
        }
        if has_merge && has_nonmerge && request.mainline.is_some_and(|n| n > 1) {
            return Err(AppCommandError::invalid_input(
                "For mixed merge/non-merge batches use parent 1; apply parent 2+ merges separately",
            ));
        }
        let mut args = vec![request.operation.clone()];
        if has_merge {
            args.extend(["--mainline".into(), request.mainline.unwrap().to_string()]);
        }
        args.extend(commits);
        let refs: Vec<_> = args.iter().map(String::as_str).collect();
        let result = super::git_workflow::operation_result(
            &path,
            &request.operation,
            output(&path, &refs).await?,
        )
        .await?;
        return serde_json::to_value(result)
            .map_err(|e| AppCommandError::invalid_input(e.to_string()));
    }
    if !["reword-merges", "split"].contains(&request.operation.as_str()) {
        return Err(AppCommandError::invalid_input(
            "Unsupported history operation",
        ));
    }
    let branch = git(&path, &["symbolic-ref", "--quiet", "HEAD"]).await?;
    let mut affected = HashSet::new();
    for hash in &commits {
        git(&path, &["merge-base", "--is-ancestor", hash, &head]).await?;
        if !request.allow_published
            && !git(
                &path,
                &[
                    "for-each-ref",
                    "--count=1",
                    "--format=%(refname)",
                    &format!("--contains={hash}"),
                    "refs/remotes",
                ],
            )
            .await?
            .is_empty()
        {
            return Err(AppCommandError::invalid_input(
                "Confirm rewriting published history",
            ));
        }
        affected.insert(hash.clone());
        for descendant in git(
            &path,
            &["rev-list", "--ancestry-path", &format!("{hash}..{head}")],
        )
        .await?
        .lines()
        {
            affected.insert(descendant.to_string());
        }
    }
    if affected.len() > 100 {
        return Err(AppCommandError::invalid_input(
            "Rewrite at most 100 affected commits",
        ));
    }
    let ordered = git(&path, &["rev-list", "--reverse", "--topo-order", &head]).await?;
    let backup = format!("refs/codeg/history-backups/{}", uuid::Uuid::new_v4());
    git(&path, &["update-ref", &backup, &head, ""]).await?;
    let mut replacements = HashMap::<String, String>::new();
    for old in ordered.lines().filter(|h| affected.contains(*h)) {
        let old_parents = parents(&path, old).await?;
        let ps: Vec<_> = old_parents
            .iter()
            .map(|p| replacements.get(p).unwrap_or(p).clone())
            .collect();
        let tree = git(&path, &["rev-parse", &format!("{old}^{{tree}}")]).await?;
        let original = git(&path, &["show", "-s", "--format=%B", old]).await?;
        let message = request.messages.get(old).unwrap_or(&original);
        let new = if request.operation == "split" && commits.iter().any(|h| h == old) {
            if commits.len() != 1 || ps.len() != 1 {
                return Err(AppCommandError::invalid_input(
                    "Split one non-root, non-merge commit",
                ));
            }
            let files = request.files.clone().unwrap_or_default();
            validate_files(&files)?;
            if files.is_empty() {
                return Err(AppCommandError::invalid_input(
                    "Select files for the first commit",
                ));
            }
            let temp = tempfile::tempdir().map_err(AppCommandError::io)?;
            let index = temp.path().join("index");
            let result = super::git_console::output(
                crate::process::tokio_command("git")
                    .args(["read-tree", &old_parents[0]])
                    .env("GIT_INDEX_FILE", &index)
                    .current_dir(&path),
            )
            .await
            .map_err(AppCommandError::io)?;
            checked(result)?;
            let mut args = vec![
                "diff",
                "--binary",
                "--no-ext-diff",
                "--no-textconv",
                &old_parents[0],
                old,
                "--",
            ];
            let specs: Vec<_> = files.iter().map(|f| format!(":(literal){f}")).collect();
            args.extend(specs.iter().map(String::as_str));
            let patch = output(&path, &args).await?;
            if !patch.status.success() {
                checked(patch)?;
                return Err(AppCommandError::invalid_input("Cannot read split patch"));
            }
            let patch_file = temp.path().join("split.patch");
            std::fs::write(&patch_file, &patch.stdout).map_err(AppCommandError::io)?;
            checked(
                super::git_console::output(
                    crate::process::tokio_command("git")
                        .args(["apply", "--cached", "--whitespace=nowarn"])
                        .arg(&patch_file)
                        .env("GIT_INDEX_FILE", &index)
                        .current_dir(&path),
                )
                .await
                .map_err(AppCommandError::io)?,
            )?;
            let first_tree = checked(
                super::git_console::output(
                    crate::process::tokio_command("git")
                        .arg("write-tree")
                        .env("GIT_INDEX_FILE", &index)
                        .current_dir(&path),
                )
                .await
                .map_err(AppCommandError::io)?,
            )?;
            let parent_tree = git(
                &path,
                &["rev-parse", &format!("{}^{{tree}}", old_parents[0])],
            )
            .await?;
            if first_tree == tree || first_tree == parent_tree {
                return Err(AppCommandError::invalid_input("Both split commits must contain changes; select a proper subset of changed files"));
            }
            let first = new_commit(&path, old, &first_tree, &ps, message).await?;
            new_commit(
                &path,
                old,
                &tree,
                &[first],
                request.second_message.as_deref().unwrap_or(""),
            )
            .await?
        } else {
            new_commit(&path, old, &tree, &ps, message).await?
        };
        replacements.insert(old.into(), new);
    }
    let new_head = replacements
        .get(&head)
        .ok_or_else(|| AppCommandError::invalid_input("No rewritten HEAD"))?;
    if git(&path, &["rev-parse", &format!("{new_head}^{{tree}}")]).await?
        != git(&path, &["rev-parse", "HEAD^{tree}"]).await?
        || git(&path, &["symbolic-ref", "--quiet", "HEAD"]).await? != branch
        || !git(&path, &["status", "--porcelain"]).await?.is_empty()
    {
        return Err(AppCommandError::invalid_input(
            "Workspace changed; rewritten commits were not installed",
        ));
    }
    git(
        &path,
        &[
            "update-ref",
            "-m",
            "codeg history rewrite",
            &branch,
            new_head,
            &head,
        ],
    )
    .await?;
    Ok(serde_json::json!({"recovery_ref":backup,"head":new_head,"conflict":null}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(operation: &str, commits: Vec<String>, head: Option<String>) -> HistoryRequest {
        HistoryRequest {
            operation: operation.into(),
            commits,
            expected_head: head,
            mainline: None,
            allow_published: false,
            messages: HashMap::new(),
            files: None,
            second_message: None,
        }
    }
    async fn fixture() -> (tempfile::TempDir, String, String, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        git(&path, &["init", "-b", "main"]).await.unwrap();
        git(&path, &["config", "user.name", "Original Author"])
            .await
            .unwrap();
        git(&path, &["config", "user.email", "author@example.invalid"])
            .await
            .unwrap();
        std::fs::write(dir.path().join("a"), "base\n").unwrap();
        std::fs::write(dir.path().join("b"), "base\n").unwrap();
        git(&path, &["add", "."]).await.unwrap();
        git(&path, &["commit", "-m", "root"]).await.unwrap();
        let root = resolve_commit(&path, "HEAD").await.unwrap();
        git(&path, &["checkout", "-b", "side"]).await.unwrap();
        std::fs::write(dir.path().join("b"), "side\n").unwrap();
        git(&path, &["commit", "-am", "side"]).await.unwrap();
        git(&path, &["checkout", "main"]).await.unwrap();
        std::fs::write(dir.path().join("a"), "main\n").unwrap();
        std::fs::write(dir.path().join("extra"), "extra\n").unwrap();
        git(&path, &["add", "."]).await.unwrap();
        git(&path, &["commit", "-m", "main change"]).await.unwrap();
        let selected = resolve_commit(&path, "HEAD").await.unwrap();
        git(&path, &["merge", "--no-ff", "-m", "merge side", "side"])
            .await
            .unwrap();
        let merge = resolve_commit(&path, "HEAD").await.unwrap();
        (dir, path, root, selected, merge)
    }
    #[tokio::test]
    async fn rewords_dag_preserving_merge_trees_parents_and_other_branches() {
        let (_dir, path, root, _, head) = fixture().await;
        let tree = git(&path, &["rev-parse", "HEAD^{tree}"]).await.unwrap();
        let side = resolve_commit(&path, "side").await.unwrap();
        git(&path, &["update-ref", "refs/remotes/origin/main", &head])
            .await
            .unwrap();
        assert!(git_history_advanced(
            path.clone(),
            request("reword-merges", vec![root.clone()], Some(head.clone()))
        )
        .await
        .is_err());
        assert_eq!(resolve_commit(&path, "HEAD").await.unwrap(), head);
        let mut r = request("reword-merges", vec![root.clone()], Some(head.clone()));
        r.allow_published = true;
        r.messages.insert(root, "new root".into());
        let result = git_history_advanced(path.clone(), r).await.unwrap();
        assert_ne!(resolve_commit(&path, "HEAD").await.unwrap(), head);
        assert_eq!(
            git(&path, &["rev-parse", "HEAD^{tree}"]).await.unwrap(),
            tree
        );
        assert_eq!(parents(&path, "HEAD").await.unwrap().len(), 2);
        assert_eq!(resolve_commit(&path, "side").await.unwrap(), side);
        assert_eq!(
            git(&path, &["rev-list", "--count", "HEAD"]).await.unwrap(),
            "4"
        );
        assert_eq!(
            resolve_commit(&path, result["recovery_ref"].as_str().unwrap())
                .await
                .unwrap(),
            head
        );
        assert!(git(&path, &["status", "--porcelain"])
            .await
            .unwrap()
            .is_empty());
    }
    #[tokio::test]
    async fn splits_older_commit_and_preserves_merge_descendant_and_rejects_dirty_stale() {
        let (dir, path, _, selected, head) = fixture().await;
        let tree = git(&path, &["rev-parse", "HEAD^{tree}"]).await.unwrap();
        let mut r = request("split", vec![selected.clone()], Some(head.clone()));
        r.files = Some(vec!["a".into()]);
        r.messages.insert(selected.clone(), "first half".into());
        r.second_message = Some("second half".into());
        let result = git_history_advanced(path.clone(), r).await.unwrap();
        assert!(result["recovery_ref"].is_string());
        assert_eq!(
            git(&path, &["rev-parse", "HEAD^{tree}"]).await.unwrap(),
            tree
        );
        assert_eq!(parents(&path, "HEAD").await.unwrap().len(), 2);
        assert_eq!(
            git(&path, &["rev-list", "--count", "HEAD"]).await.unwrap(),
            "5"
        );
        assert_eq!(
            git(&path, &["log", "-1", "--format=%s", "HEAD^1"])
                .await
                .unwrap(),
            "second half"
        );
        assert_eq!(
            git(&path, &["log", "-1", "--format=%s", "HEAD^1^1"])
                .await
                .unwrap(),
            "first half"
        );
        assert!(git_history_advanced(
            path.clone(),
            request("reword-merges", vec![selected], Some(head))
        )
        .await
        .is_err());
        let current = resolve_commit(&path, "HEAD").await.unwrap();
        std::fs::write(dir.path().join("untracked"), "keep").unwrap();
        assert!(git_history_advanced(
            path.clone(),
            request(
                "reword-merges",
                vec![current.clone()],
                Some(current.clone())
            )
        )
        .await
        .is_err());
        assert_eq!(resolve_commit(&path, "HEAD").await.unwrap(), current);
    }
    #[tokio::test]
    async fn merge_mainline_and_batch_revert_use_native_sequencer() {
        let (_dir, path, root, selected, merge) = fixture().await;
        git(&path, &["checkout", "-b", "target", &root])
            .await
            .unwrap();
        let mut pick = request(
            "cherry-pick",
            vec![selected.clone(), merge.clone()],
            Some(root),
        );
        pick.mainline = Some(1);
        let result = git_history_advanced(path.clone(), pick).await.unwrap();
        assert!(result["conflict"].is_null());
        assert_eq!(git(&path, &["show", "HEAD:b"]).await.unwrap(), "side");
        let head = resolve_commit(&path, "HEAD").await.unwrap();
        let missing = request("revert", vec![merge.clone()], Some(head.clone()));
        assert!(git_history_advanced(path.clone(), missing).await.is_err());
        let mut revert = request("revert", vec![merge, selected], Some(head));
        revert.mainline = Some(1);
        let result = git_history_advanced(path.clone(), revert).await.unwrap();
        assert!(result["conflict"].is_null());
        assert_eq!(git(&path, &["show", "HEAD:a"]).await.unwrap(), "base");
        assert_eq!(git(&path, &["show", "HEAD:b"]).await.unwrap(), "base");
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[tokio::test]
    async fn batch_conflict_abort_and_continue_preserve_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        git(&path, &["init", "-b", "main"]).await.unwrap();
        git(&path, &["config", "user.name", "Test"]).await.unwrap();
        git(&path, &["config", "user.email", "test@example.invalid"])
            .await
            .unwrap();
        std::fs::write(dir.path().join("a"), "base\n").unwrap();
        git(&path, &["add", "."]).await.unwrap();
        git(&path, &["commit", "-m", "root"]).await.unwrap();
        git(&path, &["checkout", "-b", "feature"]).await.unwrap();
        std::fs::write(dir.path().join("b"), "first\n").unwrap();
        git(&path, &["add", "."]).await.unwrap();
        git(&path, &["commit", "-m", "first"]).await.unwrap();
        let first = resolve_commit(&path, "HEAD").await.unwrap();
        std::fs::write(dir.path().join("a"), "feature\n").unwrap();
        git(&path, &["commit", "-am", "second"]).await.unwrap();
        let second = resolve_commit(&path, "HEAD").await.unwrap();
        git(&path, &["checkout", "main"]).await.unwrap();
        std::fs::write(dir.path().join("a"), "target\n").unwrap();
        git(&path, &["commit", "-am", "target"]).await.unwrap();
        let head = resolve_commit(&path, "HEAD").await.unwrap();
        for abort in [true, false] {
            let r = HistoryRequest {
                operation: "cherry-pick".into(),
                commits: vec![first.clone(), second.clone()],
                expected_head: Some(head.clone()),
                mainline: None,
                allow_published: false,
                messages: HashMap::new(),
                files: None,
                second_message: None,
            };
            let result = git_history_advanced(path.clone(), r).await.unwrap();
            assert_eq!(result["conflict"]["operation"], "cherry-pick");
            assert!(dir.path().join("b").exists());
            if abort {
                super::super::folders::git_abort_operation(path.clone(), "cherry-pick".into())
                    .await
                    .unwrap();
                assert_eq!(resolve_commit(&path, "HEAD").await.unwrap(), head);
                assert!(!dir.path().join("b").exists());
            } else {
                std::fs::write(dir.path().join("a"), "resolved\n").unwrap();
                git(&path, &["add", "a"]).await.unwrap();
                super::super::folders::git_continue_operation(path.clone(), "cherry-pick".into())
                    .await
                    .unwrap();
                assert_eq!(git(&path, &["show", "HEAD:b"]).await.unwrap(), "first");
                assert_eq!(git(&path, &["show", "HEAD:a"]).await.unwrap(), "resolved");
            }
        }
    }
}
