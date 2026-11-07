//! Immutable revision browsing and explicit detached checkout.
use super::git_history::resolve_commit;
use super::git_workflow::{checked, git, output, require_idle, validate_files, HISTORY_WRITE};
use crate::app_error::AppCommandError;
use serde::Serialize;

#[derive(Serialize)]
pub struct GitRevisionInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
    pub parents: Vec<String>,
    pub children: Vec<String>,
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_revision_info(
    path: String,
    revision: String,
) -> Result<GitRevisionInfo, AppCommandError> {
    let hash = resolve_commit(&path, &revision).await?;
    let raw = git(
        &path,
        &[
            "show",
            "-s",
            "--no-show-signature",
            "--format=%an%x00%aI%x00%P%x00%B",
            &hash,
        ],
    )
    .await?;
    let f: Vec<_> = raw.splitn(4, '\0').collect();
    if f.len() != 4 {
        return Err(AppCommandError::invalid_input(
            "Cannot read commit metadata",
        ));
    }
    let all = git(&path, &["rev-list", "--all", "--children", &hash]).await?;
    let children = all
        .lines()
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            (parts.next() == Some(hash.as_str())).then(|| parts.map(str::to_string).collect())
        })
        .unwrap_or_default();
    Ok(GitRevisionInfo {
        hash,
        author: f[0].into(),
        date: f[1].into(),
        parents: f[2].split_whitespace().map(str::to_string).collect(),
        message: f[3].trim_end().into(),
        children,
    })
}
#[derive(Serialize)]
pub struct GitRevisionTreeEntry {
    pub name: String,
    pub file: String,
    pub kind: String,
    pub oid: String,
    pub mode: String,
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_revision_tree(
    path: String,
    revision: String,
    directory: String,
) -> Result<Vec<GitRevisionTreeEntry>, AppCommandError> {
    if !directory.is_empty() {
        validate_files(std::slice::from_ref(&directory))?;
    }
    let hash = resolve_commit(&path, &revision).await?;
    let spec = if directory.is_empty() {
        hash
    } else {
        format!("{hash}:{directory}")
    };
    let raw = checked(output(&path, &["ls-tree", "-z", &spec]).await?)?;
    Ok(raw
        .split('\0')
        .filter_map(|record| {
            let (meta, name) = record.split_once('\t')?;
            let mut fields = meta.split_whitespace();
            let mode = fields.next()?.to_string();
            let kind = fields.next()?.to_string();
            let oid = fields.next()?.to_string();
            Some(GitRevisionTreeEntry {
                name: name.into(),
                file: if directory.is_empty() {
                    name.into()
                } else {
                    format!("{directory}/{name}")
                },
                kind,
                oid,
                mode,
            })
        })
        .collect())
}
#[derive(Serialize)]
pub struct GitRevisionFile {
    pub hash: String,
    pub file: String,
    pub content: Option<String>,
    pub binary: bool,
    pub size: u64,
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_revision_file(
    path: String,
    revision: String,
    file: String,
) -> Result<GitRevisionFile, AppCommandError> {
    validate_files(std::slice::from_ref(&file))?;
    let hash = resolve_commit(&path, &revision).await?;
    let spec = format!("{hash}:{file}");
    if git(&path, &["cat-file", "-t", &spec]).await? != "blob" {
        return Err(AppCommandError::invalid_input("Select a regular blob"));
    }
    let size = git(&path, &["cat-file", "-s", &spec])
        .await?
        .parse::<u64>()
        .map_err(|e| AppCommandError::invalid_input(e.to_string()))?;
    if size > 5 * 1024 * 1024 {
        return Ok(GitRevisionFile {
            hash,
            file,
            content: None,
            binary: true,
            size,
        });
    }
    let result = output(&path, &["cat-file", "blob", &spec]).await?;
    if !result.status.success() {
        checked(result)?;
        return Err(AppCommandError::invalid_input("Cannot read blob"));
    }
    let binary = result.stdout.contains(&0) || std::str::from_utf8(&result.stdout).is_err();
    let content = (!binary).then(|| String::from_utf8_lossy(&result.stdout).into_owned());
    Ok(GitRevisionFile {
        hash,
        file,
        content,
        binary,
        size,
    })
}
#[derive(Serialize)]
pub struct GitRevisionChange {
    pub file: String,
    pub status: String,
}
#[derive(Serialize)]
pub struct GitRevisionComparison {
    pub base: String,
    pub target: String,
    pub files: Vec<GitRevisionChange>,
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_revision_compare(
    path: String,
    base: String,
    target: String,
) -> Result<GitRevisionComparison, AppCommandError> {
    let base = resolve_commit(&path, &base).await?;
    let target = resolve_commit(&path, &target).await?;
    let raw = git(
        &path,
        &[
            "diff",
            "--no-ext-diff",
            "--no-renames",
            "--name-status",
            "-z",
            &base,
            &target,
            "--",
        ],
    )
    .await?;
    let mut fields = raw.split('\0');
    let mut files = Vec::new();
    while let Some(status) = fields.next() {
        if status.is_empty() {
            break;
        }
        if let Some(file) = fields.next() {
            files.push(GitRevisionChange {
                file: file.into(),
                status: status.into(),
            });
        }
    }
    Ok(GitRevisionComparison {
        base,
        target,
        files,
    })
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_revision_checkout(
    path: String,
    revision: String,
    expected_head: String,
) -> Result<String, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    require_idle(&path).await?;
    let hash = resolve_commit(&path, &revision).await?;
    let head = resolve_commit(&path, "HEAD").await?;
    if head != expected_head {
        return Err(AppCommandError::invalid_input(
            "HEAD changed; refresh before checkout",
        ));
    }
    if !git(&path, &["status", "--porcelain"]).await?.is_empty() {
        return Err(AppCommandError::invalid_input(
            "Commit, shelve or stash changes before checkout",
        ));
    }
    git(&path, &["checkout", "--detach", &hash]).await
}

#[derive(Clone, Copy)]
struct Hunk {
    old: u32,
    old_count: u32,
    new: u32,
    new_count: u32,
}
fn hunks(raw: &str) -> Vec<Hunk> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?m)^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").unwrap()
    });
    RE.captures_iter(raw)
        .map(|c| Hunk {
            old: c[1].parse().unwrap_or(0),
            old_count: c
                .get(2)
                .map(|m| m.as_str().parse().unwrap_or(0))
                .unwrap_or(1),
            new: c[3].parse().unwrap_or(0),
            new_count: c
                .get(4)
                .map(|m| m.as_str().parse().unwrap_or(0))
                .unwrap_or(1),
        })
        .collect()
}
async fn working_hunks(
    path: &str,
    file: &str,
    revision: &str,
) -> Result<Vec<Hunk>, AppCommandError> {
    Ok(hunks(
        &git(
            path,
            &[
                "diff",
                "--no-ext-diff",
                "--no-color",
                "--unified=0",
                revision,
                "--",
                &format!(":(literal){file}"),
            ],
        )
        .await?,
    ))
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_source_line(
    path: String,
    file: String,
    revision: String,
    line: u32,
) -> Result<u32, AppCommandError> {
    validate_files(std::slice::from_ref(&file))?;
    let revision = resolve_commit(&path, &revision).await?;
    let mut delta = 0i64;
    for h in working_hunks(&path, &file, &revision).await? {
        if line < h.old {
            break;
        }
        if h.old_count > 0 && line < h.old.saturating_add(h.old_count) {
            return Ok(h
                .new
                .max(1)
                .saturating_add((line - h.old).min(h.new_count.saturating_sub(1))));
        }
        if (h.old_count == 0 && line > h.old)
            || (h.old_count > 0 && line >= h.old.saturating_add(h.old_count))
        {
            delta += i64::from(h.new_count) - i64::from(h.old_count);
        }
    }
    Ok((i64::from(line) + delta).max(1).min(i64::from(u32::MAX)) as u32)
}
#[derive(Serialize)]
pub struct GitLineHistoryEntry {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_line_history(
    path: String,
    file: String,
    start: u32,
    end: u32,
    revision: Option<String>,
    skip: Option<u32>,
) -> Result<Vec<GitLineHistoryEntry>, AppCommandError> {
    validate_files(std::slice::from_ref(&file))?;
    if start == 0 || end < start {
        return Err(AppCommandError::invalid_input("Select a valid line range"));
    }
    let hash = resolve_commit(&path, revision.as_deref().unwrap_or("HEAD")).await?;
    let mut begin = start;
    let mut finish = end;
    if revision.is_none() {
        let mut delta = 0i64;
        for h in working_hunks(&path, &file, &hash).await? {
            if h.new_count > 0 && start < h.new.saturating_add(h.new_count) && end >= h.new {
                return Err(AppCommandError::invalid_input("The selected range includes uncommitted changes; commit it before tracing line history"));
            }
            if h.new_count == 0 && start <= h.new && end > h.new {
                return Err(AppCommandError::invalid_input(
                    "The selection crosses uncommitted deleted lines; commit first",
                ));
            }
            if (h.new_count == 0 && start > h.new)
                || (h.new_count > 0 && h.new.saturating_add(h.new_count) <= start)
            {
                delta += i64::from(h.old_count) - i64::from(h.new_count);
            }
        }
        begin = (i64::from(start) + delta).max(1) as u32;
        finish = (i64::from(end) + delta).max(1) as u32;
    }
    let range = format!("-L{begin},{finish}:{file}");
    let skip = format!("--skip={}", skip.unwrap_or(0));
    let raw = git(
        &path,
        &[
            "log",
            "--no-patch",
            "--no-show-signature",
            "--format=%H%x00%an%x00%aI%x00%B%x00",
            "--max-count=100",
            &skip,
            &range,
            &hash,
        ],
    )
    .await?;
    let fields: Vec<_> = raw.split('\0').collect();
    Ok(fields
        .chunks(4)
        .filter(|f| f.len() == 4)
        .map(|f| GitLineHistoryEntry {
            hash: f[0].trim().into(),
            author: f[1].into(),
            date: f[2].into(),
            message: f[3].trim_end().into(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn repo() -> (tempfile::TempDir, String) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().to_str().unwrap().to_string();
        git(&p, &["init", "-b", "main"]).await.unwrap();
        git(&p, &["config", "user.name", "Revision"]).await.unwrap();
        git(&p, &["config", "user.email", "test@example.invalid"])
            .await
            .unwrap();
        (d, p)
    }
    async fn commit(p: &str, file: &str, text: &str, message: &str) -> String {
        std::fs::write(std::path::Path::new(p).join(file), text).unwrap();
        git(p, &["add", "."]).await.unwrap();
        git(p, &["commit", "-m", message]).await.unwrap();
        resolve_commit(p, "HEAD").await.unwrap()
    }
    #[tokio::test]
    async fn browse_tree_compare_parents_children_and_safe_checkout() {
        let (d, p) = repo().await;
        let first = commit(&p, "[a].txt", "one\ntwo\n", "first").await;
        std::fs::create_dir(d.path().join("nested")).unwrap();
        let head = commit(&p, "nested/中文.txt", "later", "second").await;
        let info = git_revision_info(p.clone(), first.clone()).await.unwrap();
        assert_eq!(info.children, vec![head.clone()]);
        assert!(info.parents.is_empty());
        let entries = git_revision_tree(p.clone(), head.clone(), "nested".into())
            .await
            .unwrap();
        assert_eq!(entries[0].file, "nested/中文.txt");
        let file = git_revision_file(p.clone(), head.clone(), entries[0].file.clone())
            .await
            .unwrap();
        assert_eq!(file.content.as_deref(), Some("later"));
        let diff = git_revision_compare(p.clone(), first.clone(), head.clone())
            .await
            .unwrap();
        assert_eq!(diff.files.len(), 1);
        assert!(
            git_revision_checkout(p.clone(), first.clone(), "stale".into())
                .await
                .is_err()
        );
        std::fs::write(d.path().join("untracked"), "keep").unwrap();
        assert!(
            git_revision_checkout(p.clone(), first.clone(), head.clone())
                .await
                .is_err()
        );
        std::fs::remove_file(d.path().join("untracked")).unwrap();
        git_revision_checkout(p.clone(), first, head).await.unwrap();
        assert!(git(&p, &["symbolic-ref", "--quiet", "HEAD"]).await.is_err());
        assert!(git_revision_tree(p, "--all".into(), "".into())
            .await
            .is_err());
    }
    #[tokio::test]
    async fn line_history_and_working_line_mapping() {
        let (d, p) = repo().await;
        let root = commit(&p, "a", "one\ntwo\nthree\n", "root").await;
        let change = commit(&p, "a", "one\nchanged\nthree\n", "change second").await;
        commit(&p, "a", "one\nchanged\nTHREE\n", "change third").await;
        let lines = git_line_history(p.clone(), "a".into(), 2, 2, None, None)
            .await
            .unwrap();
        assert_eq!(
            lines.iter().map(|e| e.hash.clone()).collect::<Vec<_>>(),
            vec![change, root]
        );
        std::fs::write(d.path().join("a"), "inserted\none\nchanged\nTHREE\n").unwrap();
        assert_eq!(
            git_source_line(p.clone(), "a".into(), "HEAD".into(), 2)
                .await
                .unwrap(),
            3
        );
        let lines = git_line_history(p.clone(), "a".into(), 3, 3, None, None)
            .await
            .unwrap();
        assert_eq!(lines.len(), 2);
        assert!(git_line_history(p.clone(), "a".into(), 1, 1, None, None)
            .await
            .is_err());
        assert!(git_line_history(p, "../outside".into(), 1, 1, None, None)
            .await
            .is_err());
    }
    #[tokio::test]
    async fn insertion_and_deletion_boundaries_preserve_unchanged_lines() {
        let (d, p) = repo().await;
        commit(&p, "a", "one\ntwo\nthree\n", "root").await;
        std::fs::write(d.path().join("a"), "one\ninserted\ntwo\nthree\n").unwrap();
        assert_eq!(
            git_source_line(p.clone(), "a".into(), "HEAD".into(), 1)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            git_source_line(p.clone(), "a".into(), "HEAD".into(), 2)
                .await
                .unwrap(),
            3
        );
        assert!(git_line_history(p.clone(), "a".into(), 3, 3, None, None)
            .await
            .is_ok());
        std::fs::write(d.path().join("a"), "one\nthree\n").unwrap();
        assert!(git_line_history(p.clone(), "a".into(), 1, 1, None, None)
            .await
            .is_ok());
        assert!(git_line_history(p.clone(), "a".into(), 2, 2, None, None)
            .await
            .is_ok());
        assert!(git_line_history(p, "a".into(), 1, 2, None, None)
            .await
            .is_err());
    }
}
