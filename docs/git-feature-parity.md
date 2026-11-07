# IdeaGitTool functional parity

Target: native Codeg workflows in desktop and server modes, using
[IdeaGitTool](https://github.com/PGshen/vs-idea-gittool) at `78719a8` (0.4.4)
as the reference. This is functional parity, not a replacement VS Code host
or a pixel-for-pixel IntelliJ skin. A menu item without a working operation,
failure handling and verification does not count as implemented.

`[x]` = present; `[ ]` = remaining. Existing Codeg capabilities are identified
explicitly; their presence alone does not imply all reference edge cases have
been verified. This checklist is the scope of the full integration, not a
claim that the integration is already complete.

## Commit and working copy

- [x] Commit / Shelf / Stash / Branches / History sidebar.
- [x] Existing selected-file commit window and commit-and-push.
- [x] Existing agent-generated commit messages with cancellation.
- [x] Tracked / untracked changes, file tree, refresh, rollback, add to VCS.
- [x] Commit exactly checked paths without including unrelated staged paths.
- [x] Amend HEAD with editable previous message and explicit confirmation.
- [x] Reword older commits through bounded linear history editing.
- [x] Successful commit message history (last 50 workspace messages).
- [x] Author override, sign-off, Git signing and per-commit hook options.
- [ ] Extra commit for file movements.
- [x] Changelists: create, rename, delete, default list, move files, drag/drop.
- [x] Tracked/untracked/conflict group filters and path speed search in Commit.
- [x] Add to .gitignore and remove from VCS in Commit file menus.
- [ ] Copy path/reference.

## Shelf, Stash and operation recovery

- [x] Repository-local durable Shelf independent of ordinary Stash.
- [x] Shelf save / patch preview / apply / confirmed delete; retain on failure.
- [x] Existing Stash list, file diffs, apply and delete.
- [x] Shelve only checked files, including untracked files and literal paths.
- [x] Shelf rename, group filtering and partial whole-file restore.
- [ ] Shelf drag/drop and partial hunk restoration.
- [x] Stash creation options including untracked files and keep-index.
- [x] Automatically open conflict resolution after Stash/Shelf restore.
- [x] Restored-change conflicts finish without committing unrelated work.
- [x] Resume interrupted operations after closing/reopening a view.
- [x] Smart tool: checkout / merge / rebase / update with durable Shelf recovery,
      index restoration, pending state and conflict continuation/abort.
- [ ] Offer smart preservation directly from ordinary branch/update actions.

## History and log

- [x] Existing paginated log, branch and author filters, changed files/details.
- [x] Existing new branch from commit, diff, reset and push entry points.
- [x] Commit graph with parent edges, refs, current branch and lazy loading.
- [x] Text/hash/regex/case, date and path filters; go to hash/ref.
- [x] Topological/date order, first-parent and no-merges options.
- [x] Single non-merge commit Cherry-pick and Revert, including continue/abort/skip on conflicts.
- [x] Merge-commit Cherry-pick/Revert with reviewed mainline selection.
- [x] Checkout revision, compare revisions and show repository at revision.
- [x] Undo latest commit and linear interactive plans: reword, fixup, squash,
      drop and reorder (up to 100 commits), with recovery refs and pending controls.
- [x] Merge-preserving message edits and whole-file commit splitting, with atomic ref updates and recovery refs.
- [ ] Merge-preserving reorder/drop/squash plans and line/hunk commit splitting.
- [x] Commit multi-selection, Shift visible-range selection and reviewed native Cherry-pick/Revert batches.
- [x] Parent/child commit navigation.
- [ ] Push through selected commit and tags.
- [ ] GPG column, collapse linear branches and cherry-pick-related filters.
- [ ] Multiple repositories, root colors, grouping and additional log tabs.
- [ ] Extract selected changes and add commits to remote branch.

## Diff, file history and annotations

- [x] Full-file split / Unified toggle with persistent synchronized choice.
- [x] Existing image comparison and lightweight patch previews.
- [x] Go to Source from workspace diff and sidebar Stash diff, with line mapping.
- [x] Go to Source from standalone commit/push/Stash windows; F4 shortcut.
- [x] Paginated file history with rename following and commit-diff navigation.
- [x] History of selected lines (committed ranges, with working-line mapping).
- [x] Virtualized per-line Git Blame view, local-change markers and navigation
      to the originating commit diff, including filenames before a rename.
- [x] Current-line inline editor blame annotations and commit navigation.
- [x] Compare file with revision, branch or tag.
- [x] Create/download selected whole-file working-tree patches (binary/untracked
      included); import/check/apply/reverse patches, optionally including the index.
- [ ] Selected-hunk/line patch export and revision-range patches.

## Branches, remotes and worktrees

- [x] Existing local/remote branch tree, checkout, create/delete, merge/rebase.
- [x] Existing fetch/pull/push, remotes management and worktree operations.
- [x] Tags: list, create, checkout, push, delete and merge.
- [ ] Rename, favorites, recent branches and incoming/outgoing counters.
- [ ] Checkout-and-rebase, remote tracking and branch comparison actions.
- [ ] Push dialog options: lease-protected force push, tags and hooks.
- [ ] Push rejected workflow and force-pushed branch recovery.
- [ ] Sync branch operations across multiple repositories.
- [ ] Complete ongoing-operation controls in branch and log views.

## Conflict editor

- [x] Existing three-pane editor, editable result, synchronized scrolling,
      hunk application and conflict resolution window.
- [ ] Reference-level automatic/simple conflict resolution and word matching.
- [ ] Iterative save/reopen, revert resolution, undo and per-change menus.
- [ ] Conflict connectors, collapse unchanged fragments and context controls.
- [ ] Binary and deleted/modified conflict choices.
- [ ] Consistent conflict entry points for every Git mutation.

## Local History, console and integration

- [x] Bounded automatic snapshots from workspace observation, editor open/save,
      rename/delete and observed external create/change/remove events.
- [ ] Directory rename lineage, file-mode restoration and configurable retention.
- [x] Recent per-file/project snapshot listing, manual labels, text comparison,
      guarded single-file restore (including binary content and deletion).
- [ ] Atomic project restore, selection history and Local History patch export.
- [x] Git Console capturing command, repository, output, duration and failures.
- [ ] Native action menu and configurable equivalent keyboard shortcuts.
- [x] Persisted per-repository history query settings.
- [ ] Remaining per-repository view settings and search history.
- [ ] Git capability detection and actionable version requirements.

## Verification rule

Mutating features need real temporary-repository tests covering selected vs.
unselected paths, index preservation, conflicts, continue/abort and refusal
without data loss. UI checks must exercise the actual operation rather than
only menu visibility. Deployment uses an isolated build on `sdp-u5`, with
backups and API smoke checks against disposable repositories before marking
a feature complete. New functionality is not exercised against user projects.

## Stage 1 verification (2026-11-07)

Deployed to `sdp-u5:3080` after an isolated remote build. Frontend production
build/type checking, targeted lint, 60 frontend tests, 15 backend tests and
server-mode Clippy passed. Disposable-repository API checks covered selective
commit (including subdirectories), selective Shelf, Amend, Cherry-pick/Revert,
continue/abort/skip and restoration conflicts without implicit commits. Browser
checks performed Amend and selective Shelf and verified the unchecked index
entries remained unchanged. Program, static files and SQLite were backed up
before the service restart. This verifies stage 1 only; unchecked items above
remain in scope.

## Stage 2 verification (2026-11-07)

Deployed to `sdp-u5:3080` after remote frontend/server release builds. Targeted
lint, TypeScript, 112 distinct frontend tests and 26 backend tests passed;
server-mode Clippy passed with warnings denied. Real-repository checks covered
merge parents, literal/regex/date/path/revision filtering, pagination, rename
following, working-tree Blame, tag creation/push/lease-protected deletion,
merge/detached checkout, root reword/squash/fixup, drop, soft undo, stale/dirty/
published-state refusal, recovery refs, and rebase conflict/hook recovery.

Isolated browser checks performed history search, rewording an older commit,
tag creation, automatic history refresh, rename-following file history, and
Blame navigation to the original filename's commit diff. Repository checks
confirmed the rewritten tree was unchanged and the old HEAD remained reachable.
Browser screenshot capture timed out, so screenshot-based visual review was
not completed. Program, static files and SQLite were backed up before restarting
the production service. Remaining unchecked items still define the full-parity
scope; this does not claim that every reference feature is complete.

## Stage 3 verification (2026-11-07)

Deployed to `sdp-u5:3080` after remote builds, targeted lint, 87 distinct frontend
checks/tests and 38 backend tests, with server-mode Clippy denying warnings.
Disposable-repository API checks exercised changelist revision guards, selected
binary/untracked patch round trips with an unchanged real index, file comparison,
smart checkout, preserved staged/unstaged work, durable Shelf copies, guarded
Local History restore, real external watcher capture, save/rename/delete
snapshots, and Console success/failure/clear. Unit tests also covered merge
continue/abort restoration and restoring the oldest snapshot at retention limits.

Browser checks created/moved/filtered a changelist, performed smart checkout,
created and reverse-applied a selected-file patch, restored Local History, used
the standalone commit window to commit only one changelist, compared a file
revision and checked the actual commit command in Console. Git inspection
confirmed that the unchecked staged file never entered the commit and retained
its staged content. Added tools use an accessible selector below the common tabs;
final layout was checked by screenshot. Program/static files/SQLite were backed
up before restarting the production service. The bounded scope and remaining
unchecked items above still apply.

## Commit / Shelf / Stash follow-up verification (2026-11-07)

Remote type checking, targeted lint, 13 frontend tests, four real-repository
backend tests, server Clippy and release builds passed. API checks verified a
real GPG-signed commit with a disposable key and `verify-commit`, author and
sign-off options, refused hooks/signers, selected-file commit isolation, bounded
message history, Shelf name/group metadata, selected binary/untracked restore,
index preservation, overlap refusal, literal ignore rules, non-destructive
untracking, and include-untracked/keep-index Stash options.

Browser checks performed a commit with a custom author and Sign-off, reused
message history, renamed/group-filtered a Shelf, restored one selected file,
created a Stash with untracked files while preserving its staged file, and used
path search/untracked filtering plus an escaped ignore rule. Git/filesystem
inspection confirmed their actual results. Screenshot capture timed out; layout
was checked through page structure and interaction rather than screenshot review.

## Advanced history verification (2026-11-07)

Remote type checking, targeted lint, 10 frontend tests, nine backend tests,
server-mode Clippy and production builds passed. Real temporary repositories
covered merge-parent selection, native multi-commit Cherry-pick/Revert, conflict
continue/abort, preserving exact trees and ordered merge parents during message
edits, splitting an older commit below a merge, recovery refs, other-branch
preservation, and dirty/stale/published-history refusal.

Browser checks used Shift range selection, rewrote two selected messages while
preserving a merge, split a two-file commit, and reverted a merge relative to
its second parent. Git inspection confirmed the resulting content, merge parent
count, clean worktree and recovery refs. Browser checks used the first validated
candidate; the final recovery-state refinements passed the backend suite and a
fresh API smoke run, with its binary hash checked before deployment. Screenshot
capture timed out. Merge-DAG reorder/drop/squash and line/hunk splitting remain
explicitly unchecked above.
