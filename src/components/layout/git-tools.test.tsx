import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import type { ReactNode } from "react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import messages from "@/i18n/messages/en.json"
import type { FolderDetail, GitChangelists } from "@/lib/types"
import { GitChangelistsTab, changelistForFile } from "./git-changelists"
import { GitSmartTab } from "./git-smart-tab"
import { GitPatchTab } from "./git-patch-tab"
import { LocalHistoryTab } from "./local-history-tab"
const scene = vi.hoisted(() => ({
  lists: {
    revision: 1,
    default_id: "default",
    lists: [
      { id: "default", name: "Default", files: [] },
      { id: "feature", name: "Feature", files: ["b"] },
    ],
  } as GitChangelists,
  update: vi.fn(),
  smart: vi.fn(),
  state: vi.fn(),
  report: vi.fn(),
  preview: vi.fn(),
  apply: vi.fn(),
  restore: vi.fn(),
  content: vi.fn(),
}))
vi.mock("@/lib/api", () => ({
  gitChangelists: async () => scene.lists,
  gitChangelistUpdate: scene.update,
  gitStatus: async () => [
    { file: "a", status: " M" },
    { file: "b", status: "M " },
  ],
  gitListAllBranches: async () => ({ local: ["main", "other"] }),
  gitSmartRun: scene.smart,
  gitSmartState: scene.state,
  gitSmartResume: vi.fn(),
  gitPatchCreate: async () => "patch",
  gitPatchPreview: scene.preview,
  gitPatchApply: scene.apply,
  localHistoryList: async () => [
    {
      id: "snap",
      file: "a",
      time: "2026-11-01",
      label: "Old",
      reason: "save",
      blob: "blob",
      size: 4,
    },
  ],
  localHistoryContent: scene.content,
  localHistoryRestore: scene.restore,
  localHistoryCapture: vi.fn(),
}))
vi.mock("@/hooks/use-git-quick-actions", () => ({
  useGitQuickActions: () => ({ reportConflict: scene.report, dialogs: null }),
}))
vi.mock("@/components/diff/diff-viewer", () => ({
  DiffViewer: ({
    original,
    modified,
  }: {
    original: string
    modified: string
  }) => (
    <div>
      {original} → {modified}
    </div>
  ),
}))
vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))
const folder = { id: 1, path: "/repo" } as FolderDetail
function mount(children: ReactNode) {
  return render(
    <NextIntlClientProvider locale="en" messages={messages}>
      {children}
    </NextIntlClientProvider>
  )
}
afterEach(cleanup)
beforeEach(() => {
  vi.clearAllMocks()
  scene.state.mockResolvedValue(null)
  scene.content.mockResolvedValue({
    entry: { id: "snap", file: "a" },
    original: "old",
    current: "new",
    current_hash: "expected",
    binary: false,
  })
  scene.restore.mockResolvedValue(undefined)
  scene.preview.mockResolvedValue({
    applicable: true,
    summary: "a",
    error: null,
  })
})
describe("Git workflow tools", () => {
  it("assigns unknown paths to the default list and moves checked files with a revision guard", async () => {
    expect(changelistForFile(scene.lists, "unknown")).toBe("default")
    scene.update.mockResolvedValue(scene.lists)
    mount(<GitChangelistsTab folder={folder} />)
    await screen.findByRole("combobox", { name: "Changelist" })
    fireEvent.click(await screen.findByRole("checkbox", { name: "a" }))
    fireEvent.change(
      screen.getByRole("combobox", { name: "Move to changelist" }),
      { target: { value: "feature" } }
    )
    fireEvent.click(screen.getByRole("button", { name: "Move" }))
    await waitFor(() =>
      expect(scene.update).toHaveBeenCalledWith("/repo", 1, "move", {
        id: "feature",
        files: ["a"],
      })
    )
  })
  it("requires confirmation for smart operations and reports restore conflicts", async () => {
    const conflict = {
      operation: "stash",
      has_conflicts: true,
      conflicted_files: ["a"],
    }
    scene.smart.mockResolvedValue({
      message: "paused",
      pending: true,
      conflict,
      operation_failed: false,
    })
    mount(<GitSmartTab folder={folder} />)
    await screen.findByRole("option", { name: "other" })
    fireEvent.change(
      screen.getByRole("combobox", { name: "Target local branch" }),
      { target: { value: "other" } }
    )
    fireEvent.click(screen.getByRole("button", { name: "Save and run" }))
    expect(scene.smart).not.toHaveBeenCalled()
    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Confirm",
      })
    )
    await waitFor(() => expect(scene.report).toHaveBeenCalledWith(conflict))
    expect(scene.smart).toHaveBeenCalledWith("/repo", "checkout", "other")
  })
  it("invalidates patch approval whenever the patch or apply options change", async () => {
    mount(<GitPatchTab folder={folder} />)
    fireEvent.change(screen.getByLabelText("Patch contents"), {
      target: { value: "first patch" },
    })
    fireEvent.click(screen.getByRole("button", { name: "Check patch" }))
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Apply patch" })).toBeEnabled()
    )
    fireEvent.change(screen.getByLabelText("Patch contents"), {
      target: { value: "different patch" },
    })
    expect(screen.getByRole("button", { name: "Apply patch" })).toBeDisabled()
    expect(scene.apply).not.toHaveBeenCalled()
  })
  it("restores only after confirmation with the hash from the displayed comparison", async () => {
    mount(<LocalHistoryTab folder={folder} />)
    fireEvent.click(await screen.findByText("a · Old"))
    const restore = await screen.findByRole("button", {
      name: "Restore snapshot",
    })
    fireEvent.click(restore)
    expect(scene.restore).not.toHaveBeenCalled()
    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Restore snapshot",
      })
    )
    await waitFor(() =>
      expect(scene.restore).toHaveBeenCalledWith("/repo", "snap", "expected")
    )
  })
})
