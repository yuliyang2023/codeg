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
import { SourceEditorDialog } from "./source-editor-dialog"
const api = vi.hoisted(() => ({ read: vi.fn(), save: vi.fn() }))
vi.mock("@/lib/api", () => ({
  readFileForEdit: api.read,
  saveFileContent: api.save,
}))
vi.mock("@/lib/monaco-local", () => ({}))
vi.mock("@/lib/monaco-themes", () => ({
  useMonacoWorkspaceTheme: () => "vs-dark",
  MONACO_UNICODE_HIGHLIGHT_OPTIONS: {},
}))
vi.mock("./git-editor-trace", () => ({ GitEditorTrace: () => null }))
vi.mock("next/dynamic", () => ({
  default:
    () =>
    ({
      value,
      onChange,
    }: {
      value: string
      onChange: (text: string) => void
    }) => (
      <textarea
        aria-label="Source contents"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    ),
}))
afterEach(cleanup)
beforeEach(() => {
  api.read.mockReset()
  api.save.mockReset()
  api.read.mockResolvedValue({
    content: "old",
    etag: "etag-1",
    readonly: false,
  })
})
it("sends the read etag, preserves edits after a concurrent-save error and guards closing", async () => {
  const close = vi.fn()
  render(
    <NextIntlClientProvider locale="en" messages={messages}>
      <SourceEditorDialog path="/repo" file="a" line={2} onClose={close} />
    </NextIntlClientProvider>
  )
  fireEvent.change(await screen.findByLabelText("Source contents"), {
    target: { value: "my changes" },
  })
  api.save.mockRejectedValue(new Error("File changed on disk"))
  fireEvent.click(screen.getByRole("button", { name: "Save" }))
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "File changed on disk"
  )
  expect(api.save).toHaveBeenCalledWith("/repo", "a", "my changes", "etag-1")
  expect(screen.getByLabelText("Source contents")).toHaveValue("my changes")
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" })
  await waitFor(() =>
    expect(screen.getByText("Discard unsaved changes?")).toBeInTheDocument()
  )
  expect(close).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole("button", { name: "Discard" }))
  expect(close).toHaveBeenCalled()
})
