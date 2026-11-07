import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"
import type { FolderDetail } from "@/lib/types"
import { GitOperationBanner } from "./git-operation-banner"

const api = vi.hoisted(() => ({
  gitOperationState: vi.fn(),
  gitContinueOperation: vi.fn(),
  gitAbortOperation: vi.fn(),
  gitSkipOperation: vi.fn(),
}))
const report = vi.hoisted(() => vi.fn())
vi.mock("@/lib/api", () => api)
vi.mock("next-intl", () => ({ useTranslations: () => (key: string) => key }))
vi.mock("@/hooks/use-git-quick-actions", () => ({
  useGitQuickActions: ({ onCompleted }: { onCompleted: () => void }) => ({
    running: false,
    reportConflict: report,
    dialogs: null,
    runGitTask: async (
      _: string,
      action: () => Promise<unknown>,
      describe?: (result: unknown) => void
    ) => {
      const result = await action()
      describe?.(result)
      onCompleted()
    },
  }),
}))
const folder = { id: 1, path: "/repo" } as FolderDetail

beforeEach(() => {
  vi.clearAllMocks()
  api.gitContinueOperation.mockResolvedValue(undefined)
})
describe("ongoing Git operation", () => {
  it("recovers a restored-change conflict and offers resolution without an abort or skip", async () => {
    const conflict = {
      operation: "shelf",
      has_conflicts: true,
      conflicted_files: ["a.txt"],
    }
    api.gitOperationState.mockResolvedValue(conflict)
    render(<GitOperationBanner folder={folder} />)
    fireEvent.click(
      await screen.findByRole("button", { name: "resolveConflicts" })
    )
    expect(report).toHaveBeenCalledWith(conflict)
    expect(
      screen.queryByRole("button", { name: "abortOperation" })
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole("button", { name: "skipOperation" })
    ).not.toBeInTheDocument()
  })
  it("finishes resolved restoration without creating a commit", async () => {
    api.gitOperationState.mockResolvedValue({
      operation: "stash",
      has_conflicts: false,
      conflicted_files: [],
    })
    render(<GitOperationBanner folder={folder} />)
    fireEvent.click(
      await screen.findByRole("button", { name: "finishRestore" })
    )
    await waitFor(() =>
      expect(api.gitContinueOperation).toHaveBeenCalledWith("/repo", "stash")
    )
    expect(api.gitAbortOperation).not.toHaveBeenCalled()
  })
  it("requires confirmation before aborting a cherry-pick", async () => {
    api.gitOperationState.mockResolvedValue({
      operation: "cherry-pick",
      has_conflicts: true,
      conflicted_files: ["a.txt"],
    })
    render(<GitOperationBanner folder={folder} />)
    fireEvent.click(
      await screen.findByRole("button", { name: "abortOperation" })
    )
    expect(api.gitAbortOperation).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole("button", { name: "confirm" }))
    await waitFor(() =>
      expect(api.gitAbortOperation).toHaveBeenCalledWith("/repo", "cherry-pick")
    )
  })
})
