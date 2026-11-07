import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import type { ReactNode } from "react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import messages from "@/i18n/messages/en.json"
import { GitFileHistoryDialog } from "./git-file-history"
const scene = vi.hoisted(() => ({
  history: vi.fn(),
  blame: vi.fn(),
  diff: vi.fn(),
  source: vi.fn(),
}))
vi.mock("@/lib/api", () => ({
  gitFileHistory: scene.history,
  gitBlame: scene.blame,
}))
vi.mock("@/contexts/workspace-context", () => ({
  useWorkspaceActions: () => ({
    openCommitDiff: scene.diff,
    openFilePreview: scene.source,
  }),
}))
vi.mock("virtua", () => ({
  Virtualizer: ({
    data,
    children,
  }: {
    data: unknown[]
    children: (row: unknown) => ReactNode
  }) => <>{data.map(children)}</>,
}))
afterEach(cleanup)
beforeEach(() => {
  scene.history.mockReset()
  scene.blame.mockReset()
  scene.diff.mockReset()
  scene.diff.mockResolvedValue(undefined)
})
function mount(view: "history" | "blame") {
  render(
    <NextIntlClientProvider locale="en" messages={messages}>
      <GitFileHistoryDialog
        target={{ folderId: 8, path: "/repo", file: "new.txt", view }}
        onClose={vi.fn()}
      />
    </NextIntlClientProvider>
  )
}
describe("file history and blame navigation", () => {
  it("opens the original commit across renamed file history", async () => {
    scene.history.mockResolvedValue({
      entries: [
        {
          full_hash: "original",
          hash: "original",
          message: "Before rename",
          author: "A",
          date: "2026-11-01",
        },
      ],
    })
    mount("history")
    fireEvent.click(await screen.findByText("Before rename"))
    await waitFor(() =>
      expect(scene.diff).toHaveBeenCalledWith(
        "original",
        undefined,
        "Before rename",
        { folderId: 8 }
      )
    )
  })
  it("uses blame's original filename and never opens a zero-hash commit", async () => {
    scene.blame.mockResolvedValue([
      {
        commit: "1234567890",
        original_line: 1,
        line: 1,
        author: "A",
        file: "old.txt",
        summary: "original",
        content: "code  ",
        uncommitted: false,
      },
      {
        commit: "00000000",
        line: 2,
        author: "You",
        file: "new.txt",
        summary: "local",
        content: "local",
        uncommitted: true,
      },
    ])
    mount("blame")
    fireEvent.click(await screen.findByRole("button", { name: "12345678" }))
    await waitFor(() =>
      expect(scene.diff).toHaveBeenCalledWith(
        "1234567890",
        "old.txt",
        "original",
        { folderId: 8 }
      )
    )
    expect(screen.getByRole("button", { name: "Local" })).toBeDisabled()
  })
})
