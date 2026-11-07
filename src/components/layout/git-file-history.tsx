"use client"

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react"
import { useTranslations } from "next-intl"
import { Virtualizer } from "virtua"
import { GitFileCompareDialog } from "./git-file-compare-dialog"
import { LocalHistoryTab } from "./local-history-tab"
import { gitBlame, gitFileHistory } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import type { GitBlameLine, GitLogEntry } from "@/lib/types"
import { useActiveFolder } from "@/contexts/active-folder-context"
import { useWorkspaceActions } from "@/contexts/workspace-context"
import { Button } from "@/components/ui/button"
import { ContextMenuItem } from "@/components/ui/context-menu"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

interface Target {
  folderId: number
  path: string
  file: string
  revision?: string
  view: "history" | "blame" | "compare" | "local"
}
const HistoryContext = createContext<
  ((file: string, view: Target["view"], revision?: string) => void) | null
>(null)

export function GitFileHistoryProvider({ children }: { children: ReactNode }) {
  const { activeFolder } = useActiveFolder()
  const tools = useTranslations("Folder.gitTools")
  const [target, setTarget] = useState<Target | null>(null)
  const open = useCallback(
    (file: string, view: Target["view"], revision?: string) => {
      if (activeFolder)
        setTarget({
          folderId: activeFolder.id,
          path: activeFolder.path,
          file,
          view,
          revision,
        })
    },
    [activeFolder]
  )
  const currentScope = `${activeFolder?.id}:${activeFolder?.path}`
  const [scope, setScope] = useState(currentScope)
  if (scope !== currentScope) {
    setScope(currentScope)
    setTarget(null)
  }
  return (
    <HistoryContext.Provider value={open}>
      {children}
      {target &&
        target.folderId === activeFolder?.id &&
        target.path === activeFolder?.path &&
        (target.view === "compare" ? (
          <GitFileCompareDialog
            key={`${target.path}:${target.file}`}
            path={target.path}
            file={target.file}
            revision={target.revision}
            onClose={() => setTarget(null)}
          />
        ) : target.view === "local" ? (
          <Dialog
            open
            onOpenChange={(open) => {
              if (!open) setTarget(null)
            }}
          >
            <DialogContent className="flex h-[85vh] flex-col sm:max-w-6xl">
              <DialogHeader>
                <DialogTitle>{tools("localHistory")}</DialogTitle>
                <DialogDescription>{target.file}</DialogDescription>
              </DialogHeader>
              <div className="min-h-0 flex-1">
                <LocalHistoryTab
                  folder={{ path: target.path }}
                  file={target.file}
                />
              </div>
            </DialogContent>
          </Dialog>
        ) : (
          <GitFileHistoryDialog
            key={`${target.path}:${target.file}:${target.revision}`}
            target={{ ...target, view: target.view }}
            onClose={() => setTarget(null)}
          />
        ))}
    </HistoryContext.Provider>
  )
}

export function GitFileHistoryMenu({
  file,
  revision,
  disabled = false,
}: {
  file: string
  revision?: string
  disabled?: boolean
}) {
  const open = useContext(HistoryContext)
  const t = useTranslations("Folder.gitHistory")
  const tools = useTranslations("Folder.gitTools")
  if (!open) return null
  return (
    <>
      <ContextMenuItem
        disabled={disabled}
        onSelect={() => open(file, "compare", revision)}
      >
        {tools("compareFile")}
      </ContextMenuItem>
      <ContextMenuItem disabled={disabled} onSelect={() => open(file, "local")}>
        {tools("localHistory")}
      </ContextMenuItem>
      <ContextMenuItem
        disabled={disabled}
        onSelect={() => open(file, "history", revision)}
      >
        {t("fileHistory")}
      </ContextMenuItem>
      <ContextMenuItem
        disabled={disabled}
        onSelect={() => open(file, "blame", revision)}
      >
        {t("blame")}
      </ContextMenuItem>
    </>
  )
}

