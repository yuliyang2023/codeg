"use client"
import { useCallback, useEffect, useRef, useState } from "react"
import { useTranslations } from "next-intl"
import {
  localHistoryCapture,
  localHistoryList,
  localHistoryContent,
  localHistoryRestore,
} from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import type {
  FolderDetail,
  LocalHistoryEntry,
  LocalHistoryContent,
} from "@/lib/types"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { DiffViewer } from "@/components/diff/diff-viewer"
import { languageFromPath } from "@/lib/language-detect"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
export function LocalHistoryTab({
  folder,
  file,
}: {
  folder: Pick<FolderDetail, "path">
  file?: string
}) {
  const t = useTranslations("Folder.gitTools")
  const [entries, setEntries] = useState<LocalHistoryEntry[]>([])
  const [content, setContent] = useState<LocalHistoryContent | null>(null)
  const [label, setLabel] = useState("")
  const [filter, setFilter] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [confirm, setConfirm] = useState(false)
  const request = useRef({ id: 0 })
  const refresh = useCallback(async () => {
    setError(null)
    try {
      setEntries(await localHistoryList(folder.path, file))
    } catch (err) {
      setError(toErrorMessage(err))
    }
  }, [folder.path, file])
  useEffect(() => {
    let active = true
    const guard = request.current
    localHistoryList(folder.path, file)
      .then((items) => {
        if (active) setEntries(items)
      })
      .catch((err) => {
        if (active) setError(toErrorMessage(err))
      })
    return () => {
      active = false
      guard.id++
    }
  }, [folder.path, file])
  async function select(id: string) {
    const current = ++request.current.id
    setError(null)
    setContent(null)
    try {
      const value = await localHistoryContent(folder.path, id)
      if (current === request.current.id) setContent(value)
    } catch (err) {
      if (current === request.current.id) setError(toErrorMessage(err))
    }
  }
  return (
    <div className="flex h-full min-h-0 flex-col gap-2 p-2">
      <p className="text-xs text-muted-foreground">{t("localHint")}</p>
      <div className="flex flex-wrap gap-1">
        <Input
          aria-label={t("snapshotLabel")}
          placeholder={t("snapshotLabel")}
          value={label}
          disabled={busy}
          onChange={(e) => setLabel(e.target.value)}
        />
        <Button
          size="xs"
          disabled={busy}
          onClick={() => {
            setBusy(true)
            setError(null)
            void localHistoryCapture(folder.path, file, label)
              .then(() => {
                setLabel("")
                return refresh()
              })
              .catch((err) => setError(toErrorMessage(err)))
              .finally(() => setBusy(false))
          }}
        >
          {t("snapshot")}
        </Button>
        <Button
          size="xs"
          variant="outline"
          disabled={busy}
          onClick={() => void refresh()}
        >
          {t("refresh")}
        </Button>
        <Input
          aria-label={t("filterHistory")}
          placeholder={t("filterHistory")}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
      <div className="min-h-20 max-h-48 shrink-0 overflow-auto rounded border">
        {entries
          .filter((entry) =>
            (entry.file + " " + (entry.label ?? ""))
              .toLowerCase()
              .includes(filter.toLowerCase())
          )
          .map((entry) => (
            <button
              key={entry.id}
              className="block w-full border-b px-2 py-1 text-left text-xs hover:bg-muted"
              onClick={() => void select(entry.id)}
            >
              <span className="block truncate">
                {entry.file} {entry.label ? "· " + entry.label : ""}
              </span>
              <span className="text-muted-foreground">
                {new Date(entry.time).toLocaleString()} · {entry.reason} ·{" "}
                {entry.blob ? entry.size + " B" : t("deleted")}
              </span>
            </button>
          ))}
        {!entries.length && (
          <p className="p-2 text-xs text-muted-foreground">
            {t("emptyHistory")}
          </p>
        )}
      </div>
      {content && (
        <>
          <div className="flex items-center justify-between gap-2">
            <span className="truncate text-xs">{content.entry.file}</span>
            <Button size="xs" disabled={busy} onClick={() => setConfirm(true)}>
              {t("restoreSnapshot")}
            </Button>
          </div>
          <div className="min-h-32 flex-1">
            {content.binary ? (
              <p className="text-xs">{t("binaryHistory")}</p>
            ) : (
              <DiffViewer
                original={content.original ?? ""}
                modified={content.current ?? ""}
                originalLabel={content.entry.label ?? content.entry.time}
                modifiedLabel="WORKTREE"
                language={languageFromPath(content.entry.file)}
              />
            )}
          </div>
        </>
      )}
      <Dialog open={confirm} onOpenChange={setConfirm}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("restoreSnapshot")}</DialogTitle>
            <DialogDescription>{t("restoreConfirm")}</DialogDescription>
          </DialogHeader>
          <Button
            disabled={busy}
            onClick={() => {
              if (!content) return
              setConfirm(false)
              setBusy(true)
              setError(null)
              void localHistoryRestore(
                folder.path,
                content.entry.id,
                content.current_hash
              )
                .then(async () => {
                  await refresh()
                  await select(content.entry.id)
                })
                .catch((err) => setError(toErrorMessage(err)))
                .finally(() => setBusy(false))
            }}
          >
            {t("restoreSnapshot")}
          </Button>
        </DialogContent>
      </Dialog>
    </div>
  )
}
