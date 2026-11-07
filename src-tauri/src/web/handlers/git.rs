use std::sync::Arc;

use axum::{extract::Extension, Json};
use serde::Deserialize;

use crate::app_error::AppCommandError;
use crate::app_state::AppState;
use crate::commands::folders as folder_commands;
use crate::models::GitCredentials;

use super::folders::PathParams;

// ---------------------------------------------------------------------------
// Shared param structs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathFileParams {
    pub path: String,
    pub file: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathBranchParams {
    pub path: String,
    pub branch_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathStashRefParams {
    pub path: String,
    pub stash_ref: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathNameParams {
    pub path: String,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathNameUrlParams {
    pub path: String,
    pub name: String,
    pub url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathOperationParams {
    pub path: String,
    pub operation: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathFileContentParams {
    pub path: String,
    pub file: String,
    pub content: String,
}

// ---------------------------------------------------------------------------
// Migrated from folders.rs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusParams {
    pub path: String,
    pub show_all_untracked: Option<bool>,
}

pub async fn git_status(
    Json(params): Json<GitStatusParams>,
) -> Result<Json<Vec<folder_commands::GitStatusEntry>>, AppCommandError> {
    let result = folder_commands::git_status(params.path, params.show_all_untracked).await?;
    Ok(Json(result))
}

pub async fn git_list_all_branches(
    Json(params): Json<PathParams>,
) -> Result<Json<folder_commands::GitBranchList>, AppCommandError> {
    let result = folder_commands::git_list_all_branches(params.path).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitBranchesParams {
    pub path: String,
    pub commit: String,
}

pub async fn git_commit_branches(
    Json(params): Json<GitCommitBranchesParams>,
) -> Result<Json<Vec<String>>, AppCommandError> {
    let result = folder_commands::git_commit_branches(params.path, params.commit).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitShowFileParams {
    pub path: String,
    pub file: String,
    pub ref_name: Option<String>,
}

pub async fn git_show_file(
    Json(params): Json<GitShowFileParams>,
) -> Result<Json<String>, AppCommandError> {
    let result = folder_commands::git_show_file(params.path, params.file, params.ref_name).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitShowFileBase64Params {
    pub path: String,
    pub file: String,
    pub ref_name: Option<String>,
    pub max_bytes: Option<usize>,
}

pub async fn git_show_file_base64(
    Json(params): Json<GitShowFileBase64Params>,
) -> Result<Json<folder_commands::GitBlobBase64>, AppCommandError> {
    let result = folder_commands::git_show_file_base64(
        params.path,
        params.file,
        params.ref_name,
        params.max_bytes,
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffParams {
    pub path: String,
    pub file: Option<String>,
}

pub async fn git_diff(Json(params): Json<GitDiffParams>) -> Result<Json<String>, AppCommandError> {
    let result = folder_commands::git_diff(params.path, params.file).await?;
    Ok(Json(result))
}

pub async fn git_list_remotes(
    Json(params): Json<PathParams>,
) -> Result<Json<Vec<folder_commands::GitRemote>>, AppCommandError> {
    let result = folder_commands::git_list_remotes(params.path).await?;
    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// Migrated from version_control.rs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLogParams {
    pub path: String,
    pub limit: Option<u32>,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub skip: Option<u32>,
    pub author: Option<String>,
    pub all_branches: Option<bool>,
    pub with_files: Option<bool>,
}

pub async fn git_log(
    Json(params): Json<GitLogParams>,
) -> Result<Json<folder_commands::GitLogResult>, AppCommandError> {
    let result = folder_commands::git_log(
        params.path,
        params.limit,
        params.branch,
        params.remote,
        params.skip,
        params.author,
        params.all_branches,
        params.with_files,
    )
    .await?;
    Ok(Json(result))
}

pub async fn git_current_user(
    Json(params): Json<PathParams>,
) -> Result<Json<Option<String>>, AppCommandError> {
    let result = folder_commands::git_current_user(params.path).await?;
    Ok(Json(result))
}

pub async fn git_commit_files(
    Json(params): Json<GitCommitBranchesParams>,
) -> Result<Json<Vec<folder_commands::GitLogFileChange>>, AppCommandError> {
    let result = folder_commands::git_commit_files(params.path, params.commit).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSearchAuthorsParams {
    pub path: String,
    pub query: String,
    pub limit: Option<u32>,
}

pub async fn git_search_authors(
    Json(params): Json<GitSearchAuthorsParams>,
) -> Result<Json<Vec<String>>, AppCommandError> {
    let result =
        folder_commands::git_search_authors(params.path, params.query, params.limit).await?;
    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// New pure git handlers (Pattern A – direct function calls)
// ---------------------------------------------------------------------------

pub async fn git_init(Json(params): Json<PathParams>) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_init(params.path).await?;
    Ok(Json(()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStartPullMergeParams {
    pub path: String,
    pub upstream_commit: Option<String>,
}

pub async fn git_start_pull_merge(
    Json(params): Json<GitStartPullMergeParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_start_pull_merge(params.path, params.upstream_commit).await?;
    Ok(Json(()))
}

pub async fn git_has_merge_head(
    Json(params): Json<PathParams>,
) -> Result<Json<bool>, AppCommandError> {
    let result = folder_commands::git_has_merge_head(params.path).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPushInfoParams {
    pub path: String,
    /// Target branch; omitted means the checked-out one.
    pub branch: Option<String>,
}

pub async fn git_push_info(
    Json(params): Json<GitPushInfoParams>,
) -> Result<Json<folder_commands::GitPushInfo>, AppCommandError> {
    let result = folder_commands::git_push_info(params.path, params.branch).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitNewBranchParams {
    pub path: String,
    pub branch_name: String,
    pub start_point: Option<String>,
}

pub async fn git_new_branch(
    Json(params): Json<GitNewBranchParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_new_branch(params.path, params.branch_name, params.start_point).await?;
    Ok(Json(()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeAddParams {
    pub path: String,
    pub branch_name: String,
    pub worktree_path: String,
    #[serde(default)]
    pub base: Option<String>,
}

pub async fn git_worktree_add(
    Json(params): Json<GitWorktreeAddParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_worktree_add(
        params.path,
        params.branch_name,
        params.worktree_path,
        params.base,
    )
    .await?;
    Ok(Json(()))
}

pub async fn git_checkout(
    Json(params): Json<PathBranchParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_checkout(params.path, params.branch_name).await?;
    Ok(Json(()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitResetParams {
    pub path: String,
    pub commit: String,
    pub mode: String,
}

pub async fn git_reset(Json(params): Json<GitResetParams>) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_reset(params.path, params.commit, params.mode).await?;
    Ok(Json(()))
}

pub async fn git_list_branches(
    Json(params): Json<PathParams>,
) -> Result<Json<Vec<String>>, AppCommandError> {
    let result = folder_commands::git_list_branches(params.path).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStashPushParams {
    pub path: String,
    pub message: Option<String>,
    pub keep_index: bool,
    pub include_untracked: Option<bool>,
}

pub async fn git_stash_push(
    Json(params): Json<GitStashPushParams>,
) -> Result<Json<String>, AppCommandError> {
    let result = folder_commands::git_stash_push(
        params.path,
        params.message,
        params.keep_index,
        params.include_untracked,
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStashPopParams {
    pub path: String,
    pub stash_ref: Option<String>,
}

pub async fn git_stash_pop(
    Json(params): Json<GitStashPopParams>,
) -> Result<Json<String>, AppCommandError> {
    let result = folder_commands::git_stash_pop(params.path, params.stash_ref).await?;
    Ok(Json(result))
}

pub async fn git_stash_list(
    Json(params): Json<PathParams>,
) -> Result<Json<Vec<folder_commands::GitStashEntry>>, AppCommandError> {
    let result = folder_commands::git_stash_list(params.path).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfRefParams {
    pub path: String,
    pub shelf_ref: String,
}

#[derive(Deserialize)]
pub struct ShelfSaveParams {
    pub path: String,
    pub message: String,
    pub files: Option<Vec<String>>,
}

pub async fn git_shelf_list(
    Json(params): Json<PathParams>,
) -> Result<Json<Vec<crate::commands::git_shelf::GitShelfEntry>>, AppCommandError> {
    Ok(Json(
        crate::commands::git_shelf::git_shelf_list(params.path).await?,
    ))
}

pub async fn git_shelf_save(
    Json(params): Json<ShelfSaveParams>,
) -> Result<Json<()>, AppCommandError> {
    crate::commands::git_shelf::git_shelf_save(params.path, params.message, params.files).await?;
    Ok(Json(()))
}

pub async fn git_shelf_apply(
    Json(params): Json<ShelfRefParams>,
) -> Result<Json<crate::commands::git_workflow::GitWorkflowResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_shelf::git_shelf_apply(params.path, params.shelf_ref).await?,
    ))
}

pub async fn git_shelf_drop(
    Json(params): Json<ShelfRefParams>,
) -> Result<Json<()>, AppCommandError> {
    crate::commands::git_shelf::git_shelf_drop(params.path, params.shelf_ref).await?;
    Ok(Json(()))
}

pub async fn git_shelf_diff(
    Json(params): Json<ShelfRefParams>,
) -> Result<Json<String>, AppCommandError> {
    Ok(Json(
        crate::commands::git_shelf::git_shelf_diff(params.path, params.shelf_ref).await?,
    ))
}

pub async fn git_stash_apply(
    Json(params): Json<PathStashRefParams>,
) -> Result<Json<crate::commands::git_workflow::GitWorkflowResult>, AppCommandError> {
    let result = folder_commands::git_stash_apply(params.path, params.stash_ref).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct CommitActionParams {
    pub path: String,
    pub commit: String,
    pub operation: String,
}

pub async fn git_commit_action(
    Json(params): Json<CommitActionParams>,
) -> Result<Json<crate::commands::git_workflow::GitWorkflowResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_workflow::git_commit_action(
            params.path,
            params.commit,
            params.operation,
        )
        .await?,
    ))
}

pub async fn git_amend_info(
    Json(params): Json<PathParams>,
) -> Result<Json<crate::commands::git_workflow::GitAmendInfo>, AppCommandError> {
    Ok(Json(
        crate::commands::git_workflow::git_amend_info(params.path).await?,
    ))
}

pub async fn git_operation_state(
    Json(params): Json<PathParams>,
) -> Result<Json<Option<folder_commands::GitConflictInfo>>, AppCommandError> {
    Ok(Json(
        crate::commands::git_workflow::git_operation_state(params.path).await?,
    ))
}

#[derive(Deserialize)]
pub struct SkipParams {
    pub path: String,
    pub operation: String,
}

pub async fn git_skip_operation(
    Json(params): Json<SkipParams>,
) -> Result<Json<crate::commands::git_workflow::GitWorkflowResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_workflow::git_skip_operation(params.path, params.operation).await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmendParams {
    pub path: String,
    pub message: String,
    pub files: Vec<String>,
    pub expected_head: String,
}

pub async fn git_amend(
    Json(params): Json<AmendParams>,
) -> Result<Json<crate::commands::git_workflow::GitAmendResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_workflow::git_amend(
            params.path,
            params.message,
            params.files,
            params.expected_head,
        )
        .await?,
    ))
}

pub async fn git_stash_drop(
    Json(params): Json<PathStashRefParams>,
) -> Result<Json<String>, AppCommandError> {
    let result = folder_commands::git_stash_drop(params.path, params.stash_ref).await?;
    Ok(Json(result))
}

pub async fn git_stash_clear(
    Json(params): Json<PathParams>,
) -> Result<Json<String>, AppCommandError> {
    let result = folder_commands::git_stash_clear(params.path).await?;
    Ok(Json(result))
}

pub async fn git_stash_show(
    Json(params): Json<PathStashRefParams>,
) -> Result<Json<Vec<folder_commands::GitStatusEntry>>, AppCommandError> {
    let result = folder_commands::git_stash_show(params.path, params.stash_ref).await?;
    Ok(Json(result))
}

pub async fn git_is_tracked(
    Json(params): Json<PathFileParams>,
) -> Result<Json<bool>, AppCommandError> {
    let result = folder_commands::git_is_tracked(params.path, params.file).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffWithBranchParams {
    pub path: String,
    pub branch: String,
    pub file: Option<String>,
}

pub async fn git_diff_with_branch(
    Json(params): Json<GitDiffWithBranchParams>,
) -> Result<Json<String>, AppCommandError> {
    let result =
        folder_commands::git_diff_with_branch(params.path, params.branch, params.file).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitShowDiffParams {
    pub path: String,
    pub commit: String,
    pub file: Option<String>,
}

pub async fn git_show_diff(
    Json(params): Json<GitShowDiffParams>,
) -> Result<Json<String>, AppCommandError> {
    let result = folder_commands::git_show_diff(params.path, params.commit, params.file).await?;
    Ok(Json(result))
}

pub async fn git_rollback_file(
    Json(params): Json<PathFileParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_rollback_file(params.path, params.file).await?;
    Ok(Json(()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitAddFilesParams {
    pub path: String,
    pub files: Vec<String>,
}

pub async fn git_add_files(
    Json(params): Json<GitAddFilesParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_add_files(params.path, params.files).await?;
    Ok(Json(()))
}

pub async fn git_add_remote(
    Json(params): Json<PathNameUrlParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_add_remote(params.path, params.name, params.url).await?;
    Ok(Json(()))
}

pub async fn git_remove_remote(
    Json(params): Json<PathNameParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_remove_remote(params.path, params.name).await?;
    Ok(Json(()))
}

pub async fn git_set_remote_url(
    Json(params): Json<PathNameUrlParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_set_remote_url(params.path, params.name, params.url).await?;
    Ok(Json(()))
}

pub async fn git_merge(
    Json(params): Json<PathBranchParams>,
) -> Result<Json<folder_commands::GitMergeResult>, AppCommandError> {
    let result = folder_commands::git_merge(params.path, params.branch_name).await?;
    Ok(Json(result))
}

pub async fn git_rebase(
    Json(params): Json<PathBranchParams>,
) -> Result<Json<folder_commands::GitRebaseResult>, AppCommandError> {
    let result = folder_commands::git_rebase(params.path, params.branch_name).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDeleteBranchParams {
    pub path: String,
    pub branch_name: String,
    pub force: bool,
}

pub async fn git_delete_branch(
    Json(params): Json<GitDeleteBranchParams>,
) -> Result<Json<String>, AppCommandError> {
    let result =
        folder_commands::git_delete_branch(params.path, params.branch_name, params.force).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemoveWorktreeParams {
    pub path: String,
    pub branch_name: String,
    pub source_folder_id: i32,
    pub delete_branch: bool,
    pub force: bool,
}

pub async fn git_remove_worktree(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<GitRemoveWorktreeParams>,
) -> Result<Json<folder_commands::GitWorktreeRemoval>, AppCommandError> {
    let result = folder_commands::git_remove_worktree_core(
        &state.emitter,
        &state.db,
        params.path,
        params.branch_name,
        params.source_folder_id,
        params.delete_branch,
        params.force,
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDeleteRemoteBranchParams {
    pub path: String,
    pub remote: String,
    pub branch: String,
    pub credentials: Option<GitCredentials>,
}

pub async fn git_delete_remote_branch(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<GitDeleteRemoteBranchParams>,
) -> Result<Json<()>, AppCommandError> {
    let db = &state.db;
    folder_commands::git_delete_remote_branch_core(
        &params.path,
        &params.remote,
        &params.branch,
        params.credentials.as_ref(),
        db,
        &state.data_dir,
    )
    .await?;
    Ok(Json(()))
}

pub async fn git_list_conflicts(
    Json(params): Json<PathParams>,
) -> Result<Json<Vec<String>>, AppCommandError> {
    let result = folder_commands::git_list_conflicts(params.path).await?;
    Ok(Json(result))
}

pub async fn git_conflict_file_versions(
    Json(params): Json<PathFileParams>,
) -> Result<Json<folder_commands::GitConflictFileVersions>, AppCommandError> {
    let result = folder_commands::git_conflict_file_versions(params.path, params.file).await?;
    Ok(Json(result))
}

pub async fn git_resolve_conflict(
    Json(params): Json<PathFileContentParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_resolve_conflict(params.path, params.file, params.content).await?;
    Ok(Json(()))
}

pub async fn git_abort_operation(
    Json(params): Json<PathOperationParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_abort_operation(params.path, params.operation).await?;
    Ok(Json(()))
}

pub async fn git_continue_operation(
    Json(params): Json<PathOperationParams>,
) -> Result<Json<()>, AppCommandError> {
    folder_commands::git_continue_operation(params.path, params.operation).await?;
    Ok(Json(()))
}

// ---------------------------------------------------------------------------
// Remote git handlers (Pattern B – need AppHandle for DB access)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPullParams {
    pub path: String,
    pub credentials: Option<GitCredentials>,
}

pub async fn git_pull(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<GitPullParams>,
) -> Result<Json<folder_commands::GitPullResult>, AppCommandError> {
    let db = &state.db;
    let result = folder_commands::git_pull_core(
        &params.path,
        params.credentials.as_ref(),
        db,
        &state.data_dir,
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFetchParams {
    pub path: String,
    pub credentials: Option<GitCredentials>,
}

pub async fn git_fetch(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<GitFetchParams>,
) -> Result<Json<String>, AppCommandError> {
    let db = &state.db;
    let result = folder_commands::git_fetch_core(
        &params.path,
        params.credentials.as_ref(),
        db,
        &state.data_dir,
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitUpdateBranchParams {
    pub path: String,
    pub branch: String,
    pub is_remote: bool,
    pub credentials: Option<GitCredentials>,
}

pub async fn git_update_branch(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<GitUpdateBranchParams>,
) -> Result<Json<folder_commands::GitPullResult>, AppCommandError> {
    let db = &state.db;
    let result = folder_commands::git_update_branch_core(
        &params.path,
        &params.branch,
        params.is_remote,
        params.credentials.as_ref(),
        db,
        &state.data_dir,
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPushParams {
    pub folder_id: Option<i32>,
    pub path: String,
    pub remote: Option<String>,
    /// Target branch; omitted means the checked-out one.
    pub branch: Option<String>,
    pub credentials: Option<GitCredentials>,
}

pub async fn git_push(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<GitPushParams>,
) -> Result<Json<folder_commands::GitPushResult>, AppCommandError> {
    let db = &state.db;
    let emitter = state.emitter.clone();
    let result = folder_commands::git_push_core(
        &state.data_dir,
        &emitter,
        params.folder_id,
        &params.path,
        params.remote.as_deref(),
        params.branch.as_deref(),
        params.credentials.as_ref(),
        db,
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitParams {
    pub options: Option<folder_commands::GitCommitOptions>,
    pub folder_id: Option<i32>,
    pub path: String,
    pub message: String,
    pub files: Vec<String>,
}

pub async fn git_commit(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<GitCommitParams>,
) -> Result<Json<folder_commands::GitCommitResult>, AppCommandError> {
    let db = &state.db;
    let emitter = state.emitter.clone();
    let result = folder_commands::git_commit_options_core(
        &emitter,
        params.folder_id,
        &db.conn,
        &params.path,
        &params.message,
        &params.files,
        params.options.unwrap_or_default(),
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFetchRemoteParams {
    pub path: String,
    pub name: String,
    pub credentials: Option<GitCredentials>,
}

pub async fn git_fetch_remote(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<GitFetchRemoteParams>,
) -> Result<Json<String>, AppCommandError> {
    let db = &state.db;
    let result = folder_commands::git_fetch_remote_core(
        &params.path,
        &params.name,
        params.credentials.as_ref(),
        db,
        &state.data_dir,
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRepositoryParams {
    pub url: String,
    pub target_dir: String,
    pub credentials: Option<GitCredentials>,
}

pub async fn clone_repository(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<CloneRepositoryParams>,
) -> Result<Json<()>, AppCommandError> {
    let db = &state.db;
    folder_commands::clone_repository_core(
        &params.url,
        &params.target_dir,
        params.credentials.as_ref(),
        db,
        &state.data_dir,
    )
    .await?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct GitHistoryParams {
    pub path: String,
    pub query: crate::commands::git_history::GitHistoryQuery,
}

pub async fn git_history(
    Json(params): Json<GitHistoryParams>,
) -> Result<Json<crate::commands::git_history::GitHistoryResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_history::git_history(params.path, params.query).await?,
    ))
}

#[derive(Deserialize)]
pub struct GitBlameParams {
    pub path: String,
    pub file: String,
    pub revision: Option<String>,
}
pub async fn git_blame(
    Json(params): Json<GitBlameParams>,
) -> Result<Json<Vec<crate::commands::git_history::GitBlameLine>>, AppCommandError> {
    Ok(Json(
        crate::commands::git_history::git_blame(params.path, params.file, params.revision).await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTagCreateParams {
    pub path: String,
    pub name: String,
    pub revision: String,
    pub message: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTagActionParams {
    pub path: String,
    pub name: String,
    pub operation: String,
    pub expected_object: String,
    pub remote: Option<String>,
}
pub async fn git_tags(
    Json(params): Json<PathParams>,
) -> Result<Json<Vec<crate::commands::git_tags::GitTag>>, AppCommandError> {
    Ok(Json(
        crate::commands::git_tags::git_tags(params.path).await?,
    ))
}
pub async fn git_tag_create(
    Json(params): Json<GitTagCreateParams>,
) -> Result<Json<String>, AppCommandError> {
    Ok(Json(
        crate::commands::git_tags::git_tag_create(
            params.path,
            params.name,
            params.revision,
            params.message,
        )
        .await?,
    ))
}
pub async fn git_tag_action(
    Json(params): Json<GitTagActionParams>,
) -> Result<Json<crate::commands::git_workflow::GitWorkflowResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_tags::git_tag_action(
            params.path,
            params.name,
            params.operation,
            params.expected_object,
            params.remote,
        )
        .await?,
    ))
}

#[derive(Deserialize)]
pub struct GitHistoryPlanParams {
    pub path: String,
    pub base: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHistoryEditParams {
    pub path: String,
    pub base: String,
    pub expected_head: String,
    pub steps: Vec<crate::commands::git_history_edit::GitHistoryStep>,
    pub allow_published: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitUndoParams {
    pub path: String,
    pub expected_head: String,
    pub allow_published: bool,
}
pub async fn git_history_plan(
    Json(params): Json<GitHistoryPlanParams>,
) -> Result<Json<crate::commands::git_history_edit::GitHistoryPlan>, AppCommandError> {
    Ok(Json(
        crate::commands::git_history_edit::git_history_plan(params.path, params.base).await?,
    ))
}
pub async fn git_history_edit(
    Json(params): Json<GitHistoryEditParams>,
) -> Result<Json<crate::commands::git_history_edit::GitHistoryEditResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_history_edit::git_history_edit(
            params.path,
            params.base,
            params.expected_head,
            params.steps,
            params.allow_published,
        )
        .await?,
    ))
}
pub async fn git_undo_commit(
    Json(params): Json<GitUndoParams>,
) -> Result<Json<crate::commands::git_workflow::GitWorkflowResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_history_edit::git_undo_commit(
            params.path,
            params.expected_head,
            params.allow_published,
        )
        .await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitChangelistsParams {
    pub path: String,
}
pub async fn git_changelists(
    Json(params): Json<GitChangelistsParams>,
) -> Result<Json<crate::commands::git_changelists::GitChangelists>, AppCommandError> {
    Ok(Json(
        crate::commands::git_changelists::git_changelists(params.path).await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitChangelistUpdateParams {
    pub path: String,
    pub expected_revision: u64,
    pub operation: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub files: Option<Vec<String>>,
}
pub async fn git_changelist_update(
    Json(params): Json<GitChangelistUpdateParams>,
) -> Result<Json<crate::commands::git_changelists::GitChangelists>, AppCommandError> {
    Ok(Json(
        crate::commands::git_changelists::git_changelist_update(
            params.path,
            params.expected_revision,
            params.operation,
            params.id,
            params.name,
            params.files,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSmartStateParams {
    pub path: String,
}
pub async fn git_smart_state(
    Json(params): Json<GitSmartStateParams>,
) -> Result<Json<Option<crate::commands::git_smart::GitSmartState>>, AppCommandError> {
    Ok(Json(
        crate::commands::git_smart::git_smart_state(params.path).await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSmartRunParams {
    pub path: String,
    pub operation: String,
    pub target: String,
}
pub async fn git_smart_run(
    Json(params): Json<GitSmartRunParams>,
) -> Result<Json<crate::commands::git_smart::GitSmartResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_smart::git_smart_run(params.path, params.operation, params.target)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSmartResumeParams {
    pub path: String,
    pub finish_manual: bool,
}
pub async fn git_smart_resume(
    Json(params): Json<GitSmartResumeParams>,
) -> Result<Json<crate::commands::git_smart::GitSmartResult>, AppCommandError> {
    Ok(Json(
        crate::commands::git_smart::git_smart_resume(params.path, params.finish_manual).await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCompareFileParams {
    pub path: String,
    pub file: String,
    pub base: String,
    pub target: String,
}
pub async fn git_compare_file(
    Json(params): Json<GitCompareFileParams>,
) -> Result<Json<crate::commands::git_patch::GitFileComparison>, AppCommandError> {
    Ok(Json(
        crate::commands::git_patch::git_compare_file(
            params.path,
            params.file,
            params.base,
            params.target,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPatchCreateParams {
    pub path: String,
    pub files: Vec<String>,
}
pub async fn git_patch_create(
    Json(params): Json<GitPatchCreateParams>,
) -> Result<Json<String>, AppCommandError> {
    Ok(Json(
        crate::commands::git_patch::git_patch_create(params.path, params.files).await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPatchPreviewParams {
    pub path: String,
    pub patch: String,
    pub reverse: bool,
    pub stage: bool,
}
pub async fn git_patch_preview(
    Json(params): Json<GitPatchPreviewParams>,
) -> Result<Json<crate::commands::git_patch::GitPatchPreview>, AppCommandError> {
    Ok(Json(
        crate::commands::git_patch::git_patch_preview(
            params.path,
            params.patch,
            params.reverse,
            params.stage,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPatchApplyParams {
    pub path: String,
    pub patch: String,
    pub reverse: bool,
    pub stage: bool,
}
pub async fn git_patch_apply(
    Json(params): Json<GitPatchApplyParams>,
) -> Result<Json<String>, AppCommandError> {
    Ok(Json(
        crate::commands::git_patch::git_patch_apply(
            params.path,
            params.patch,
            params.reverse,
            params.stage,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalHistoryCaptureParams {
    pub path: String,
    pub file: Option<String>,
    pub label: Option<String>,
}
pub async fn local_history_capture(
    Json(params): Json<LocalHistoryCaptureParams>,
) -> Result<Json<usize>, AppCommandError> {
    Ok(Json(
        crate::commands::local_history::local_history_capture(
            params.path,
            params.file,
            params.label,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalHistoryListParams {
    pub path: String,
    pub file: Option<String>,
}
pub async fn local_history_list(
    Json(params): Json<LocalHistoryListParams>,
) -> Result<Json<Vec<crate::commands::local_history::LocalHistoryEntry>>, AppCommandError> {
    Ok(Json(
        crate::commands::local_history::local_history_list(params.path, params.file).await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalHistoryContentParams {
    pub path: String,
    pub id: String,
}
pub async fn local_history_content(
    Json(params): Json<LocalHistoryContentParams>,
) -> Result<Json<crate::commands::local_history::LocalHistoryContent>, AppCommandError> {
    Ok(Json(
        crate::commands::local_history::local_history_content(params.path, params.id).await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalHistoryRestoreParams {
    pub path: String,
    pub id: String,
    pub expected_hash: Option<String>,
}
pub async fn local_history_restore(
    Json(params): Json<LocalHistoryRestoreParams>,
) -> Result<Json<()>, AppCommandError> {
    Ok(Json(
        crate::commands::local_history::local_history_restore(
            params.path,
            params.id,
            params.expected_hash,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitConsoleListParams {
    pub path: String,
}
pub async fn git_console_list(
    Json(params): Json<GitConsoleListParams>,
) -> Result<Json<Vec<crate::commands::git_console::GitConsoleEntry>>, AppCommandError> {
    Ok(Json(
        crate::commands::git_console::git_console_list(params.path).await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitConsoleClearParams {
    pub path: String,
}
pub async fn git_console_clear(
    Json(params): Json<GitConsoleClearParams>,
) -> Result<Json<()>, AppCommandError> {
    Ok(Json(
        crate::commands::git_console::git_console_clear(params.path).await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRevisionInfoParams {
    pub path: String,
    pub revision: String,
}
pub async fn git_revision_info(
    Json(params): Json<GitRevisionInfoParams>,
) -> Result<Json<crate::commands::git_revision::GitRevisionInfo>, AppCommandError> {
    Ok(Json(
        crate::commands::git_revision::git_revision_info(params.path, params.revision).await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRevisionTreeParams {
    pub path: String,
    pub revision: String,
    pub directory: String,
}
pub async fn git_revision_tree(
    Json(params): Json<GitRevisionTreeParams>,
) -> Result<Json<Vec<crate::commands::git_revision::GitRevisionTreeEntry>>, AppCommandError> {
    Ok(Json(
        crate::commands::git_revision::git_revision_tree(
            params.path,
            params.revision,
            params.directory,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRevisionFileParams {
    pub path: String,
    pub revision: String,
    pub file: String,
}
pub async fn git_revision_file(
    Json(params): Json<GitRevisionFileParams>,
) -> Result<Json<crate::commands::git_revision::GitRevisionFile>, AppCommandError> {
    Ok(Json(
        crate::commands::git_revision::git_revision_file(params.path, params.revision, params.file)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRevisionCompareParams {
    pub path: String,
    pub base: String,
    pub target: String,
}
pub async fn git_revision_compare(
    Json(params): Json<GitRevisionCompareParams>,
) -> Result<Json<crate::commands::git_revision::GitRevisionComparison>, AppCommandError> {
    Ok(Json(
        crate::commands::git_revision::git_revision_compare(
            params.path,
            params.base,
            params.target,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRevisionCheckoutParams {
    pub path: String,
    pub revision: String,
    pub expected_head: String,
}
pub async fn git_revision_checkout(
    Json(params): Json<GitRevisionCheckoutParams>,
) -> Result<Json<String>, AppCommandError> {
    Ok(Json(
        crate::commands::git_revision::git_revision_checkout(
            params.path,
            params.revision,
            params.expected_head,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSourceLineParams {
    pub path: String,
    pub file: String,
    pub revision: String,
    pub line: u32,
}
pub async fn git_source_line(
    Json(params): Json<GitSourceLineParams>,
) -> Result<Json<u32>, AppCommandError> {
    Ok(Json(
        crate::commands::git_revision::git_source_line(
            params.path,
            params.file,
            params.revision,
            params.line,
        )
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLineHistoryParams {
    pub path: String,
    pub file: String,
    pub start: u32,
    pub end: u32,
    pub revision: Option<String>,
    pub skip: Option<u32>,
}
pub async fn git_line_history(
    Json(params): Json<GitLineHistoryParams>,
) -> Result<Json<Vec<crate::commands::git_revision::GitLineHistoryEntry>>, AppCommandError> {
    Ok(Json(
        crate::commands::git_revision::git_line_history(
            params.path,
            params.file,
            params.start,
            params.end,
            params.revision,
            params.skip,
        )
        .await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitToolsParams {
    pub path: String,
    pub operation: String,
    pub files: Option<Vec<String>>,
    pub shelf_ref: Option<String>,
    pub name: Option<String>,
    pub group: Option<String>,
}
pub async fn git_commit_tools(
    Json(p): Json<GitCommitToolsParams>,
) -> Result<Json<serde_json::Value>, AppCommandError> {
    Ok(Json(
        crate::commands::git_commit_tools::git_commit_tools(
            p.path,
            p.operation,
            p.files,
            p.shelf_ref,
            p.name,
            p.group,
        )
        .await?,
    ))
}

#[derive(Deserialize)]
pub struct GitHistoryAdvancedParams {
    pub path: String,
    pub request: crate::commands::git_history_advanced::HistoryRequest,
}
pub async fn git_history_advanced(
    Json(p): Json<GitHistoryAdvancedParams>,
) -> Result<Json<serde_json::Value>, AppCommandError> {
    Ok(Json(
        crate::commands::git_history_advanced::git_history_advanced(p.path, p.request).await?,
    ))
}
