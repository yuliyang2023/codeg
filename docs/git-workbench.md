# Git workbench

The **Commits** sidebar includes Commit, Shelf, Stash, Branches and History.
History remains the initial view. Commit reuses Codeg's existing changes and
AI commit-message controls. Branches shares the searchable branch selector,
credential prompts and operation dialogs used elsewhere in Codeg.

Shelf saves the checked tracked changes and untracked files from the current repository.
Ignored files are excluded. A preview shows the saved patch, including binary
file summaries. Restore applies the changes and keeps the saved copy; Delete
requires confirmation. If Git reports conflicts, the copy is retained.
Shelf records are local to the repository and are not pushed automatically.

The changes toolbar and standalone commit window include **Amend latest
commit**. The dialog loads the previous message, warns if a remote-tracking
branch contains the commit, and lets the user choose additional files. No files
are selected initially, so a message-only amend leaves staged files out.
The server rejects the operation if HEAD changed after the dialog was loaded.
Amend uses a separate index; an unchecked staged file cannot enter the commit.
Regular selected-file commits also exclude unrelated staged paths.

The history context menu includes **Cherry-pick** and **Revert** for individual
non-merge commits. These operations require a clean tracked working tree/index.
They do not push automatically. On conflicts, the existing three-pane conflict
editor is available, with continue, abort and skip controls in the workbench.
Merge commits require mainline selection and are currently refused.

Shelf/Stash restoration opens conflict resolution when Git leaves unmerged
paths. The saved copy stays intact. Finishing restoration only verifies that
conflicts are resolved; it does not create a commit. A restore marker in the
worktree's Git directory makes the unfinished operation discoverable after a
page reload. "Resolve later" keeps the conflicted working tree as it is.

The shelf backend stores stash-compatible objects under
`refs/codeg/shelves/<uuid>`, independently of `refs/stash`. Saving first creates
a uniquely named recovery stash, persists the shelf reference, then removes that
stash entry. If persistence or cleanup fails, the changes remain in the stash or
the shelf. Shelf operations do not use `git reset --hard` or `git clean`.
Linked worktrees share shelves, as they share the repository's refs.

All operations use Codeg's Transport layer; the five `git_shelf_*` commands are
registered in both the Tauri command list and Axum API router.

