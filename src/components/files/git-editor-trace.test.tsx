import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import type { Monaco } from "@monaco-editor/react"
import type { editor as Editor } from "monaco-editor"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import messages from "@/i18n/messages/en.json"
import { GitEditorTrace } from "./git-editor-trace"
const api = vi.hoisted(() => ({ blame: vi.fn(), history: vi.fn() }))
vi.mock("@/lib/api", () => ({
  gitBlame: api.blame,
  gitLineHistory: api.history,
}))
vi.mock("@/components/layout/git-revision-browser", () => ({
  GitRevisionBrowser: () => <div>revision browser</div>,
}))
afterEach(cleanup)
beforeEach(() => {
  api.blame.mockReset()
  api.history.mockReset()
  api.blame.mockResolvedValue([
    {
      line: 2,
      commit: "abcdef123",
      author: "Author",
      summary: "Changed line",
      uncommitted: false,
    },
  ])
  api.history.mockResolvedValue([])
})
function setup() {
  const clear = vi.fn()
  const decorate = vi.fn(() => ({ clear }))
  let cursor: (event: { position: { lineNumber: number } }) => void = () => {}
  const actions = new Map<string, { run: () => void }>()
  const editor = {
    getPosition: () => ({ lineNumber: 2 }),
    getSelection: () => ({
      startLineNumber: 2,
      endLineNumber: 4,
      endColumn: 1,
    }),
    getModel: () => ({ getLineCount: () => 5, getLineMaxColumn: () => 10 }),
    onDidChangeCursorPosition: (fn: typeof cursor) => {
      cursor = fn
      return { dispose: vi.fn() }
    },
    createDecorationsCollection: decorate,
    addAction: (action: { id: string; run: () => void }) => {
      actions.set(action.id, action)
      return { dispose: vi.fn() }
    },
  } as unknown as Editor.IStandaloneCodeEditor
  const monaco = {
    editor: { InjectedTextCursorStops: { None: 0 } },
  } as unknown as Monaco
  const component = (dirty = false) => (
    <NextIntlClientProvider locale="en" messages={messages}>
      <GitEditorTrace
        editor={editor}
        monaco={monaco}
        path="/repo"
        file="a"
        content="one\ntwo"
        dirty={dirty}
      />
    </NextIntlClientProvider>
  )
  return {
    component,
    clear,
    decorate,
    actions,
    move: (line: number) => cursor({ position: { lineNumber: line } }),
  }
}
it("shows current-line blame, clears the decoration when dirty or disabled, and navigates its commit", async () => {
  const scene = setup()
  const view = render(scene.component())
  fireEvent.click(screen.getByRole("button", { name: "Inline blame" }))
  await waitFor(() => expect(scene.decorate).toHaveBeenCalled())
  expect(JSON.stringify(scene.decorate.mock.calls)).toContain(
    "Author · abcdef12 · Changed line"
  )
  fireEvent.click(screen.getByRole("button", { name: "Author · abcdef12" }))
  expect(screen.getByText("revision browser")).toBeInTheDocument()
  view.rerender(scene.component(true))
  expect(scene.clear).toHaveBeenCalled()
  expect(
    screen.queryByRole("button", { name: "Author · abcdef12" })
  ).not.toBeInTheDocument()
})
it("uses selected whole lines, excludes an end-column-one line and blocks unsaved ranges", async () => {
  const scene = setup()
  const view = render(scene.component())
  act(() => scene.actions.get("codeg.git.selectedLineHistory")!.run())
  await waitFor(() =>
    expect(api.history).toHaveBeenCalledWith("/repo", "a", 2, 3)
  )
  view.unmount()
  api.history.mockClear()
  render(scene.component(true))
  fireEvent.click(screen.getByRole("button", { name: "Selected line history" }))
  expect(screen.getByRole("alert")).toHaveTextContent("Save changes")
  expect(api.history).not.toHaveBeenCalled()
})
