import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import messages from "@/i18n/messages/en.json"
import { GitHistoryAdvancedDialog } from "./git-history-advanced-dialog"
const api = vi.hoisted(() => vi.fn())
vi.mock("@/lib/api", () => ({ gitHistoryAdvanced: api }))
afterEach(cleanup)
beforeEach(() => {
  api.mockReset()
})
const commits = ["new", "old"]
function mount(
  rows: { hash: string; parents: string[]; message: string; files: string[] }[],
  published = false
) {
  api.mockImplementation(async (_path, r) =>
    r.operation === "inspect"
      ? { head: "head", published, commits: rows }
      : { conflict: null, recovery_ref: "backup" }
  )
  const close = vi.fn(),
    done = vi.fn(),
    conflict = vi.fn()
  render(
    <NextIntlClientProvider locale="en" messages={messages}>
      <GitHistoryAdvancedDialog
        path="/repo"
        commits={commits}
        onClose={close}
        onCompleted={done}
        onConflict={conflict}
      />
    </NextIntlClientProvider>
  )
  return { close, done, conflict }
}
it("reviews merge parent choice and oldest-first batch order", async () => {
  mount([
    { hash: "new", parents: ["p1", "p2"], message: "merge", files: [] },
    { hash: "old", parents: ["base", "other"], message: "old", files: [] },
  ])
  await screen.findByLabelText("History operation")
  fireEvent.change(screen.getByRole("spinbutton"), { target: { value: "2" } })
  fireEvent.click(
    screen.getByRole("button", { name: "Apply reviewed operation" })
  )
  await waitFor(() => expect(api).toHaveBeenCalledTimes(2))
  expect(api).toHaveBeenLastCalledWith(
    "/repo",
    expect.objectContaining({
      operation: "cherry-pick",
      commits: ["old", "new"],
      mainline: 2,
      expectedHead: "head",
    })
  )
})
it("requires rewrite/signature and published confirmation before preserving merges", async () => {
  mount(
    [{ hash: "new", parents: ["p1", "p2"], message: "merge", files: [] }],
    true
  )
  fireEvent.change(await screen.findByLabelText("History operation"), {
    target: { value: "reword-merges" },
  })
  const apply = screen.getByRole("button", { name: "Apply reviewed operation" })
  expect(apply).toBeDisabled()
  fireEvent.click(screen.getAllByRole("checkbox")[0])
  expect(apply).toBeDisabled()
  fireEvent.click(screen.getAllByRole("checkbox")[1])
  fireEvent.change(screen.getByLabelText("Message for new"), {
    target: { value: "new merge message" },
  })
  fireEvent.click(apply)
  expect(await screen.findByRole("status")).toHaveTextContent("backup")
  expect(api).toHaveBeenLastCalledWith(
    "/repo",
    expect.objectContaining({
      operation: "reword-merges",
      messages: { new: "new merge message" },
      allowPublished: true,
    })
  )
})
it("splits only checked paths and keeps a refused draft", async () => {
  mount([
    { hash: "new", parents: ["p1"], message: "original", files: ["a", "b"] },
  ])
  fireEvent.change(await screen.findByLabelText("History operation"), {
    target: { value: "split" },
  })
  fireEvent.click(screen.getByLabelText("a"))
  fireEvent.change(screen.getByLabelText("Second commit message"), {
    target: { value: "remaining" },
  })
  fireEvent.click(screen.getAllByRole("checkbox")[2])
  api.mockRejectedValueOnce(new Error("HEAD changed"))
  fireEvent.click(
    screen.getByRole("button", { name: "Apply reviewed operation" })
  )
  expect(await screen.findByRole("alert")).toHaveTextContent("HEAD changed")
  expect(screen.getByLabelText("Second commit message")).toHaveValue(
    "remaining"
  )
  expect(api).toHaveBeenLastCalledWith(
    "/repo",
    expect.objectContaining({
      operation: "split",
      files: ["a"],
      secondMessage: "remaining",
    })
  )
})
