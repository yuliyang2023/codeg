"use client"

import { useState } from "react"
import { useTranslations } from "next-intl"
import { Tabs as TabsPrimitive } from "radix-ui"
import { Button } from "@/components/ui/button"
import { useActiveFolder } from "@/contexts/active-folder-context"
import { useWorkspaceActions } from "@/contexts/workspace-context"
import { joinFsPath } from "@/lib/path-utils"
import { useGitQuickActions } from "@/hooks/use-git-quick-actions"
import { AuxPanelNoFolderEmpty } from "./aux-panel-no-folder-empty"
import { BranchDropdown } from "./branch-dropdown"
import { GitChangesTab } from "./aux-panel-git-changes-tab"
import { GitLogTab } from "./aux-panel-git-log-tab"
import { GitShelfTab } from "./git-shelf-tab"
import { StashWorkspace } from "./unstash-dialog"
import { GitTagsTab } from "./git-tags-tab"
import { GitChangelistsTab } from "./git-changelists"
import { GitSmartTab } from "./git-smart-tab"
import { GitPatchTab } from "./git-patch-tab"
import { LocalHistoryTab } from "./local-history-tab"
import { GitRevisionBrowser } from "./git-revision-browser"
import { GitConsoleTab } from "./git-console-tab"
import { GitOperationBanner } from "./git-operation-banner"
import type { FolderDetail } from "@/lib/types"

const SECTIONS = [
  "commit",
  "shelf",
  "stash",
  "branches",
  "tags",
  "history",
  "changelists",
  "smart",
  "patch",
  "localHistory",
  "console",
  "revisions",
] as const
type Section = (typeof SECTIONS)[number]

function StashTab({ folder }: { folder: FolderDetail }) {
  const { openFilePreview } = useWorkspaceActions()
  const t = useTranslations("Folder.gitWorkbench")
  const [revision, setRevision] = useState(0)
  const actions = useGitQuickActions({
    folderId: folder.id,
    folderPath: folder.path,
    onCompleted: () => setRevision((value) => value + 1),
  })
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 gap-2 border-b p-2">
        <Button
          size="sm"
          variant="ghost"
          disabled={actions.running}
          onClick={actions.openStashDialog}
        >
          {t("saveStash")}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={actions.running}
          onClick={() => setRevision((value) => value + 1)}
        >
          {t("refresh")}
        </Button>
      </div>
      <div className="min-h-0 flex-1">
        <StashWorkspace
          key={revision}
          folderPath={folder.path}
          folderId={folder.id}
          compact
          onOpenSource={async (file, line) => {
            await openFilePreview(joinFsPath(folder.path, file), { line })
          }}
        />
      </div>
      {actions.dialogs}
    </div>
  )
}

/** Native Git workflows inspired by IdeaGitTool; all operations use Transport. */
export function GitWorkbenchTab() {
  const { activeFolder } = useActiveFolder()
  return activeFolder ? (
    <GitWorkbench
      key={`${activeFolder.id}:${activeFolder.path}`}
      folder={activeFolder}
    />
  ) : (
    <AuxPanelNoFolderEmpty />
  )
}

function GitWorkbench({ folder }: { folder: FolderDetail }) {
  const t = useTranslations("Folder.gitWorkbench")
  // History remains the initial view of the existing "Commits" rail entry.
  const [section, setSection] = useState<Section>("history")
  const [historyRefreshToken, setHistoryRefreshToken] = useState(0)
  const [visited, setVisited] = useState<Set<Section>>(
    () => new Set(["history"])
  )
  return (
    <TabsPrimitive.Root
      value={section}
      onValueChange={(value) => {
        if (!SECTIONS.includes(value as Section)) return
        const next = value as Section
        if (next === "history" && section !== "history") {
          setHistoryRefreshToken((value) => value + 1)
        }
        setSection(next)
        setVisited((previous) => new Set(previous).add(next))
      }}
      orientation="horizontal"
      className="flex h-full min-h-0 flex-col"
    >
      {section === "revisions" && (
        <GitRevisionBrowser
          path={folder.path}
          onClose={() => setSection("history")}
          onChanged={() => setHistoryRefreshToken((value) => value + 1)}
        />
      )}
      <GitOperationBanner folder={folder} />
      <div className="shrink-0 overflow-x-auto border-b px-1">
        {/* Explicit layout isolates this row from the outer vertical tab rail's
            named Tailwind group, while Radix retains keyboard navigation. */}
        <TabsPrimitive.List
          className="flex h-9 min-w-max items-stretch gap-1"
          aria-label={t("title")}
        >
          {SECTIONS.slice(0, 6).map((id) => (
            <TabsPrimitive.Trigger
              key={id}
              value={id}
              className="shrink-0 border-b-2 border-transparent px-2 text-xs text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring data-[state=active]:border-foreground data-[state=active]:text-foreground"
            >
              {t(id)}
            </TabsPrimitive.Trigger>
          ))}
        </TabsPrimitive.List>
      </div>
      <div className="shrink-0 border-b px-2 py-1">
        <select
          aria-label={t("tools")}
          className="w-full rounded border bg-background px-2 py-1 text-xs"
          value={SECTIONS.indexOf(section) >= 6 ? section : ""}
          onChange={(event) => {
            const next = event.target.value as Section
            if (!SECTIONS.includes(next)) return
            setSection(next)
            setVisited((previous) => new Set(previous).add(next))
          }}
        >
          <option value="" disabled>
            {t("tools")}
          </option>
          {SECTIONS.slice(6).map((id) => (
            <option key={id} value={id}>
              {t(id)}
            </option>
          ))}
        </select>
      </div>
      <TabsPrimitive.Content
        value="changelists"
        className="m-0 min-h-0 flex-1 overflow-hidden"
      >
        <GitChangelistsTab folder={folder} />
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="smart"
        className="m-0 min-h-0 flex-1 overflow-auto"
      >
        <GitSmartTab folder={folder} />
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="patch"
        className="m-0 min-h-0 flex-1 overflow-hidden"
      >
        <GitPatchTab folder={folder} />
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="localHistory"
        className="m-0 min-h-0 flex-1 overflow-hidden"
      >
        <LocalHistoryTab folder={folder} />
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="console"
        className="m-0 min-h-0 flex-1 overflow-hidden"
      >
        <GitConsoleTab folder={folder} />
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="commit"
        forceMount
        className="m-0 min-h-0 flex-1 overflow-hidden data-[state=inactive]:hidden"
      >
        {visited.has("commit") && <GitChangesTab showOperationBanner={false} />}
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="shelf"
        className="m-0 min-h-0 flex-1 overflow-hidden"
      >
        <GitShelfTab folder={folder} />
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="stash"
        className="m-0 min-h-0 flex-1 overflow-hidden"
      >
        <StashTab folder={folder} />
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="branches"
        className="m-0 min-h-0 flex-1 overflow-auto"
      >
        <BranchDropdown folder={folder} isChatMode={false} inline />
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="tags"
        className="m-0 min-h-0 flex-1 overflow-hidden"
      >
        <GitTagsTab folder={folder} />
      </TabsPrimitive.Content>
      <TabsPrimitive.Content
        value="history"
        forceMount
        className="m-0 min-h-0 flex-1 overflow-hidden data-[state=inactive]:hidden"
      >
        {visited.has("history") && (
          <GitLogTab refreshToken={historyRefreshToken} />
        )}
      </TabsPrimitive.Content>
    </TabsPrimitive.Root>
  )
}
