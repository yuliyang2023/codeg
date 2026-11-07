import { act, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"
import type { FolderDetail } from "@/lib/types"
import { GitShelfTab } from "./git-shelf-tab"

const api = vi.hoisted(() => ({
  gitCommitTools: vi.fn(),
  gitShelfList: vi.fn(),
  gitShelfSave: vi.fn(),
  gitShelfApply: vi.fn(),
  gitShelfDrop: vi.fn(),
  gitShelfDiff: vi.fn(),
  gitStatus: vi.fn(),
}))
const reportConflict = vi.hoisted(() => vi.fn())
vi.mock("@/lib/api", () => api)
vi.mock("next-intl", () => ({ useTranslations: () => (key: string) => key }))
vi.mock("@/hooks/use-git-quick-actions", () => ({
  useGitQuickActions: ({ onCompleted }: { onCompleted: () => void }) => ({
    running: false,
    dialogs: null,
    reportConflict,
    runGitTask: async (
      _: string,
      action: () => Promise<unknown>,
      describe?: (result: unknown) => unknown
    ) => {
      const result = await action()
      describe?.(result)
      onCompleted()
    },
  }),
}))
const folder = { id: 1, path: "/one" } as FolderDetail
const entry = {
  ref_name: "refs/codeg/shelves/a",
  message: "My changes",
  date: "2026-11-07",
}

beforeEach(() => {
  vi.clearAllMocks()
  api.gitCommitTools.mockResolvedValue({ messages: [], shelves: {} })
  api.gitShelfList.mockResolvedValue([entry])
  api.gitShelfSave.mockResolvedValue(undefined)
  api.gitShelfApply.mockResolvedValue({ message: "restored", conflict: null })
  api.gitShelfDrop.mockResolvedValue(undefined)
  api.gitShelfDiff.mockResolvedValue("+saved change")
  api.gitStatus.mockResolvedValue([{ status: "M", file: "selected.txt" }])
})

describe("Shelf", () => {
  it("passes only checked files and opens conflict resolution for a partial restore", async () => {
    api.gitStatus.mockResolvedValue([
      { file: "selected.txt", status: "M" },
      { file: "unchecked.txt", status: "M" },
    ])
    const conflict = {
      operation: "shelf",
      has_conflicts: true,
      conflicted_files: ["selected.txt"],
    }
    api.gitShelfApply.mockResolvedValue({ message: "conflicts", conflict })
    render(<GitShelfTab folder={folder} />)
    await screen.findByText(entry.message)
    fireEvent.click(screen.getByRole("button", { name: "restoreShelf" }))
    await waitFor(() => expect(reportConflict).toHaveBeenCalledWith(conflict))
    fireEvent.click(screen.getByRole("button", { name: "saveShelf" }))
    fireEvent.click(
      await screen.findByRole("checkbox", { name: "unchecked.txt" })
    )
    const input = screen.getByLabelText("shelfName")
    fireEvent.change(input, { target: { value: "selected work" } })
    fireEvent.submit(input.closest("form")!)
    await waitFor(() =>
      expect(api.gitShelfSave).toHaveBeenCalledWith("/one", "selected work", [
        "selected.txt",
      ])
    )
  })
  it("restores without deleting and requires confirmation for deletion", async () => {
    render(<GitShelfTab folder={folder} />)
    await screen.findByText(entry.message)
    fireEvent.click(screen.getByRole("button", { name: "restoreShelf" }))
    await waitFor(() =>
      expect(api.gitShelfApply).toHaveBeenCalledWith("/one", entry.ref_name)
    )
    expect(api.gitShelfDrop).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole("button", { name: "deleteShelf" }))
    expect(api.gitShelfDrop).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole("button", { name: "cancel" }))
    expect(api.gitShelfDrop).not.toHaveBeenCalled()
  })

  it("does not save on the Enter key used to confirm an IME composition", async () => {
    render(<GitShelfTab folder={folder} />)
    await screen.findByText(entry.message)
    fireEvent.click(screen.getByRole("button", { name: "saveShelf" }))
    await screen.findByRole("checkbox", { name: "selected.txt" })
    const input = screen.getByLabelText("shelfName")
    fireEvent.change(input, { target: { value: "中文修改" } })
    fireEvent.compositionStart(input)
    expect(
      fireEvent.keyDown(input, {
        key: "Enter",
        keyCode: 229,
        isComposing: true,
      })
    ).toBe(false)
    expect(api.gitShelfSave).not.toHaveBeenCalled()
    fireEvent.compositionEnd(input)
    fireEvent.submit(input.closest("form")!)
    await waitFor(() =>
      expect(api.gitShelfSave).toHaveBeenCalledWith("/one", "中文修改", [
        "selected.txt",
      ])
    )
  })

  it("ignores a preview request that completes after its dialog is closed", async () => {
    let resolve!: (value: string) => void
    api.gitShelfDiff.mockReturnValue(
      new Promise<string>((done) => {
        resolve = done
      })
    )
    render(<GitShelfTab folder={folder} />)
    await screen.findByText(entry.message)
    fireEvent.click(screen.getByRole("button", { name: "preview" }))
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" })
    await act(async () => resolve("old preview"))
    expect(screen.queryByText("old preview")).not.toBeInTheDocument()
  })
})