The workflow is inspired by
[IdeaGitTool](https://github.com/PGshen/vs-idea-gittool). This is a native Codeg
implementation: it does not load the VS Code extension or copy its source or
IntelliJ icon assets. The remaining full-parity work, including interactive
history editing and Local History, is tracked in [git-feature-parity.md](git-feature-parity.md).

## History exploration and editing

History now includes a paginated commit graph with real parent edges, HEAD and
branch/tag decorations. Filters cover commit messages (literal or regular
expression, optional case matching), date ranges, repository-relative literal
paths, first-parent traversal, merge exclusion and topological/date order.
A hash, branch or tag can be used as the starting revision, overriding the
branch scope while retaining the other filters. Filters are saved
per repository. Filtered-out commits are not represented as direct edges.

File context menus in the file tree, changes view and expanded commit details
open **File history** or **Git Blame**. File history follows renames and loads
older commits on demand. Opening a historical entry shows its commit diff,
including the original filenames. Blame shows each line's author and commit,
marks local changes, and opens the originating commit diff. Source-line opening
is available for current working-tree annotations; historical line numbers are
not treated as current source positions. Inline editor annotations and selected
line history remain separate pending work.

The **Tags** page lists lightweight/annotated tags and supports creation, commit
inspection, detached checkout, merge, push and confirmed local/remote deletion.
Checkout/merge require a clean working tree. Tag actions verify the local tag
object has not changed since it was loaded. Remote deletion additionally uses a
lease, so a remotely changed tag is refused instead of deleted. A push never
forces replacement of a remote tag.

The history context menu opens a reviewable **Edit history from here** plan,
ordered oldest first. It supports Pick, Reword, Squash, Fixup, Drop and reordering.
Plans cover at most 100 linear commits; histories containing merge commits are
refused. Reword messages are stored as data, never interpolated into commands.
All working changes must first be committed, shelved or stashed. The server
checks the expected HEAD and requires explicit confirmation for commits already
contained in remote-tracking branches. No history operation pushes automatically.
**Undo latest commit** moves HEAD to its first parent with a soft reset, keeping
working files and the index; it cannot undo an initial or merge commit here.

Before a rewrite/undo, the old HEAD is saved under a unique
`refs/codeg/history-backups/<uuid>` reference. These refs are retained and are
excluded from the normal all-branches graph. During an interactive rebase,
message files stay in the worktree's Git directory so conflicts can be resolved
and the operation continued or aborted after a page reload. A refused commit
hook can also leave a pending rebase with no conflicted files; the operation
banner still exposes Continue/Abort/Skip. Successful completion or abort removes
temporary editor files, while keeping the recovery ref.

The history editor uses [Git interactive rebase](https://git-scm.com/docs/git-rebase)
underneath the native review dialog. Merge-preserving interactive plans, arbitrary
exec commands, splitting commits and editing the todo list of an already paused
rebase are not part of this bounded editor.

## Changelists, smart operations, patches and local recovery

**Changelists** store per-workspace file assignments outside the repository.
Create, rename, delete, choose a default list, move checked files or drag file
rows onto list buttons. New/unassigned files belong to the current default list;
deleting a list moves its assignments to the default. The commit sidebar and
standalone commit window can filter by list. Only visible checked paths enter
a standalone commit; quick commit covers tracked paths in the displayed list.
Successful normal commits remove those paths' explicit assignments. Metadata
updates use revision guards and refresh across views; organizing lists does
not stage or alter files. Changelists operate at file granularity.

**Smart** runs checkout of a local branch, merge, rebase or pull-merge after
saving tracked, staged and untracked changes. It persists a stash-compatible
Shelf ref before executing the operation. On completion it attempts restoration
with the original index. Merge/rebase conflicts use the existing operation
controls; continuing or aborting then restores saved work. Restore conflicts
are a separate pending operation and never commit saved work. If index restore
fails without changing the tree, Git retries the content merge without index
restoration; conflicting files require manual resolution/staging. Filename
collisions and other partial failures keep the Shelf copy and a pending marker
for manual review. An interrupted/restarted service can rediscover the marker.
All recovery copies remain in Shelf until explicitly deleted. Remote operations
use Git's configured credentials/helpers; credential failures preserve the copy.
These actions are available in Smart; existing ordinary branch actions keep
their existing behavior.

**Compare file versions** is available in file context menus alongside File
history/Blame. Enter a commit, branch, tag or `WORKTREE` on either side. Text
comparisons require UTF-8, at most 5 MiB; a missing file on a valid revision is
shown as empty. Invalid revisions are errors. The existing split/Unified viewer
is reused. This comparison uses the specified pathname on both sides rather
than automatically following a rename.

**Patches** exports checked working-tree paths using a temporary index, including
binary and untracked files. The real index remains unchanged. Download the
result or import/paste a patch (up to 10 MiB), check applicability and confirm
application. Reverse and index application are explicit choices. Any patch or
option edit invalidates the preview; Git checks again when applying. Git's
normal path/symlink safety remains enabled. Export is for whole selected files,
not selected hunks or lines.

**Local History** stores snapshots separately from Git under Codeg's workbench
data directory. Workspace watching seeds a bounded project snapshot and records
observed external changes. Editor open/save and file rename/delete also capture
before-images. History starts when Codeg observes a workspace; it cannot recover
content changed before observation. Only the currently observed paths can be
captured from external events, and changes faster than watcher debounce may be
coalesced. Files under `.git`, `node_modules`, `target`, `.next`, Codeg metadata
and symbolic links are excluded; the initial scan also honors ignore files.
Explicit manual/editor captures can include other ignored files.

Retention is bounded to 30 days, 2,000 entries and 256 MiB of referenced blobs
per workspace; files are limited to 2 MiB and project scans to 64 MiB/2,000
files per batch. Labels can be attached to manual file/project captures. Browse
and filter recent snapshots, compare text against the current file, and confirm
restoration of an individual file (including binary bytes or deletion). The
comparison's content hash guards restoration and a before-restore snapshot
retains displaced content. Binary preview, project-wide atomic restoration,
selection history/patch export and permission restoration are not included.

**Console** records Codeg's Git commands in the folder backend and native
workbench services, including repository, time, duration, exit status and bounded
stdout/stderr. It retains 200 records per workspace, with each output stream
limited to 16,384 characters. Recognized HTTP credentials and token/password
fields are redacted. Filter, expand or clear records from the Console page.
Commands launched independently by terminals/agents or inside hooks are outside
this capture. The Console is a bounded operation log, not a command shell.

## Revision browsing and editor tracing

Use **Tools → Browse revisions**, or right-click a history commit and choose
**Browse revisions**. Enter a commit, tag or branch; parent/child buttons navigate
the graph, and directories load the immutable file tree at that commit. Text
blobs up to 5 MiB can be previewed read-only. Binary/non-UTF-8/large blobs and
submodule contents are not rendered. Enter a second revision in the comparison
field to compare the two resolved commits, then select a changed file. Both
Split and Unified use the shared Diff viewer preference.

**Check out commit** explicitly confirms detached HEAD and requires a clean
working tree (including untracked files), an idle Git operation and the same
HEAD that was reviewed. Branches are left intact; switch back through Branches
or create a branch before continuing development.

The standalone Commit, Push and Stash Diff editors offer **Go to Source** in
the context menu and on **F4**. Historical lines are mapped into the current
working file; deleted/changed lines use the nearest surviving position. The
source dialog supports editing and etag-guarded save, and confirms discarding
unsaved changes. Missing current files produce an error.

In source editors, **Selected line history** traces the selected whole lines
with Git's line history and supports paging and opening the originating commit.
Unsaved edits must be saved first. Saved uncommitted changes outside the range
are mapped back to HEAD; selections containing uncommitted edits or crossing
uncommitted deletions are refused with an explanation.

**Inline blame** toggles a current-line annotation in the editor. Moving the
cursor updates the author/commit/summary; the toolbar commit link opens version
browsing. Local lines are marked uncommitted. Annotations are hidden while
unsaved edits are present and refreshed after saving.


## Collapse unchanged Diff regions

The full Diff viewer toolbar includes **Collapse unchanged regions**. The choice
is remembered and synchronized across open viewers, independently of Split or
Unified layout. Folding retains three context lines around changes and hides
unchanged regions of at least eight lines. Monaco's fold controls reveal more
lines (20 at a time) or the entire region. Disable the toggle to show the full
file again. Image diffs and lightweight patch previews are unaffected.

## Commit options, groups and Shelf management

The standalone Commit window provides path speed search and All / Tracked /
Untracked / Conflicts filters in addition to changelists. Only selected visible
files enter the commit. File context menus can append a literal root-relative
rule to `.gitignore` or remove a tracked file from the index while preserving
the working file. Git refuses removal when staged content would be lost; the
staged deletion still needs committing.

Expand **Commit options and message history** to reuse one of the last 50
successful messages recorded by Codeg for that workspace, override the author
(`Name <email>`), add Signed-off-by, explicitly sign/disable signing, or skip
hooks for this commit. Defaults follow the repository's Git configuration.
Signing uses Git's configured GPG/SSH signing program and keys; Codeg does not
store private keys. A signer or hook failure leaves the draft available.

Shelf **Manage / partial restore** persists a display name and group outside
Git and filters the Shelf list by group. Full restore retains its merge/conflict
workflow. Partial restore selects whole files, including untracked/binary
content; it checks all selected patches, keeps the real index unchanged and
retains the Shelf. Overlapping edits are refused rather than partially applied.
Use full restore when a three-way merge is needed. Grouping is by metadata;
partial hunk/line restoration is outside this feature.

Stash creation now offers **Include untracked files** together with
**Keep index**. Ignored files remain excluded. Message and both options are
cleared after successful creation or cancellation; failures retain the choices.

## Advanced history actions and ranges

History rows have selection checkboxes. Shift-click selects the contiguous
range of currently loaded/visible rows; **Advanced history actions** reviews
those exact commits. Cherry-pick runs in oldest-to-newest display order and
Revert in newest-to-oldest order. Merge commits expose numbered parent hashes
for mainline choice. A mixed merge/non-merge batch uses parent 1; parent 2+
merges and root commits can be applied separately. Git's native sequencer retains the remaining
commits on conflicts, with the existing Continue / Abort / Skip controls.

**Reword, preserving merge structure** changes selected commit messages and
rebuilds their affected descendants (maximum 100), preserving each original
tree, author identity/date and ordered merge parents. Other branch refs stay
at their existing commits. This supplements the existing linear rebase editor;
DAG reorder/drop/squash plans are not offered by this mode.

**Split one commit by files** works on a non-root, non-merge ancestor of HEAD.
Select a proper subset of its changed files and supply both messages. The first
commit contains those files; the second contains the remaining changes. Later
commits, including merge resolutions, retain their original trees and parent
structure. Whole-file additions/deletions/binary content are supported through
a temporary Git index. Line/hunk splitting is not offered.

Both rewrite modes build commits without touching the working tree or real
index, retain the previous HEAD under `refs/codeg/history-backups/…`, and switch
the current branch with an expected-old-value update only after construction
and final clean-tree checks. Dirty, detached, stale and unconfirmed published
history is refused. Generated commit signatures are not retained and hooks do
not run; the review requires explicit acknowledgement. Original commits remain
reachable through recovery refs. Changes to other branches or remote branches
are separate operations.
