import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"
import { AmendCommitDialog } from "./amend-commit-dialog"

const api = vi.hoisted(() => ({
  gitAmend: vi.fn(),
  gitAmendInfo: vi.fn(),
  gitStatus: vi.fn(),
}))
vi.mock("@/lib/api", () => api)
vi.mock("next-intl", () => ({ useTranslations: () => (key: string) => key }))

beforeEach(() => {
  vi.clearAllMocks()
  api.gitAmendInfo.mockResolvedValue({
    head: "1234567890",
    message: "previous",
    published: true,
  })
  api.gitStatus.mockResolvedValue([
    { file: "a.txt", status: "M" },
    { file: "b.txt", status: "M" },
  ])
  api.gitAmend.mockResolvedValue({ head: "updated" })
})

describe("Amend commit", () => {
  it("loads the previous message, warns for published history and only submits checked files", async () => {
    const done = vi.fn()
    render(
      <AmendCommitDialog
        path="/repo"
        open
        onOpenChange={vi.fn()}
        onCompleted={done}
      />
    )
    const box = await screen.findByLabelText("amendMessage")
    expect(box).toHaveValue("previous")
    expect(screen.getByText("amendPublished")).toBeVisible()
    expect(screen.getByRole("checkbox", { name: "a.txt" })).not.toBeChecked()
    fireEvent.click(screen.getByRole("checkbox", { name: "a.txt" }))
    fireEvent.change(box, { target: { value: "revised" } })
    fireEvent.click(screen.getByRole("button", { name: "amend" }))
    await waitFor(() =>
      expect(api.gitAmend).toHaveBeenCalledWith(
        "/repo",
        "revised",
        ["a.txt"],
        "1234567890"
      )
    )
    expect(done).toHaveBeenCalledOnce()
  })

  it("keeps the dialog and draft when the operation is refused", async () => {
    api.gitAmend.mockRejectedValue(new Error("HEAD changed"))
    const done = vi.fn()
    render(
      <AmendCommitDialog
        path="/repo"
        open
        onOpenChange={vi.fn()}
        onCompleted={done}
      />
    )
    await screen.findByLabelText("amendMessage")
    fireEvent.click(screen.getByRole("button", { name: "amend" }))
    expect(await screen.findByRole("alert")).toHaveTextContent("HEAD changed")
    expect(screen.getByLabelText("amendMessage")).toHaveValue("previous")
    expect(done).not.toHaveBeenCalled()
  })
})
