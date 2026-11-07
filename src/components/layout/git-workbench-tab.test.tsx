import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { GitWorkbenchTab } from "./git-workbench-tab"

vi.mock("./git-changelists", () => ({
  GitChangelistsTab: () => <div>Changelist tools</div>,
}))
vi.mock("./git-smart-tab", () => ({
  GitSmartTab: () => <div>Smart tools</div>,
}))
vi.mock("./git-patch-tab", () => ({
  GitPatchTab: () => <div>Patch tools</div>,
}))
vi.mock("./local-history-tab", () => ({
  LocalHistoryTab: () => <div>Local snapshots</div>,
}))
vi.mock("./git-console-tab", () => ({
  GitConsoleTab: () => <div>Git commands</div>,
}))

vi.mock("./git-operation-banner", () => ({ GitOperationBanner: () => null }))

const scene = vi.hoisted(() => ({
  folder: { id: 1, path: "/one" } as { id: number; path: string } | null,
}))
vi.mock("next-intl", () => ({ useTranslations: () => (key: string) => key }))
vi.mock("@/contexts/active-folder-context", () => ({
  useActiveFolder: () => ({ activeFolder: scene.folder }),
}))
vi.mock("@/contexts/workspace-context", () => ({
  useWorkspaceActions: () => ({ openFilePreview: vi.fn() }),
}))
vi.mock("@/hooks/use-git-quick-actions", () => ({
  useGitQuickActions: () => ({
    running: false,
    openStashDialog: vi.fn(),
    dialogs: null,
  }),
}))
vi.mock("./aux-panel-no-folder-empty", () => ({
  AuxPanelNoFolderEmpty: () => <div>No folder</div>,
}))
vi.mock("./aux-panel-git-changes-tab", () => ({
  GitChangesTab: () => <textarea aria-label="message" />,
}))
vi.mock("./aux-panel-git-log-tab", () => ({
  GitLogTab: () => <div>Commit history</div>,
}))
vi.mock("./git-shelf-tab", () => ({
  GitShelfTab: ({ folder }: { folder: { path: string } }) => (
    <div>Shelf {folder.path}</div>
  ),
}))
vi.mock("./unstash-dialog", () => ({
  StashWorkspace: ({ folderPath }: { folderPath: string }) => (
    <div>Stashes {folderPath}</div>
  ),
}))
vi.mock("./branch-dropdown", () => ({
  BranchDropdown: ({ inline }: { inline: boolean }) => (
    <div>{inline ? "Inline branches" : "Popup branches"}</div>
  ),
}))

describe("Git workbench", () => {
  it("makes added tools reachable without horizontal tab scrolling", () => {
    scene.folder = { id: 1, path: "/one" }
    render(<GitWorkbenchTab />)
    fireEvent.change(screen.getByRole("combobox", { name: "tools" }), {
      target: { value: "changelists" },
    })
    expect(screen.getByText("Changelist tools")).toBeVisible()
    fireEvent.change(screen.getByRole("combobox", { name: "tools" }), {
      target: { value: "localHistory" },
    })
    expect(screen.getByText("Local snapshots")).toBeVisible()
  })

  it("preserves history and commit drafts while switching workflow tabs", () => {
    scene.folder = { id: 1, path: "/one" }
    render(<GitWorkbenchTab />)
    expect(screen.getByText("Commit history")).toBeVisible()
    const select = (name: string) =>
      fireEvent.mouseDown(screen.getByRole("tab", { name }), {
        button: 0,
        ctrlKey: false,
      })
    select("commit")
    fireEvent.change(screen.getByLabelText("message"), {
      target: { value: "My draft" },
    })
    select("shelf")
    expect(screen.getByText("Shelf /one")).toBeVisible()
    select("stash")
    expect(screen.getByText("Stashes /one")).toBeVisible()
    select("branches")
    expect(screen.getByText("Inline branches")).toBeVisible()
    select("commit")
    expect(screen.getByLabelText("message")).toHaveValue("My draft")
  })

  it("resets repository-specific state when changing folders", () => {
    scene.folder = { id: 1, path: "/one" }
    const view = render(<GitWorkbenchTab />)
    fireEvent.mouseDown(screen.getByRole("tab", { name: "shelf" }), {
      button: 0,
      ctrlKey: false,
    })
    expect(screen.getByText("Shelf /one")).toBeVisible()
    scene.folder = { id: 2, path: "/two" }
    view.rerender(<GitWorkbenchTab />)
    expect(screen.queryByText("Shelf /one")).not.toBeInTheDocument()
    expect(screen.getByRole("tab", { name: "history" })).toHaveAttribute(
      "aria-selected",
      "true"
    )
    scene.folder = null
    view.rerender(<GitWorkbenchTab />)
    expect(screen.getByText("No folder")).toBeVisible()
  })
})
