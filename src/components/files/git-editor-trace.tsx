"use client"
import { useCallback, useEffect, useState, useSyncExternalStore } from "react"
import type { editor as Editor } from "monaco-editor"
import type { Monaco } from "@monaco-editor/react"
import { useTranslations } from "next-intl"
import { gitBlame, gitLineHistory } from "@/lib/api"
import type { GitBlameLine, GitLineHistoryEntry } from "@/lib/types"
import { toErrorMessage } from "@/lib/app-error"
import { GitRevisionBrowser } from "@/components/layout/git-revision-browser"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

export function GitLineHistoryDialog({
  path,
  file,
  start,
  end,
  onClose,
}: {
  path: string
  file: string
  start: number
  end: number
  onClose: () => void
}) {
  const t = useTranslations("Folder.gitRevision")
  const [entries, setEntries] = useState<GitLineHistoryEntry[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [revision, setRevision] = useState<string | null>(null)
  const [more, setMore] = useState(false)
  useEffect(() => {
    let active = true
    gitLineHistory(path, file, start, end)
      .then((result) => {
        if (active) {
          setEntries(result)
          setMore(result.length === 100)
        }
      })
      .catch((err) => {
        if (active) setError(toErrorMessage(err))
      })
      .finally(() => {
        if (active) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [path, file, start, end])
  return (
    <>
      <Dialog
        open
        onOpenChange={(open) => {
          if (!open) onClose()
        }}
      >
        <DialogContent className="flex max-h-[85vh] flex-col sm:max-w-3xl">
          <DialogHeader>
            <DialogTitle>{t("lineHistory")}</DialogTitle>
            <DialogDescription>
              {file}:{start}–{end}
            </DialogDescription>
          </DialogHeader>
          {error && (
            <p
              role="alert"
              className="whitespace-pre-wrap text-sm text-destructive"
            >
              {error}
            </p>
          )}
          {loading && <p role="status">{t("loading")}</p>}
          <div className="min-h-0 overflow-auto">
            {entries.map((entry) => (
              <button
                key={entry.hash}
                className="block w-full border-b p-2 text-left text-sm hover:bg-muted"
                onClick={() => setRevision(entry.hash)}
              >
                <span className="block">{entry.message.split("\n")[0]}</span>
                <span className="font-mono text-xs text-muted-foreground">
                  {entry.hash.slice(0, 8)} · {entry.author} ·{" "}
                  {new Date(entry.date).toLocaleString()}
                </span>
              </button>
            ))}
          </div>
          {more && (
            <Button
              disabled={loading}
              onClick={() => {
                setLoading(true)
                void gitLineHistory(
                  path,
                  file,
                  start,
                  end,
                  undefined,
                  entries.length
                )
                  .then((result) => {
                    setEntries((previous) => [...previous, ...result])
                    setMore(result.length === 100)
                  })
                  .catch((err) => setError(toErrorMessage(err)))
                  .finally(() => setLoading(false))
              }}
            >
              {t("more")}
            </Button>
          )}
        </DialogContent>
      </Dialog>
      {revision && (
        <GitRevisionBrowser
          path={path}
          initialRevision={revision}
          onClose={() => setRevision(null)}
        />
      )}
    </>
  )
}

export function GitEditorTrace({
  editor,
  monaco,
  path,
  file,
  content,
  dirty = false,
}: {
  editor: Editor.IStandaloneCodeEditor | null
  monaco: Monaco | null
  path: string
  file: string
  content: string
  dirty?: boolean
}) {
  const t = useTranslations("Folder.gitRevision")
  const [enabled, setEnabled] = useState(false)
  const [lines, setLines] = useState<GitBlameLine[]>([])
  const subscribeCursor = useCallback(
    (notify: () => void) => {
      const listener = editor?.onDidChangeCursorPosition(notify)
      return () => listener?.dispose()
    },
    [editor]
  )
  const line = useSyncExternalStore(
    subscribeCursor,
    () => editor?.getPosition()?.lineNumber ?? 1,
    () => 1
  )
  const [range, setRange] = useState<{ start: number; end: number } | null>(
    null
  )
  const [revision, setRevision] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let active = true
    if (enabled && !dirty)
      gitBlame(path, file)
        .then((result) => {
          if (active) {
            setLines(result)
            setError(null)
          }
        })
        .catch((err) => {
          if (active) {
            setLines([])
            setError(toErrorMessage(err))
          }
        })
    return () => {
      active = false
    }
  }, [path, file, content, enabled, dirty])
  const blame = !dirty ? lines.find((entry) => entry.line === line) : undefined
  useEffect(() => {
    if (!editor || !enabled || !blame) return
    const model = editor.getModel()
    if (!model || blame.line > model.getLineCount()) return
    const label = blame.uncommitted
      ? t("uncommitted")
      : `${blame.author} · ${blame.commit.slice(0, 8)} · ${blame.summary}`
    const decorations = editor.createDecorationsCollection([
      {
        range: {
          startLineNumber: blame.line,
          startColumn: model.getLineMaxColumn(blame.line),
          endLineNumber: blame.line,
          endColumn: model.getLineMaxColumn(blame.line),
        },
        options: {
          after: {
            content: "    " + label,
            inlineClassName: "codeg-inline-blame",
            cursorStops: monaco?.editor.InjectedTextCursorStops.None,
          },
          showIfCollapsed: true,
        },
      },
    ])
    return () => decorations.clear()
  }, [editor, monaco, enabled, blame, t])
  useEffect(() => {
    if (!editor) return
    const open = () => {
      if (dirty) {
        setError(t("saveFirst"))
        return
      }
      const selection = editor.getSelection()
      if (!selection) return
      let end = selection.endLineNumber
      if (end > selection.startLineNumber && selection.endColumn === 1) end--
      setRange({ start: selection.startLineNumber, end })
    }
    const action = editor.addAction({
      id: "codeg.git.selectedLineHistory",
      label: t("lineHistory"),
      contextMenuGroupId: "navigation",
      contextMenuOrder: 2,
      run: open,
    })
    const toggle = editor.addAction({
      id: "codeg.git.inlineBlame",
      label: t("inlineBlame"),
      contextMenuGroupId: "navigation",
      contextMenuOrder: 3,
      run: () => setEnabled((value) => !value),
    })
    return () => {
      action.dispose()
      toggle.dispose()
    }
  }, [editor, dirty, t])
  return (
    <>
      <div className="flex shrink-0 flex-wrap items-center gap-1 border-b px-2 py-1 text-xs">
        <Button
          size="xs"
          variant={enabled ? "secondary" : "ghost"}
          onClick={() => setEnabled(!enabled)}
        >
          {t("inlineBlame")}
        </Button>
        <Button
          size="xs"
          variant="ghost"
          onClick={() => {
            if (dirty) {
              setError(t("saveFirst"))
              return
            }
            const selection = editor?.getSelection()
            if (selection) {
              let end = selection.endLineNumber
              if (end > selection.startLineNumber && selection.endColumn === 1)
                end--
              setRange({ start: selection.startLineNumber, end })
            }
          }}
        >
          {t("lineHistory")}
        </Button>
        {enabled && blame && !blame.uncommitted && (
          <button
            className="max-w-64 truncate text-primary hover:underline"
            onClick={() => setRevision(blame.commit)}
            title={blame.summary}
          >
            {blame.author} · {blame.commit.slice(0, 8)}
          </button>
        )}
        {enabled && dirty && <span>{t("saveFirst")}</span>}
        {error && (
          <span role="alert" className="text-destructive">
            {error}
          </span>
        )}
      </div>
      {range && (
        <GitLineHistoryDialog
          key={`${path}:${file}:${range.start}:${range.end}`}
          path={path}
          file={file}
          start={range.start}
          end={range.end}
          onClose={() => setRange(null)}
        />
      )}{" "}
      {revision && (
        <GitRevisionBrowser
          path={path}
          initialRevision={revision}
          onClose={() => setRevision(null)}
        />
      )}
    </>
  )
}
