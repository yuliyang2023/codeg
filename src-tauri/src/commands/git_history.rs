//! Read-only history queries. Revisions are resolved before use and file filters
//! are literal pathspecs, so search input never becomes a Git option or range.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::folders::{GitLogEntry, GitLogFileChange};
use super::git_workflow::{checked, git, output, validate_files};
use crate::app_error::AppCommandError;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GitHistoryQuery {
    pub limit: Option<u32>,
    pub skip: Option<u32>,
    pub branch: Option<String>,
    pub author: Option<String>,
    pub all_branches: bool,
    pub search: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub since: String,
    pub until: String,
    pub file: String,
    pub follow: bool,
    pub order: String,
    pub first_parent: bool,
    pub no_merges: bool,
    pub revision: String,
}

#[derive(Serialize)]
pub struct GitHistoryEntry {
    #[serde(flatten)]
    pub entry: GitLogEntry,
    pub parents: Vec<String>,
    pub refs: Vec<String>,
    pub is_head: bool,
}

#[derive(Serialize)]
pub struct GitHistoryResult {
    pub entries: Vec<GitHistoryEntry>,
    pub has_upstream: bool,
}

pub(crate) async fn resolve_commit(path: &str, revision: &str) -> Result<String, AppCommandError> {
    if revision.trim().is_empty() || revision.contains('\0') {
        return Err(AppCommandError::invalid_input(
            "Enter a commit, branch or tag",
        ));
    }
    git(
        path,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{revision}^{{commit}}"),
        ],
    )
    .await
}

fn date_arg(value: &str, end: bool) -> Result<String, AppCommandError> {
    if value.len() != 10 || chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
        return Err(AppCommandError::invalid_input(
            "Use valid dates in YYYY-MM-DD format",
        ));
    }
    Ok(format!(
        "{value} {}",
        if end { "23:59:59" } else { "00:00:00" }
    ))
}

