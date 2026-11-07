import { act, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import { beforeEach, describe, expect, it, vi } from "vitest"
import enMessages from "@/i18n/messages/en.json"

// Shared recorder for the fake Monaco pair below. Declared through
// `vi.hoisted` so the module factory (which runs before this file's body) can
// reach it.
const harness = vi.hoisted(() => {
  const events: string[] = []
  // The model pair the fake widget currently holds — the state Monaco's
  // DiffEditorWidget consults through its `onWillDispose` listeners.
  const state: { attached: Record<string, object> | null } = { attached: null }
  const actions = new Map<
    string,
    { label: string; keybindings: number[]; run: () => Promise<void> }
  >()
  const positions = { original: 1, modified: 1 }
  const changes: {
    originalStartLineNumber: number
    originalEndLineNumber: number
    modifiedStartLineNumber: number
    modifiedEndLineNumber: number
  }[] = []
  return {
    events,
    state,
    actions,
    positions,
    changes,
    monaco: { KeyCode: { F4: 62 } },
  }
})

// Stand-in for `@monaco-editor/react`'s DiffEditor that reproduces the two
// behaviours the component's teardown exists for: the library disposes both
// text models BEFORE the widget, and Monaco's DiffEditorWidget reports a model
// disposed while still attached as a bug ("TextModel got disposed before
// DiffEditorWidget model got reset", rethrown by its default error handler).
vi.mock("@monaco-editor/react", async () => {
  const { createElement, useEffect } = await import("react")

  const makeDiffEditor = () => {
    let model: Record<string, { dispose: () => void }> | null = null
    const detach = () => {
      model = null
      harness.state.attached = null
    }
    // Monaco's widget listens for `onWillDispose` on the pair it holds: it
    // reports the bug and then resets itself, which is why a real teardown
    // surfaces one error rather than two.
    const makeModel = (side: "original" | "modified") => {
      const m = {
        dispose: () => {
          if (harness.state.attached?.[side] === m) {
            harness.events.push(`bug:${side}`)
            detach()
          }
          harness.events.push(`${side}:dispose`)
        },
      }
      return m
    }
    model = { original: makeModel("original"), modified: makeModel("modified") }
    harness.state.attached = model
    const makeInnerEditor = (side: "original" | "modified") => ({
      revealLineInCenter: () => {},
      setPosition: () => {},
      getPosition: () => ({ lineNumber: harness.positions[side], column: 1 }),
      getModel: () => ({ getLineCount: () => 100 }),
      addAction: (action: {
        label: string
        keybindings: number[]
        run: () => Promise<void>
      }) => {
        harness.actions.set(side, action)
        return {
          dispose: () => {
            if (harness.actions.get(side) === action)
              harness.actions.delete(side)
          },
        }
      },
    })
    const originalEditor = makeInnerEditor("original")
    const modifiedEditor = makeInnerEditor("modified")
    return {
      getModel: () => model,
      setModel: (next: typeof model) => {
        model = next
        harness.state.attached = next
      },
      getOriginalEditor: () => originalEditor,
      getModifiedEditor: () => modifiedEditor,
      getLineChanges: () => harness.changes,
      onDidUpdateDiff: () => ({ dispose: () => {} }),
      dispose: () => harness.events.push("editor:dispose"),
    }
  }

  const DiffEditor = ({
    onMount,
    options,
  }: {
    onMount?: (editor: ReturnType<typeof makeDiffEditor>) => void
    options?: {
      renderSideBySide?: boolean
      hideUnchangedRegions?: { enabled?: boolean }
    }
  }) => {
    useEffect(() => {
      const editor = makeDiffEditor()
      onMount?.(editor)
      return () => {
        // @monaco-editor/react@4.7.0's unmount path, verbatim in spirit:
        // models first, widget last.
        const current = editor.getModel()
        current?.original?.dispose()
        current?.modified?.dispose()
        editor.dispose()
      }
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [])
    return createElement("div", {
      "data-testid": "mock-diff-editor",
      "data-side-by-side": String(options?.renderSideBySide),
      "data-collapse": String(options?.hideUnchangedRegions?.enabled),
    })
  }

  return {
    DiffEditor,
    useMonaco: () => harness.monaco,
    loader: { config: () => {} },
  }
})

vi.mock("@/lib/monaco-themes", () => ({
  defineMonacoThemes: () => {},
  MONACO_UNICODE_HIGHLIGHT_OPTIONS: {},
  useMonacoWorkspaceTheme: () => "vs-dark",
}))

vi.mock("@/hooks/use-appearance", () => ({
  useZoomLevel: () => ({ zoomLevel: 100 }),
  useEditorFont: () => ({
    editorFontStack: "monospace",
    editorFontSize: 12,
    editorLigatures: false,
    editorWordWrap: false,
  }),
}))

const { DiffViewer } = await import("./diff-viewer")
const { DiffEditor: MockDiffEditor } =
  (await import("@monaco-editor/react")) as unknown as {
    DiffEditor: () => React.ReactElement
  }

function Wrapper({ children }: { children: React.ReactNode }) {
  return (
    <NextIntlClientProvider locale="en" messages={enMessages}>
      {children}
    </NextIntlClientProvider>
  )
}

beforeEach(() => {
  localStorage.clear()
  harness.actions.clear()
  harness.changes.length = 0
  harness.positions.original = 1
  harness.positions.modified = 1
})

describe("DiffViewer Go to Source", () => {
  it("registers both context menus, maps old-side lines and cleans up", async () => {
    const openSource = vi.fn()
    harness.positions.original = 7
    harness.positions.modified = 20
    harness.changes.push({
      originalStartLineNumber: 2,
      originalEndLineNumber: 0,
      modifiedStartLineNumber: 3,
      modifiedEndLineNumber: 5,
    })
    const view = render(
      <DiffViewer original="a" modified="b" onOpenSource={openSource} />,
      { wrapper: Wrapper }
    )
    await waitFor(() => expect(harness.actions.size).toBe(2))
    expect(harness.actions.get("original")?.label).toBe("Go to Source")
    expect(harness.actions.get("original")?.keybindings).toEqual([62])
    expect(harness.actions.get("modified")?.keybindings).toEqual([62])
    await act(() => harness.actions.get("original")!.run())
    expect(openSource).toHaveBeenLastCalledWith(10)
    await act(() => harness.actions.get("modified")!.run())
    expect(openSource).toHaveBeenLastCalledWith(20)
    const nextSource = vi.fn()
    view.rerender(
      <DiffViewer original="a" modified="b" onOpenSource={nextSource} />
    )
    await act(() => harness.actions.get("modified")!.run())
    expect(nextSource).toHaveBeenCalledWith(20)
    expect(openSource).toHaveBeenCalledTimes(2)
    view.unmount()
    expect(harness.actions.size).toBe(0)
  })
})

describe("DiffViewer layout", () => {
  const labels = enMessages.Folder.diffPreview.viewMode

  it("switches to unified without remounting the editor and remembers the choice", async () => {
    harness.events.length = 0
    const view = render(<DiffViewer original="before" modified="after" />, {
      wrapper: Wrapper,
    })
    const editor = await screen.findByTestId("mock-diff-editor")
    expect(editor).toHaveAttribute("data-side-by-side", "true")
    fireEvent.click(screen.getByRole("button", { name: labels.unified }))
    expect(editor).toHaveAttribute("data-side-by-side", "false")
    expect(localStorage.getItem("workspace:diff-view-mode")).toBe("unified")
    expect(harness.events).toEqual([])
    view.unmount()
    render(<DiffViewer original="other" modified="file" />, {
      wrapper: Wrapper,
    })
    expect(await screen.findByTestId("mock-diff-editor")).toHaveAttribute(
      "data-side-by-side",
      "false"
    )
    fireEvent.click(screen.getByRole("button", { name: labels.split }))
    expect(screen.getByTestId("mock-diff-editor")).toHaveAttribute(
      "data-side-by-side",
      "true"
    )
  })

  it("synchronizes multiple open viewers and changes from another browser tab", async () => {
    render(
      <>
        <DiffViewer original="a" modified="b" />
        <DiffViewer original="c" modified="d" />
      </>,
      { wrapper: Wrapper }
    )
    await waitFor(() =>
      expect(screen.getAllByTestId("mock-diff-editor")).toHaveLength(2)
    )
    fireEvent.click(screen.getAllByRole("button", { name: labels.unified })[0])
    expect(
      screen
        .getAllByTestId("mock-diff-editor")
        .every((e) => e.dataset.sideBySide === "false")
    ).toBe(true)
    act(() => {
      localStorage.setItem("workspace:diff-view-mode", "split")
      window.dispatchEvent(
        new StorageEvent("storage", { key: "workspace:diff-view-mode" })
      )
    })
    expect(
      screen
        .getAllByTestId("mock-diff-editor")
        .every((e) => e.dataset.sideBySide === "true")
    ).toBe(true)
  })
})

describe("DiffViewer teardown", () => {
  beforeEach(() => {
    harness.events.length = 0
    harness.state.attached = null
  })

  it("disposes the diff models only after detaching them from the widget", async () => {
    const { unmount } = render(<DiffViewer original="a" modified="b" />, {
      wrapper: Wrapper,
    })
    expect(await screen.findByTestId("mock-diff-editor")).toBeInTheDocument()

    unmount()

    // No "TextModel got disposed before DiffEditorWidget model got reset":
    // the widget no longer holds either model by the time it is disposed.
    expect(harness.events.filter((e) => e.startsWith("bug:"))).toEqual([])
    // Both models still get disposed exactly once — detaching must not leak
    // them (they are anonymous, so nothing else could ever reclaim them).
    expect(harness.events).toEqual([
      "original:dispose",
      "modified:dispose",
      "editor:dispose",
    ])
  })

  it("the fake reproduces the library bug when the component does not intervene", async () => {
    // Guards the test above from passing vacuously: rendered bare, the same
    // fake records exactly the disposal-while-attached that Monaco flags.
    const { unmount } = render(<MockDiffEditor />)
    expect(await screen.findByTestId("mock-diff-editor")).toBeInTheDocument()

    unmount()

    expect(harness.events).toEqual([
      "bug:original",
      "original:dispose",
      "modified:dispose",
      "editor:dispose",
    ])
  })
})

it("synchronizes unchanged-region folding without remounting and keeps it in Unified mode", async () => {
  const view = render(
    <>
      <DiffViewer original="a" modified="b" />
      <DiffViewer original="c" modified="d" />
    </>,
    { wrapper: Wrapper }
  )
  await screen.findAllByTestId("mock-diff-editor")
  fireEvent.click(
    screen.getAllByRole("button", { name: "Collapse unchanged regions" })[0]
  )
  expect(
    screen
      .getAllByTestId("mock-diff-editor")
      .every((e) => e.dataset.collapse === "true")
  ).toBe(true)
  fireEvent.click(
    screen.getAllByRole("button", {
      name: enMessages.Folder.diffPreview.viewMode.unified,
    })[0]
  )
  expect(
    screen
      .getAllByTestId("mock-diff-editor")
      .every(
        (e) => e.dataset.collapse === "true" && e.dataset.sideBySide === "false"
      )
  ).toBe(true)
  expect(localStorage.getItem("workspace:diff-collapse-unchanged")).toBe("true")
  view.unmount()
  render(<DiffViewer original="e" modified="f" />, { wrapper: Wrapper })
  expect(await screen.findByTestId("mock-diff-editor")).toHaveAttribute(
    "data-collapse",
    "true"
  )
  act(() => {
    localStorage.setItem("workspace:diff-collapse-unchanged", "false")
    window.dispatchEvent(
      new StorageEvent("storage", { key: "workspace:diff-collapse-unchanged" })
    )
  })
  expect(screen.getByTestId("mock-diff-editor")).toHaveAttribute(
    "data-collapse",
    "false"
  )
})
