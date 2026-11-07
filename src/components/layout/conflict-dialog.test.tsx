import { act, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import { beforeEach, describe, expect, it, vi } from "vitest"
import messages from "@/i18n/messages/en.json"
import { ConflictDialog } from "./conflict-dialog"

const api = vi.hoisted(() => ({
  gitListConflicts: vi.fn(),
  gitAbortOperation: vi.fn(),
  gitContinueOperation: vi.fn(),
  openMergeWindow: vi.fn(),
}))
const listeners = vi.hoisted(
  () =>
    new Map<string, (payload: { folder_id: number; file?: string }) => void>()
)
vi.mock("@/lib/api", () => api)
vi.mock("@/lib/platform", () => ({
  subscribe: async (
    name: string,
    fn: (payload: { folder_id: number; file?: string }) => void
  ) => {
    listeners.set(name, fn)
    return () => listeners.delete(name)
  },
}))
beforeEach(() => {
  vi.clearAllMocks()
  listeners.clear()
  api.gitListConflicts.mockResolvedValue([])
  api.gitContinueOperation.mockResolvedValue(undefined)
})

describe("conflict workflow", () => {
  it("offers restored-work resolution without an unsafe abort and finishes as a restore", async () => {
    const onResolved = vi.fn()
    render(
      <NextIntlClientProvider locale="en" messages={messages}>
        <ConflictDialog
          folderId={1}
          folderPath="/repo"
          conflictInfo={{
            operation: "shelf",
            has_conflicts: true,
            conflicted_files: ["a.txt"],
          }}
          onClose={vi.fn()}
          onResolved={onResolved}
        />
      </NextIntlClientProvider>
    )
    expect(
      screen.queryByRole("button", {
        name: messages.Folder.branchDropdown.conflict.abort,
      })
    ).not.toBeInTheDocument()
    await waitFor(() =>
      expect(listeners.has("folder://merge-conflict-resolved")).toBe(true)
    )
    act(() =>
      listeners.get("folder://merge-conflict-resolved")!({
        folder_id: 1,
        file: "a.txt",
      })
    )
    fireEvent.click(
      screen.getByRole("button", {
        name: messages.Folder.gitWorkbench.finishRestore,
      })
    )
    await waitFor(() =>
      expect(api.gitContinueOperation).toHaveBeenCalledWith("/repo", "shelf")
    )
    expect(onResolved).toHaveBeenCalledOnce()
    expect(api.gitAbortOperation).not.toHaveBeenCalled()
  })

  it("shows new files when continuing a rebase encounters the next conflict", async () => {
    api.gitContinueOperation.mockRejectedValue(new Error("next conflict"))
    api.gitListConflicts.mockResolvedValue(["b.txt"])
    render(
      <NextIntlClientProvider locale="en" messages={messages}>
        <ConflictDialog
          folderId={1}
          folderPath="/repo"
          conflictInfo={{
            operation: "rebase",
            has_conflicts: true,
            conflicted_files: ["a.txt"],
          }}
          onClose={vi.fn()}
          onResolved={vi.fn()}
        />
      </NextIntlClientProvider>
    )
    await waitFor(() =>
      expect(listeners.has("folder://merge-conflict-resolved")).toBe(true)
    )
    act(() =>
      listeners.get("folder://merge-conflict-resolved")!({
        folder_id: 1,
        file: "a.txt",
      })
    )
    fireEvent.click(
      screen.getByRole("button", {
        name: messages.Folder.branchDropdown.conflict.completeMerge,
      })
    )
    expect(await screen.findByText("b.txt")).toBeVisible()
  })
})