fn escape_ere(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        if "\\.^$*+?()[]{}|".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_history(
    path: String,
    query: GitHistoryQuery,
) -> Result<GitHistoryResult, AppCommandError> {
    git(&path, &["rev-parse", "--git-dir"]).await?;
    let head_output = output(&path, &["rev-parse", "--verify", "HEAD"]).await?;
    let head = if head_output.status.success() {
        checked(head_output)?
    } else {
        String::new()
    };
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let skip = query.skip.unwrap_or(0);
    let mut options = vec![
        "log".to_string(),
        "--no-show-signature".to_string(),
        "--no-color".to_string(),
        "--format=%H%x00%h%x00%an%x00%aI%x00%P%x00%B%x00".to_string(),
        format!("--max-count={limit}"),
        format!("--skip={skip}"),
        "--extended-regexp".to_string(),
    ];
    match query.order.as_str() {
        "" | "topo" => options.push("--topo-order".to_string()),
        "date" => options.push("--date-order".to_string()),
        _ => return Err(AppCommandError::invalid_input("Unsupported history order")),
    }
    if query.first_parent {
        options.push("--first-parent".to_string());
    }
    if query.no_merges {
        options.push("--no-merges".to_string());
    }
    if !query.since.is_empty() {
        options.push(format!("--since={}", date_arg(&query.since, false)?));
    }
    if !query.until.is_empty() {
        options.push(format!("--until={}", date_arg(&query.until, true)?));
    }
    if !query.since.is_empty() && !query.until.is_empty() && query.since > query.until {
        return Err(AppCommandError::invalid_input(
            "Start date must not follow end date",
        ));
    }
    if let Some(author) = query.author.as_ref().filter(|a| !a.is_empty()) {
        options.push(format!("--author=^{} <", escape_ere(author)));
    }
    if !query.search.is_empty() {
        let pattern = if query.regex {
            query.search.clone()
        } else {
            escape_ere(&query.search)
        };
        options.push(format!("--grep={pattern}"));
    }
    if !query.case_sensitive {
        options.push("--regexp-ignore-case".to_string());
    }
    // rev-parse HEAD can fail in an unborn repository; explicit invalid revisions
    // still produce an actionable error rather than an empty result.
    if !query.revision.trim().is_empty() {
        options.push(resolve_commit(&path, query.revision.trim()).await?);
    } else if query.all_branches {
        options.extend([
            "--branches".to_string(),
            "--remotes".to_string(),
            "--tags".to_string(),
        ]);
        if !head.is_empty() {
            options.push(head.clone());
        }
    } else if let Some(branch) = query.branch.as_ref().filter(|b| !b.is_empty()) {
        if head.is_empty() && branch == "HEAD" {
            return Ok(GitHistoryResult {
                entries: vec![],
                has_upstream: false,
            });
        }
        options.push(resolve_commit(&path, branch).await?);
    } else if !head.is_empty() {
        options.push(head.clone());
    } else {
        return Ok(GitHistoryResult {
            entries: vec![],
            has_upstream: false,
        });
    }
    let mut paths = vec!["--".to_string()];
    if !query.file.is_empty() {
        validate_files(std::slice::from_ref(&query.file))?;
        paths.push(format!(":(literal){}", query.file));
        // Keep true object parents when filtering paths. Git's default history
        // simplification can otherwise imply a direct edge that does not exist.
        options.push(
            if query.follow {
                "--follow"
            } else {
                "--full-history"
            }
            .to_string(),
        );
    }
    let refs_output = git(
        &path,
        &[
            "for-each-ref",
            "--format=%(objectname)%00%(*objectname)%00%(refname)",
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
    )
    .await?;
    let mut refs: HashMap<String, Vec<String>> = HashMap::new();
    for line in refs_output.lines() {
        let parts: Vec<_> = line.splitn(3, '\0').collect();
        if parts.len() == 3 {
            refs.entry(
                if parts[1].is_empty() {
                    parts[0]
                } else {
                    parts[1]
                }
                .to_string(),
            )
            .or_default()
            .push(
                parts[2]
                    .strip_prefix("refs/heads/")
                    .or_else(|| parts[2].strip_prefix("refs/remotes/"))
                    .map(str::to_string)
                    .unwrap_or_else(|| parts[2].replace("refs/tags/", "tag: ")),
            );
        }
    }
    let mut args = options.clone();
    args.extend(paths.clone());
    let args: Vec<_> = args.iter().map(String::as_str).collect();
    let raw = git(&path, &args).await?;
    let fields: Vec<_> = raw.split('\0').collect();
    let mut entries = Vec::new();
    for fields in fields.chunks(6) {
        if fields.len() < 6 {
            break;
        }
        let hash = fields[0].trim();
        if hash.is_empty() {
            continue;
        }
        entries.push(GitHistoryEntry {
            parents: fields[4].split_whitespace().map(str::to_string).collect(),
            refs: refs.remove(hash).unwrap_or_default(),
            is_head: hash == head,
            entry: GitLogEntry {
                full_hash: hash.to_string(),
                hash: fields[1].to_string(),
                author: fields[2].to_string(),
                date: fields[3].to_string(),
                message: fields[5].trim_end().to_string(),
                files: Vec::<GitLogFileChange>::new(),
                pushed: None,
            },
        });
    }
    // Same filters/order as the page. Taking skip+limit unpublished commits is
    // sufficient to classify every commit in that page, even for sparse filters.
    let mut unpublished: Vec<String> = options
        .into_iter()
        .filter(|a| {
            !a.starts_with("--format=")
                && !a.starts_with("--skip=")
                && !a.starts_with("--max-count=")
        })
        .collect();
    unpublished.extend([
        "--format=%H".to_string(),
        format!("--max-count={}", skip.saturating_add(limit)),
        "--not".to_string(),
        "--remotes".to_string(),
    ]);
    unpublished.extend(paths);
    let args: Vec<_> = unpublished.iter().map(String::as_str).collect();
    let unpublished = git(&path, &args).await?;
    let unpublished: HashSet<_> = unpublished.lines().collect();
    for entry in &mut entries {
        entry.entry.pushed = Some(!unpublished.contains(entry.entry.full_hash.as_str()));
    }
    let has_upstream = refs_output.lines().any(|line| {
        line.split('\0')
            .nth(2)
            .is_some_and(|r| r.starts_with("refs/remotes/"))
    });
    Ok(GitHistoryResult {
        entries,
        has_upstream,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        git(&path, &["init", "-b", "main"]).await.unwrap();
        git(&path, &["config", "user.name", "Test (A)|B"])
            .await
            .unwrap();
        git(&path, &["config", "user.email", "test@example.invalid"])
            .await
            .unwrap();
        (dir, path)
    }
    async fn commit(path: &str, file: &str, content: &str, message: &str) -> String {
        std::fs::write(std::path::Path::new(path).join(file), content).unwrap();
        git(path, &["add", "--", file]).await.unwrap();
        git(path, &["commit", "-m", message]).await.unwrap();
        git(path, &["rev-parse", "HEAD"]).await.unwrap()
    }
    #[tokio::test]
    async fn graph_preserves_merge_parents_head_annotated_tags_and_publication() {
        let (_dir, path) = repo().await;
        let root = commit(&path, "a.txt", "root", "root").await;
        git(&path, &["checkout", "-b", "topic"]).await.unwrap();
        let topic = commit(&path, "b.txt", "topic", "topic").await;
        git(&path, &["checkout", "main"]).await.unwrap();
        let main = commit(&path, "a.txt", "main", "main").await;
        git(&path, &["merge", "--no-ff", "topic", "-m", "merge"])
            .await
            .unwrap();
        let head = git(&path, &["rev-parse", "HEAD"]).await.unwrap();
        git(&path, &["tag", "-a", "v1", "-m", "release"])
            .await
            .unwrap();
        git(&path, &["update-ref", "refs/remotes/origin/main", &root])
            .await
            .unwrap();
        let result = git_history(
            path,
            GitHistoryQuery {
                all_branches: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(result.entries.len(), 4);
        let first = &result.entries[0];
        assert_eq!(first.entry.full_hash, head);
        assert!(first.is_head);
        assert_eq!(first.parents, vec![main, topic]);
        assert!(first.refs.contains(&"tag: v1".into()));
        assert_eq!(first.entry.pushed, Some(false));
        assert_eq!(result.entries.last().unwrap().entry.pushed, Some(true));
        assert!(result.has_upstream);
    }
    #[tokio::test]
    async fn literal_regex_author_and_page_filters_match_before_pagination() {
        let (_dir, path) = repo().await;
        commit(
            &path,
            "a.txt",
            "one",
            "Fix [literal]\n\n__COMMIT_MESSAGE_END__",
        )
        .await;
        commit(&path, "a.txt", "two", "unrelated").await;
        let last = commit(&path, "a.txt", "three", "FIX [literal]").await;
        let query = GitHistoryQuery {
            search: "fix [literal]".into(),
            author: Some("Test (A)|B".into()),
            limit: Some(1),
            ..Default::default()
        };
        let result = git_history(path.clone(), query).await.unwrap();
        assert_eq!(result.entries[0].entry.full_hash, last);
        let result = git_history(
            path.clone(),
            GitHistoryQuery {
                search: "fix [literal]".into(),
                skip: Some(1),
                limit: Some(1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(result.entries[0]
            .entry
            .message
            .contains("__COMMIT_MESSAGE_END__"));
        let result = git_history(
            path.clone(),
            GitHistoryQuery {
                search: "^FIX".into(),
                regex: true,
                case_sensitive: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(result.entries.len(), 1);
        assert!(git_history(
            path,
            GitHistoryQuery {
                search: "[".into(),
                regex: true,
                ..Default::default()
            }
        )
        .await
        .is_err());
    }
    #[tokio::test]
    async fn literal_path_revision_and_option_injection_are_safe() {
        let (_dir, path) = repo().await;
        let first = commit(&path, "[literal].txt", "one", "first").await;
        commit(&path, "other.txt", "two", "second").await;
        let result = git_history(
            path.clone(),
            GitHistoryQuery {
                file: "[literal].txt".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry.full_hash, first);
        let result = git_history(
            path.clone(),
            GitHistoryQuery {
                revision: first[..10].into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(result.entries.len(), 1);
        for revision in ["--all", "main..HEAD", "missing-ref"] {
            assert!(git_history(
                path.clone(),
                GitHistoryQuery {
                    revision: revision.into(),
                    ..Default::default()
                }
            )
            .await
            .is_err());
        }
        assert!(git_history(
            path,
            GitHistoryQuery {
                file: "../outside".into(),
                ..Default::default()
            }
        )
        .await
        .is_err());
    }
    #[tokio::test]
    async fn follows_renames_and_blames_committed_and_local_lines() {
        let (_dir, path) = repo().await;
        let original = commit(&path, "old.txt", "first\nsecond\n", "original").await;
        git(&path, &["mv", "old.txt", "new.txt"]).await.unwrap();
        git(&path, &["commit", "-m", "rename"]).await.unwrap();
        let result = git_history(
            path.clone(),
            GitHistoryQuery {
                file: "new.txt".into(),
                follow: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[1].entry.full_hash, original);
        std::fs::write(
            std::path::Path::new(&path).join("new.txt"),
            "first\nlocal with spaces  \n",
        )
        .unwrap();
        let lines = git_blame(path.clone(), "new.txt".into(), None)
            .await
            .unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].commit, original);
        assert_eq!(lines[0].file, "old.txt");
        assert!(lines[1].uncommitted);
        assert_eq!(lines[1].content, "local with spaces  ");
        let lines = git_blame(path.clone(), "new.txt".into(), Some("HEAD".into()))
            .await
            .unwrap();
        assert!(!lines[1].uncommitted);
        assert_eq!(lines[1].content, "second");
        assert!(git_blame(path, "../outside".into(), None).await.is_err());
    }

    #[tokio::test]
    async fn empty_repository_and_invalid_dates() {
        let (_dir, path) = repo().await;
        assert!(git_history(path.clone(), GitHistoryQuery::default())
            .await
            .unwrap()
            .entries
            .is_empty());
        commit(&path, "a", "test", "initial").await;
        for date in ["2026-02-31", "yesterday", "2026-13-01"] {
            assert!(git_history(
                path.clone(),
                GitHistoryQuery {
                    since: date.into(),
                    ..Default::default()
                }
            )
            .await
            .is_err());
        }
        let result = git_history(
            path,
            GitHistoryQuery {
                since: "2099-01-01".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(result.entries.is_empty());
    }
}

#[derive(Serialize)]
pub struct GitBlameLine {
    pub commit: String,
    pub original_line: u32,
    pub line: u32,
    pub author: String,
    pub date: String,
    pub summary: String,
    pub file: String,
    pub content: String,
    pub uncommitted: bool,
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_blame(
    path: String,
    file: String,
    revision: Option<String>,
) -> Result<Vec<GitBlameLine>, AppCommandError> {
    validate_files(std::slice::from_ref(&file))?;
    let mut args = vec![
        "-c".to_string(),
        "core.quotePath=false".to_string(),
        "blame".to_string(),
        "--line-porcelain".to_string(),
    ];
    if let Some(revision) = revision.filter(|r| !r.is_empty()) {
        args.push(resolve_commit(&path, &revision).await?);
    }
    args.extend(["--".to_string(), file.clone()]);
    let args: Vec<_> = args.iter().map(String::as_str).collect();
    let result = output(&path, &args).await?;
    if !result.status.success() {
        return checked(result).map(|_| Vec::new());
    }
    let raw = String::from_utf8_lossy(&result.stdout);
    let mut lines = Vec::new();
    let mut current: Option<GitBlameLine> = None;
    for line in raw.lines() {
        if let Some(content) = line.strip_prefix('\t') {
            if let Some(mut item) = current.take() {
                item.content = content.to_string();
                lines.push(item);
            }
        } else if let Some(author) = line.strip_prefix("author ") {
            if let Some(item) = &mut current {
                item.author = author.to_string();
            }
        } else if let Some(date) = line.strip_prefix("author-time ") {
            if let Some(item) = &mut current {
                item.date = date.to_string();
            }
        } else if let Some(summary) = line.strip_prefix("summary ") {
            if let Some(item) = &mut current {
                item.summary = summary.to_string();
            }
        } else if let Some(name) = line.strip_prefix("filename ") {
            if let Some(item) = &mut current {
                item.file = decode_blame_path(name);
            }
        } else {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() >= 3
                && (fields[0].len() == 40 || fields[0].len() == 64)
                && fields[0].bytes().all(|b| b.is_ascii_hexdigit())
            {
                current = Some(GitBlameLine {
                    commit: fields[0].into(),
                    original_line: fields[1].parse().unwrap_or(0),
                    line: fields[2].parse().unwrap_or(0),
                    author: String::new(),
                    date: String::new(),
                    summary: String::new(),
                    file: file.clone(),
                    content: String::new(),
                    uncommitted: fields[0].bytes().all(|b| b == b'0'),
                });
            }
        }
    }
    Ok(lines)
}

fn decode_blame_path(value: &str) -> String {
    if !value.starts_with('"') || !value.ends_with('"') {
        return value.to_string();
    }
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 1;
    while i + 1 < bytes.len() {
        let byte = bytes[i];
        i += 1;
        if byte != b'\\' {
            out.push(byte);
            continue;
        }
        if i + 1 >= bytes.len() {
            break;
        }
        let escape = bytes[i];
        i += 1;
        let decoded = match escape {
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'b' => 8,
            b'f' => 12,
            b'v' => 11,
            b'a' => 7,
            b'0'..=b'7' => {
                let mut number = (escape - b'0') as u16;
                for _ in 0..2 {
                    if i + 1 < bytes.len() && (b'0'..=b'7').contains(&bytes[i]) {
                        number = number * 8 + (bytes[i] - b'0') as u16;
                        i += 1;
                    } else {
                        break;
                    }
                }
                number as u8
            }
            other => other,
        };
        out.push(decoded);
    }
    String::from_utf8_lossy(&out).into_owned()
}
