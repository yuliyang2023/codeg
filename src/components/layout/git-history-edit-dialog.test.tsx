import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import messages from "@/i18n/messages/en.json"
import { GitHistoryEditDialog } from "./git-history-edit-dialog"
const scene = vi.hoisted(() => ({
  plan: {
    head: "head",
    base: "base",
    parent: "parent",
    published: false,
    commits: [
      { hash: "base", message: "old" },
      { hash: "head", message: "latest" },
    ],
  },
  edit: vi.fn(),
  undo: vi.fn(),
}))
vi.mock("@/lib/api", () => ({
  gitHistoryPlan: async () => scene.plan,
  gitHistoryEdit: scene.edit,
  gitUndoCommit: scene.undo,
}))
vi.mock("sonner", () => ({ toast: { success: vi.fn(), warning: vi.fn() } }))
afterEach(cleanup)
beforeEach(() => {
  scene.plan.published = false
  scene.edit.mockReset()
  scene.undo.mockReset()
})
function mount(undo = false) {
  const onClose = vi.fn()
  const onCompleted = vi.fn()
  const onConflict = vi.fn()
  render(
    <NextIntlClientProvider locale="en" messages={messages}>
      <GitHistoryEditDialog
        path="/repo"
        base="base"
        undo={undo}
        onClose={onClose}
        onCompleted={onCompleted}
        onConflict={onConflict}
      />
    </NextIntlClientProvider>
  )
  return { onClose, onCompleted, onConflict }
}
describe("history edit review", () => {
  it("keeps a refused reword draft and requires an actual plan change", async () => {
    scene.edit.mockRejectedValue(new Error("hook refused"))
    const { onClose } = mount()
    const apply = await screen.findByRole("button", {
      name: "Apply history plan",
    })
    await screen.findByLabelText("Action for base")
    expect(apply).toBeDisabled()
    fireEvent.change(screen.getByLabelText("Action for base"), {
      target: { value: "reword" },
    })
    fireEvent.change(screen.getByLabelText("New message for base"), {
      target: { value: "new message" },
    })
    fireEvent.click(apply)
    expect(await screen.findByRole("alert")).toHaveTextContent("hook refused")
    expect(screen.getByLabelText("New message for base")).toHaveValue(
      "new message"
    )
    expect(onClose).not.toHaveBeenCalled()
  })
  it("requires published-history confirmation and hands conflicts to recovery", async () => {
    scene.plan.published = true
    const conflict = { operation: "rebase", has_conflicts: true, files: ["a"] }
    scene.edit.mockResolvedValue({ recovery_ref: "backup", conflict })
    const { onCompleted, onConflict } = mount()
    await screen.findByLabelText("Action for head")
    fireEvent.change(screen.getByLabelText("Action for head"), {
      target: { value: "fixup" },
    })
    const apply = screen.getByRole("button", { name: "Apply history plan" })
    expect(apply).toBeDisabled()
    fireEvent.click(screen.getByRole("checkbox"))
    fireEvent.click(apply)
    await waitFor(() => expect(onCompleted).toHaveBeenCalled())
    expect(scene.edit).toHaveBeenCalledWith(
      "/repo",
      scene.plan,
      expect.arrayContaining([
        expect.objectContaining({ hash: "head", action: "fixup" }),
      ]),
      true
    )
    expect(onConflict).toHaveBeenCalledWith(conflict)
  })
})