export function GitFileHistoryDialog({
  target,
  onClose,
}: {
  target: Omit<Target, "view"> & { view: "history" | "blame" }
  onClose: () => void
}) {
  const t = useTranslations("Folder.gitHistory")
  const { openCommitDiff, openFilePreview } = useWorkspaceActions()
  const [view, setView] = useState(target.view)
  const [entries, setEntries] = useState<GitLogEntry[]>([])
  const [lines, setLines] = useState<GitBlameLine[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [more, setMore] = useState(false)
  const generation = useRef(0)
  const viewportRef = useRef<HTMLElement | null>(null)
  const [viewport, setViewport] = useState<HTMLElement | null>(null)
  const captureViewport = useCallback((node: HTMLDivElement | null) => {
    viewportRef.current = node
    setViewport(node)
  }, [])
  useEffect(() => {
    let active = true
    const current = ++generation.current
    setLoading(true)
    setError(null)
    setEntries([])
    setLines([])
    const request =
      view === "history"
        ? gitFileHistory(target.path, target.file, 0, target.revision)
        : gitBlame(target.path, target.file, target.revision)
    request
      .then((result) => {
        if (!active || current !== generation.current) return
        if (Array.isArray(result)) setLines(result)
        else {
          setEntries(result.entries)
          setMore(result.entries.length === 100)
        }
      })
      .catch((err) => {
        if (active && current === generation.current)
          setError(toErrorMessage(err))
      })
      .finally(() => {
        if (active && current === generation.current) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [target, view])
  async function loadMore() {
    const current = generation.current
    setLoading(true)
    setError(null)
    try {
      const result = await gitFileHistory(
        target.path,
        target.file,
        entries.length,
        target.revision
      )
      if (current !== generation.current) return
      setEntries((previous) => [...previous, ...result.entries])
      setMore(result.entries.length === 100)
    } catch (err) {
      if (current === generation.current) setError(toErrorMessage(err))
    } finally {
      if (current === generation.current) setLoading(false)
    }
  }
  async function showCommit(commit: string, file?: string, message?: string) {
    try {
      await openCommitDiff(commit, file, message, { folderId: target.folderId })
      onClose()
    } catch (err) {
      setError(toErrorMessage(err))
    }
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent className="flex h-[80vh] max-h-[900px] flex-col sm:max-w-5xl">
        <DialogHeader>
          <DialogTitle>
            {view === "history" ? t("fileHistory") : t("blame")}
          </DialogTitle>
          <DialogDescription className="break-all">
            {target.file} · {target.revision?.slice(0, 12) ?? t("workingTree")}
          </DialogDescription>
        </DialogHeader>
        <div className="flex gap-2">
          <Button
            size="xs"
            variant={view === "history" ? "secondary" : "ghost"}
            onClick={() => setView("history")}
          >
            {t("fileHistory")}
          </Button>
          <Button
            size="xs"
            variant={view === "blame" ? "secondary" : "ghost"}
            onClick={() => setView("blame")}
          >
            {t("blame")}
          </Button>
        </div>
        {error && (
          <p
            role="alert"
            className="text-xs text-destructive whitespace-pre-wrap"
          >
            {error}
          </p>
        )}
        {loading && (
          <p role="status" className="text-xs text-muted-foreground">
            {t("loading")}
          </p>
        )}
        {!loading &&
          !error &&
          (view === "history" ? entries.length : lines.length) === 0 && (
            <p className="text-sm text-muted-foreground">{t("empty")}</p>
          )}
        <div
          ref={captureViewport}
          className="min-h-0 flex-1 overflow-auto rounded border"
        >
          {viewport && view === "history" && (
            <Virtualizer scrollRef={viewportRef} data={entries} itemSize={60}>
              {(entry) => (
                <button
                  key={entry.full_hash}
                  className="flex w-full flex-col gap-1 border-b px-3 py-2 text-left hover:bg-muted"
                  onClick={() =>
                    void showCommit(entry.full_hash, undefined, entry.message)
                  }
                >
                  <span className="truncate text-sm">
                    {entry.message.split("\n")[0]}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {entry.hash} · {entry.author} ·{" "}
                    {new Date(entry.date).toLocaleString()}
                  </span>
                </button>
              )}
            </Virtualizer>
          )}
          {viewport && view === "blame" && (
            <Virtualizer scrollRef={viewportRef} data={lines} itemSize={26}>
              {(line) => (
                <div
                  key={line.line}
                  className="flex min-w-max items-center gap-2 border-b px-2 py-1 font-mono text-xs hover:bg-muted/50"
                >
                  <button
                    className="w-20 shrink-0 text-left text-primary hover:underline disabled:text-muted-foreground"
                    title={line.summary}
                    disabled={line.uncommitted}
                    onClick={() =>
                      void showCommit(line.commit, line.file, line.summary)
                    }
                  >
                    {line.uncommitted
                      ? t("uncommitted")
                      : line.commit.slice(0, 8)}
                  </button>
                  <span
                    className="w-28 shrink-0 truncate text-muted-foreground"
                    title={line.author}
                  >
                    {line.author}
                  </span>
                  <button
                    className="w-10 shrink-0 text-right text-muted-foreground hover:underline"
                    title={t("goToSource")}
                    disabled={!!target.revision}
                    onClick={() =>
                      void openFilePreview(target.file, {
                        line: line.line,
                        folderId: target.folderId,
                      })
                        .then(onClose)
                        .catch((err) => setError(toErrorMessage(err)))
                    }
                  >
                    {line.line}
                  </button>
                  <span className="whitespace-pre">{line.content}</span>
                </div>
              )}
            </Virtualizer>
          )}
        </div>
        {view === "history" && more && (
          <Button size="xs" disabled={loading} onClick={() => void loadMore()}>
            {t("loadMore")}
          </Button>
        )}
      </DialogContent>
    </Dialog>
  )
}
