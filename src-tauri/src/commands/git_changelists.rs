use super::git_workbench_store::{directory, read, write};
use super::git_workflow::validate_files;
use crate::app_error::AppCommandError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;
static WRITE: Mutex<()> = Mutex::new(());

#[derive(Clone, Serialize, Deserialize)]
pub struct GitChangelist {
    pub id: String,
    pub name: String,
    pub files: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct GitChangelists {
    pub revision: u64,
    pub default_id: String,
    pub lists: Vec<GitChangelist>,
}
impl Default for GitChangelists {
    fn default() -> Self {
        Self {
            revision: 0,
            default_id: "default".into(),
            lists: vec![GitChangelist {
                id: "default".into(),
                name: "Default".into(),
                files: vec![],
            }],
        }
    }
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_changelists(path: String) -> Result<GitChangelists, AppCommandError> {
    let _guard = WRITE
        .lock()
        .map_err(|_| AppCommandError::invalid_input("Changelist storage is busy"))?;
    read(&directory(&path)?.join("changelists.json"))
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_changelist_update(
    path: String,
    expected_revision: u64,
    operation: String,
    id: Option<String>,
    name: Option<String>,
    files: Option<Vec<String>>,
) -> Result<GitChangelists, AppCommandError> {
    let _guard = WRITE
        .lock()
        .map_err(|_| AppCommandError::invalid_input("Changelist storage is busy"))?;
    let file = directory(&path)?.join("changelists.json");
    let mut state: GitChangelists = read(&file)?;
    if expected_revision != state.revision {
        return Err(AppCommandError::invalid_input(
            "Changelists changed; refresh before editing",
        ));
    }
    let id = id.unwrap_or_default();
    let valid_name = || -> Result<String, AppCommandError> {
        let name = name.as_deref().unwrap_or("").trim();
        if name.is_empty() || name.len() > 200 || name.contains(['\0', '\n', '\r']) {
            return Err(AppCommandError::invalid_input(
                "Enter a single-line changelist name",
            ));
        }
        if state.lists.iter().any(|l| l.id != id && l.name == name) {
            return Err(AppCommandError::invalid_input(
                "Changelist name already exists",
            ));
        }
        Ok(name.into())
    };
    match operation.as_str() {
        "create" => {
            let name = valid_name()?;
            state.lists.push(GitChangelist {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                files: vec![],
            });
        }
        "rename" => {
            let name = valid_name()?;
            let list = state
                .lists
                .iter_mut()
                .find(|l| l.id == id)
                .ok_or_else(|| AppCommandError::not_found("Changelist does not exist"))?;
            list.name = name;
        }
        "default" => {
            if !state.lists.iter().any(|l| l.id == id) {
                return Err(AppCommandError::not_found("Changelist does not exist"));
            }
            state.default_id = id;
        }
        "delete" => {
            if state.lists.len() == 1 {
                return Err(AppCommandError::invalid_input(
                    "Keep at least one changelist",
                ));
            }
            let index = state
                .lists
                .iter()
                .position(|l| l.id == id)
                .ok_or_else(|| AppCommandError::not_found("Changelist does not exist"))?;
            let removed = state.lists.remove(index);
            if state.default_id == id {
                state.default_id = state.lists[0].id.clone();
            }
            let default = state
                .lists
                .iter_mut()
                .find(|l| l.id == state.default_id)
                .unwrap();
            default.files.extend(removed.files);
        }
        "move" => {
            let files = files.unwrap_or_default();
            validate_files(&files)?;
            if !state.lists.iter().any(|l| l.id == id) {
                return Err(AppCommandError::not_found("Changelist does not exist"));
            }
            let selected: HashSet<_> = files.iter().collect();
            for list in &mut state.lists {
                list.files.retain(|f| !selected.contains(f));
            }
            state
                .lists
                .iter_mut()
                .find(|l| l.id == id)
                .unwrap()
                .files
                .extend(files);
        }
        _ => {
            return Err(AppCommandError::invalid_input(
                "Unsupported changelist operation",
            ))
        }
    }
    for list in &mut state.lists {
        list.files.sort();
        list.files.dedup();
    }
    state.revision += 1;
    write(&file, &state)?;
    Ok(state)
}

pub(crate) fn forget_paths(path: &str, files: &[String]) {
    let Ok(_guard) = WRITE.lock() else {
        return;
    };
    let Ok(directory) = directory(path) else {
        return;
    };
    let file = directory.join("changelists.json");
    let Ok(mut state) = read::<GitChangelists>(&file) else {
        return;
    };
    let mut changed = false;
    for list in &mut state.lists {
        let before = list.files.len();
        list.files.retain(|f| !files.contains(f));
        changed |= before != list.files.len();
    }
    if changed {
        state.revision += 1;
        let _ = write(&file, &state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn list_lifecycle_stale_revision_and_moving_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let initial = git_changelists(path.clone()).await.unwrap();
        assert_eq!(initial.lists.len(), 1);
        let created = git_changelist_update(
            path.clone(),
            0,
            "create".into(),
            None,
            Some("Feature".into()),
            None,
        )
        .await
        .unwrap();
        let id = created.lists[1].id.clone();
        assert!(git_changelist_update(
            path.clone(),
            0,
            "rename".into(),
            Some(id.clone()),
            Some("stale".into()),
            None
        )
        .await
        .is_err());
        let moved = git_changelist_update(
            path.clone(),
            created.revision,
            "move".into(),
            Some(id.clone()),
            None,
            Some(vec!["[literal].txt".into(), "old -> new".into()]),
        )
        .await
        .unwrap();
        assert_eq!(moved.lists[1].files.len(), 2);
        let renamed = git_changelist_update(
            path.clone(),
            moved.revision,
            "rename".into(),
            Some(id.clone()),
            Some("Renamed".into()),
            None,
        )
        .await
        .unwrap();
        let deleted = git_changelist_update(
            path.clone(),
            renamed.revision,
            "delete".into(),
            Some(id),
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(deleted.lists[0].files.len(), 2);
        forget_paths(&path, &["[literal].txt".into()]);
        assert_eq!(
            git_changelists(path.clone()).await.unwrap().lists[0].files,
            vec!["old -> new"]
        );
        assert!(git_changelist_update(
            path,
            deleted.revision + 1,
            "move".into(),
            Some("default".into()),
            None,
            Some(vec!["../outside".into()])
        )
        .await
        .is_err());
    }
}
