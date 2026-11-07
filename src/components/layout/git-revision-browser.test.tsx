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
import { GitRevisionBrowser } from "./git-revision-browser"
const api = vi.hoisted(() => ({
  info: vi.fn(),
  tree: vi.fn(),
  file: vi.fn(),
  compare: vi.fn(),
  diff: vi.fn(),
  checkout: vi.fn(),
}))
vi.mock("@/lib/api", () => ({
  gitRevisionInfo: api.info,
  gitRevisionTree: api.tree,
  gitRevisionFile: api.file,
  gitRevisionCompare: api.compare,
  gitCompareFile: api.diff,
  gitRevisionCheckout: api.checkout,
}))
vi.mock("next/dynamic", () => ({
  default: () => () => <div>historical editor</div>,
}))
vi.mock("@/lib/monaco-local", () => ({}))
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
vi.mock("@/lib/monaco-themes", () => ({
  useMonacoWorkspaceTheme: () => "vs-dark",
  MONACO_UNICODE_HIGHLIGHT_OPTIONS: {},
}))
afterEach(cleanup)
beforeEach(() => {
  Object.values(api).forEach((fn) => fn.mockReset())
  api.info.mockImplementation(async (_path, revision) => ({
    hash: revision === "HEAD" ? "head" : revision,
    message: "commit " + revision,
    author: "A",
    date: "2026-11-07",
    parents: revision === "parent" ? [] : ["parent"],
    children: revision === "parent" ? ["head"] : [],
  }))
  api.tree.mockResolvedValue([{ name: "a.txt", file: "a.txt", kind: "blob" }])
  api.compare.mockResolvedValue({
    base: "parent",
    target: "head",
    files: [{ file: "a.txt", status: "M" }],
  })
  api.diff.mockResolvedValue({
    original: "before",
    modified: "after",
    original_label: "parent",
    modified_label: "head",
  })
})
function mount() {
  const changed = vi.fn()
  render(
    <NextIntlClientProvider locale="en" messages={messages}>
      <GitRevisionBrowser path="/repo" onClose={vi.fn()} onChanged={changed} />
    </NextIntlClientProvider>
  )
  return changed
}
it("navigates parents/children and compares immutable revision hashes", async () => {
  mount()
  fireEvent.click(await screen.findByRole("button", { name: "parent" }))
  await waitFor(() =>
    expect(api.tree).toHaveBeenCalledWith("/repo", "parent", "")
  )
  fireEvent.click(await screen.findByRole("button", { name: "head" }))
  await waitFor(() =>
    expect(api.tree).toHaveBeenLastCalledWith("/repo", "head", "")
  )
  fireEvent.change(screen.getByLabelText("Compare from revision"), {
    target: { value: "parent" },
  })
  fireEvent.click(screen.getByRole("button", { name: "Compare" }))
  fireEvent.click(await screen.findByRole("button", { name: "M a.txt" }))
  expect(await screen.findByText("before → after")).toBeInTheDocument()
  expect(api.diff).toHaveBeenCalledWith("/repo", "a.txt", "parent", "head")
})
it("requires explicit checkout review, retains an error and sends the captured HEAD", async () => {
  const changed = mount()
  await screen.findByRole("button", { name: "a.txt" })
  fireEvent.click(screen.getByRole("button", { name: "Check out commit" }))
  await screen.findByText(/detached HEAD/)
  expect(api.checkout).not.toHaveBeenCalled()
  api.checkout.mockRejectedValueOnce(new Error("working tree changed"))
  const buttons = screen.getAllByRole("button", { name: "Check out commit" })
  fireEvent.click(buttons[buttons.length - 1])
  expect((await screen.findAllByRole("alert"))[0]).toHaveTextContent(
    "working tree changed"
  )
  expect(api.checkout).toHaveBeenCalledWith("/repo", "head", "head")
  expect(changed).not.toHaveBeenCalled()
})
