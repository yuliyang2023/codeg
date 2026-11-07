use super::git_history::resolve_commit;
use super::git_workflow::{
    checked, git, operation_result, output, require_idle, GitWorkflowResult, HISTORY_WRITE,
};
use crate::app_error::AppCommandError;
use serde::Serialize;

#[derive(Serialize)]
pub struct GitTag {
    pub name: String,
    pub object: String,
    pub commit: String,
    pub annotated: bool,
    pub message: String,
    pub date: String,
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_tags(path: String) -> Result<Vec<GitTag>, AppCommandError> {
    let raw=git(&path,&["for-each-ref","--sort=-creatordate","--format=%(refname:strip=2)%00%(objectname)%00%(*objectname)%00%(objecttype)%00%(contents:subject)%00%(creatordate:iso-strict)","refs/tags"]).await?;
    Ok(raw
        .lines()
        .filter_map(|line| {
            let f: Vec<_> = line.splitn(6, '\0').collect();
            (f.len() == 6).then(|| GitTag {
                name: f[0].into(),
                object: f[1].into(),
                commit: if f[2].is_empty() { f[1] } else { f[2] }.into(),
                annotated: f[3] == "tag",
                message: f[4].into(),
                date: f[5].into(),
            })
        })
        .collect())
}

async fn valid_tag(path: &str, name: &str) -> Result<(), AppCommandError> {
    if name.is_empty() || name.starts_with('-') || name.contains('\0') {
        return Err(AppCommandError::invalid_input("Enter a valid tag name"));
    }
    git(path, &["check-ref-format", &format!("refs/tags/{name}")]).await?;
    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_tag_create(
    path: String,
    name: String,
    revision: String,
    message: Option<String>,
) -> Result<String, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    valid_tag(&path, &name).await?;
    let commit = resolve_commit(&path, &revision).await?;
    let mut args = vec!["tag", "--no-sign"];
    let message = message.filter(|m| !m.trim().is_empty());
    if let Some(message) = &message {
        args.extend(["-a", "-m", message]);
    }
    args.extend(["--", &name, &commit]);
    git(&path, &args).await
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_tag_action(
    path: String,
    name: String,
    operation: String,
    expected_object: String,
    remote: Option<String>,
) -> Result<GitWorkflowResult, AppCommandError> {
    let _guard = HISTORY_WRITE.lock().await;
    valid_tag(&path, &name).await?;
    let tag_ref = format!("refs/tags/{name}");
    let current = git(
        &path,
        &["rev-parse", "--verify", "--end-of-options", &tag_ref],
    )
    .await?;
    if current != expected_object {
        return Err(AppCommandError::invalid_input(
            "Tag changed; refresh before continuing",
        ));
    }
    let result = match operation.as_str() {
        "delete" => {
            return git(&path, &["update-ref", "-d", &tag_ref, &expected_object])
                .await
                .map(|message| GitWorkflowResult {
                    message,
                    conflict: None,
                })
        }
        "checkout" | "merge" => {
            require_idle(&path).await?;
            if !git(&path, &["status", "--porcelain"]).await?.is_empty() {
                return Err(AppCommandError::invalid_input(
                    "Commit, shelve or stash changes first",
                ));
            }
            let commit = resolve_commit(&path, &tag_ref).await?;
            if operation == "checkout" {
                output(&path, &["checkout", "--detach", &commit]).await?
            } else {
                output(&path, &["merge", "--no-edit", &commit]).await?
            }
        }
        "push" | "delete-remote" => {
            let remote = remote
                .ok_or_else(|| AppCommandError::invalid_input("Select a configured remote"))?;
            let remotes = git(&path, &["remote"]).await?;
            if remote.starts_with('-') || !remotes.lines().any(|r| r == remote) {
                return Err(AppCommandError::invalid_input("Select a configured remote"));
            }
            let refspec = if operation == "push" {
                format!("{expected_object}:{tag_ref}")
            } else {
                format!(":{tag_ref}")
            };
            let mut args = vec!["push".to_string(), "--porcelain".to_string()];
            if operation == "delete-remote" {
                args.push(format!("--force-with-lease={tag_ref}:{expected_object}"));
            }
            args.extend(["--".to_string(), remote, refspec]);
            let args: Vec<_> = args.iter().map(String::as_str).collect();
            output(&path, &args).await?
        }
        _ => return Err(AppCommandError::invalid_input("Unsupported tag operation")),
    };
    if operation == "merge" {
        operation_result(&path, "merge", result).await
    } else {
        checked(result).map(|message| GitWorkflowResult {
            message,
            conflict: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn tag_lifecycle_and_remote_lease_protection() {
        let dir = tempfile::tempdir().unwrap();
        let remote = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let remote_path = remote.path().to_str().unwrap();
        git(remote_path, &["init", "--bare"]).await.unwrap();
        git(&path, &["init", "-b", "main"]).await.unwrap();
        git(&path, &["config", "user.name", "Tag Test"])
            .await
            .unwrap();
        git(&path, &["config", "user.email", "test@example.invalid"])
            .await
            .unwrap();
        std::fs::write(dir.path().join("a"), "base").unwrap();
        git(&path, &["add", "a"]).await.unwrap();
        git(&path, &["commit", "-m", "initial"]).await.unwrap();
        git(&path, &["remote", "add", "origin", remote_path])
            .await
            .unwrap();
        git_tag_create(
            path.clone(),
            "v1".into(),
            "HEAD".into(),
            Some("release".into()),
        )
        .await
        .unwrap();
        git_tag_create(path.clone(), "light".into(), "HEAD".into(), None)
            .await
            .unwrap();
        let tags = git_tags(path.clone()).await.unwrap();
        assert_eq!(tags.len(), 2);
        let tag = tags.iter().find(|t| t.name == "v1").unwrap();
        assert!(tag.annotated);
        assert_eq!(tag.message, "release");
        assert!(
            git_tag_create(path.clone(), "--all".into(), "HEAD".into(), None)
                .await
                .is_err()
        );
        assert!(git_tag_action(
            path.clone(),
            "v1".into(),
            "delete".into(),
            "stale".into(),
            None
        )
        .await
        .is_err());
        git_tag_action(
            path.clone(),
            "v1".into(),
            "push".into(),
            tag.object.clone(),
            Some("origin".into()),
        )
        .await
        .unwrap();
        assert_eq!(
            git(remote_path, &["rev-parse", "refs/tags/v1"])
                .await
                .unwrap(),
            tag.object
        );
        git(remote_path, &["update-ref", "refs/tags/v1", &tag.commit])
            .await
            .unwrap();
        assert!(git_tag_action(
            path.clone(),
            "v1".into(),
            "delete-remote".into(),
            tag.object.clone(),
            Some("origin".into())
        )
        .await
        .is_err());
        git(remote_path, &["update-ref", "refs/tags/v1", &tag.object])
            .await
            .unwrap();
        git_tag_action(
            path.clone(),
            "v1".into(),
            "delete-remote".into(),
            tag.object.clone(),
            Some("origin".into()),
        )
        .await
        .unwrap();
        assert!(git(remote_path, &["rev-parse", "--verify", "refs/tags/v1"])
            .await
            .is_err());
        git_tag_action(
            path.clone(),
            "v1".into(),
            "checkout".into(),
            tag.object.clone(),
            None,
        )
        .await
        .unwrap();
        assert!(git(&path, &["symbolic-ref", "--quiet", "HEAD"])
            .await
            .is_err());
        git_tag_action(
            path.clone(),
            "v1".into(),
            "delete".into(),
            tag.object.clone(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(git_tags(path).await.unwrap().len(), 1);
    }
}
