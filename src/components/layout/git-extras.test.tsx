import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import { useState } from "react"
import messages from "@/i18n/messages/en.json"
import { GitCommitOptionsPanel } from "./git-commit-options"
import { GitShelfManage } from "./git-shelf-manage"
import type { GitCommitOptions } from "@/lib/api"
const api = vi.hoisted(() => vi.fn())
vi.mock("@/lib/api", () => ({ gitCommitTools: api }))
afterEach(cleanup)
beforeEach(() => {
  api.mockReset()
})
function Wrapper({ children }: { children: React.ReactNode }) {
  return (
    <NextIntlClientProvider locale="en" messages={messages}>
      {children}
    </NextIntlClientProvider>
  )
}
it("loads successful messages and keeps author/signing/hook choices explicit", async () => {
  api.mockResolvedValue({ messages: ["Previous message"], shelves: {} })
  const message = vi.fn()
  function Scene() {
    const [value, setValue] = useState<GitCommitOptions>({})
    return (
      <>
        <GitCommitOptionsPanel
          path="/repo"
          value={value}
          onChange={setValue}
          onMessage={message}
        />
        <output>{JSON.stringify(value)}</output>
      </>
    )
  }
  render(<Scene />, { wrapper: Wrapper })
  fireEvent.click(screen.getByText("Commit options and message history"))
  await screen.findByRole("option", { name: "Previous message" })
  fireEvent.change(screen.getByLabelText("Previous successful messages"), {
    target: { value: "Previous message" },
  })
  expect(message).toHaveBeenCalledWith("Previous message")
  fireEvent.change(screen.getByLabelText("Commit signing"), {
    target: { value: "signed" },
  })
  fireEvent.change(screen.getByLabelText("Signing key (optional)"), {
    target: { value: "key-1" },
  })
  fireEvent.click(screen.getByLabelText("Skip Git hooks for this commit"))
  expect(screen.getByRole("status")).toHaveTextContent('"signingKey":"key-1"')
})
it("reviews selected shelf files and preserves the dialog on restore failure", async () => {
  api.mockImplementation(async (_p, op) => {
    if (op === "shelf-paths") return ["a", "b"]
    throw new Error("Patch overlaps local changes")
  })
  const close = vi.fn(),
    changed = vi.fn()
  render(
    <GitShelfManage
      path="/repo"
      reference="ref"
      name="Shelf"
      group=""
      onClose={close}
      onChanged={changed}
    />,
    { wrapper: Wrapper }
  )
  fireEvent.click(await screen.findByLabelText("a"))
  fireEvent.click(
    screen.getByRole("button", { name: "Restore selected files…" })
  )
  expect(api).toHaveBeenCalledTimes(1)
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm selected-file restore" })
  )
  expect(await screen.findByRole("alert")).toHaveTextContent("Patch overlaps")
  expect(api).toHaveBeenLastCalledWith("/repo", "shelf-restore", {
    shelfRef: "ref",
    files: ["a"],
  })
  expect(close).not.toHaveBeenCalled()
  expect(changed).not.toHaveBeenCalled()
})
it("saves name and group without applying any shelf files", async () => {
  api.mockResolvedValue([])
  const close = vi.fn()
  render(
    <GitShelfManage
      path="/repo"
      reference="ref"
      name="Old"
      group=""
      onClose={close}
      onChanged={vi.fn()}
    />,
    { wrapper: Wrapper }
  )
  fireEvent.change(screen.getByLabelText("Shelf name"), {
    target: { value: "New" },
  })
  fireEvent.change(screen.getByLabelText("Shelf group"), {
    target: { value: "Feature" },
  })
  fireEvent.click(screen.getByRole("button", { name: "Save name and group" }))
  await waitFor(() => expect(close).toHaveBeenCalled())
  expect(api).toHaveBeenLastCalledWith("/repo", "shelf-meta", {
    shelfRef: "ref",
    name: "New",
    group: "Feature",
  })
})
