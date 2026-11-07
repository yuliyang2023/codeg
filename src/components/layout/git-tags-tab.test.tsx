import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import messages from "@/i18n/messages/en.json"
import { GitTagsTab } from "./git-tags-tab"
import type { FolderDetail } from "@/lib/types"
const scene = vi.hoisted(() => ({
  tag: {
    name: "v1",
    object: "tag-object",
    commit: "commit",
    annotated: true,
    message: "release",
    date: "",
  },
  create: vi.fn(),
  action: vi.fn(),
}))
vi.mock("@/lib/api", () => ({
  gitTags: async () => [scene.tag],
  gitListRemotes: async () => [{ name: "origin", url: "local" }],
  gitTagCreate: scene.create,
  gitTagAction: scene.action,
}))
vi.mock("@/hooks/use-git-quick-actions", () => ({
  useGitQuickActions: () => ({
    running: false,
    reportConflict: vi.fn(),
    dialogs: null,
  }),
}))
vi.mock("@/contexts/workspace-context", () => ({
  useWorkspaceActions: () => ({ openCommitDiff: vi.fn() }),
}))
vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}))
afterEach(cleanup)
beforeEach(() => {
  scene.create.mockReset()
  scene.action.mockReset()
})
function mount() {
  render(
    <NextIntlClientProvider locale="en" messages={messages}>
      <GitTagsTab folder={{ id: 1, path: "/repo" } as FolderDetail} />
    </NextIntlClientProvider>
  )
}
describe("tag operations", () => {
  it("retains a refused creation draft for correction", async () => {
    scene.create.mockRejectedValue(new Error("tag exists"))
    mount()
    await screen.findByText("v1")
    fireEvent.click(screen.getByRole("button", { name: "Create tag" }))
    fireEvent.change(screen.getByLabelText("Tag name"), {
      target: { value: "v1" },
    })
    fireEvent.change(screen.getByLabelText("Annotation (optional)"), {
      target: { value: "new release" },
    })
    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Create tag",
      })
    )
    expect(await screen.findByRole("alert")).toHaveTextContent("tag exists")
    expect(screen.getByLabelText("Annotation (optional)")).toHaveValue(
      "new release"
    )
  })
  it("requires confirmation before deleting the selected remote tag", async () => {
    scene.action.mockResolvedValue({ conflict: null, message: "deleted" })
    mount()
    await screen.findByText("v1")
    fireEvent.click(screen.getByRole("button", { name: "Delete remote tag" }))
    expect(scene.action).not.toHaveBeenCalled()
    const dialog = screen.getByRole("dialog")
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Delete remote tag" })
    )
    await waitFor(() =>
      expect(scene.action).toHaveBeenCalledWith(
        "/repo",
        scene.tag,
        "delete-remote",
        "origin"
      )
    )
  })
})
