"use client"
import { useCallback, useEffect, useMemo, useState } from "react"
import { useTranslations } from "next-intl"
import { gitChangelists, gitChangelistUpdate, gitStatus } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import type { FolderDetail, GitChangelists, GitStatusEntry } from "@/lib/types"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { GitFileSelection, selectedGitPaths } from "./git-file-selection"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

export function changelistForFile(
  state: GitChangelists,
  file: string,
  status?: string
): string {
  const key =
    status && /[RC]/.test(status) && file.includes(" -> ")
      ? file.slice(file.lastIndexOf(" -> ") + 4)
      : file
  return (
    state.lists.find(
      (list) => list.files.includes(file) || list.files.includes(key)
    )?.id ?? state.default_id
  )
}
export function useChangelists(path: string | null) {
  const [loaded, setLoaded] = useState<{
    path: string
    state: GitChangelists
  } | null>(null)
  const [selection, setSelection] = useState<{
    path: string
    id: string
  } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const refresh = useCallback(async () => {
    if (!path) return
    try {
      const state = await gitChangelists(path)
      setLoaded((previous) =>
        previous?.path === path && previous.state.revision >= state.revision
          ? previous
          : { path, state }
      )
      setError(null)
    } catch (err) {
      setError(toErrorMessage(err))
    }
  }, [path])
  useEffect(() => {
    let active = true
    const load = () => {
      if (path)
        Promise.resolve()
          .then(() => gitChangelists(path))
          .then((state) => {
            if (active) {
              setLoaded((previous) =>
                previous?.path === path &&
                previous.state.revision >= state.revision
                  ? previous
                  : { path, state }
              )
              setError(null)
            }
          })
          .catch((err) => {
            if (active) setError(toErrorMessage(err))
          })
    }
    load()
    const timer = setInterval(load, 3000)
    return () => {
      active = false
      clearInterval(timer)
    }
  }, [path])
  const state = loaded?.path === path ? loaded.state : null
  const id =
    selection?.path === path &&
    state?.lists.some((list) => list.id === selection.id)
      ? selection.id
      : "all"
  return {
    state,
    id,
    error,
    refresh,
    setId: (id: string) => {
      if (path) setSelection({ path, id })
    },
    includes: (file: string) =>
      id === "all" || !state || changelistForFile(state, file) === id,
  }
}
export function ChangelistFilter({
  model,
}: {
  model: ReturnType<typeof useChangelists>
}) {
  const t = useTranslations("Folder.gitTools")
  if (!model.state) return null
  return (
    <div className="flex shrink-0 items-center gap-2 border-b px-2 py-1">
      <select
        className="min-w-0 flex-1 bg-background text-xs"
        aria-label={t("changelist")}
        value={model.id}
        onChange={(e) => model.setId(e.target.value)}
      >
        <option value="all">{t("allLists")}</option>
        {model.state.lists.map((list) => (
          <option key={list.id} value={list.id}>
            {list.name}
            {list.id === model.state?.default_id ? " ★" : ""}
          </option>
        ))}
      </select>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label={t("refresh")}
        onClick={() => void model.refresh()}
      >
        ↻
      </Button>
    </div>
  )
}

export function GitChangelistsTab({ folder }: { folder: FolderDetail }) {
  const t = useTranslations("Folder.gitTools")
  const model = useChangelists(folder.path)
  const [entries, setEntries] = useState<GitStatusEntry[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [name, setName] = useState("")
  const [destination, setDestination] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [confirm, setConfirm] = useState<string | null>(null)
  useEffect(() => {
    let active = true
    gitStatus(folder.path, true)
      .then((files) => {
        if (active) setEntries(files)
      })
      .catch((err) => {
        if (active) setError(toErrorMessage(err))
      })
    return () => {
      active = false
    }
  }, [folder.path])
  const active = model.state?.lists.find((list) => list.id === model.id)
  const visible = useMemo(
    () =>
      entries.filter(
        (entry) =>
          model.id === "all" ||
          !model.state ||
          changelistForFile(model.state, entry.file, entry.status) === model.id
      ),
    [entries, model.id, model.state]
  )
  async function update(
    operation: string,
    values: { id?: string; name?: string; files?: string[] }
  ) {
    if (!model.state || busy) return
    setBusy(true)
    setError(null)
    try {
      await gitChangelistUpdate(
        folder.path,
        model.state.revision,
        operation,
        values
      )
      await model.refresh()
      setSelected(new Set())
      setConfirm(null)
      if (operation === "create") setName("")
    } catch (err) {
      setError(toErrorMessage(err))
      await model.refresh()
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="flex h-full min-h-0 flex-col">
      <ChangelistFilter model={model} />
      <div className="flex flex-wrap gap-1 border-b p-2">
        {model.state?.lists.map((list) => (
          <button
            key={list.id}
            className="rounded border px-2 py-1 text-xs hover:bg-muted"
            disabled={busy}
            onClick={() => model.setId(list.id)}
            onDragOver={(event) => event.preventDefault()}
            onDrop={(event) => {
              event.preventDefault()
              try {
                const names = JSON.parse(
                  event.dataTransfer.getData("application/x-codeg-changes")
                )
                if (
                  Array.isArray(names) &&
                  names.every((name) =>
                    entries.some((entry) => entry.file === name)
                  )
                )
                  void update("move", {
                    id: list.id,
                    files: selectedGitPaths(entries, new Set(names)),
                  })
              } catch {
                /* Ignore unrelated drags. */
              }
            }}
          >
            {list.name}
            {list.id === model.state?.default_id ? " ★" : ""}
          </button>
        ))}
      </div>
      <div className="space-y-2 p-2">
        <div className="flex gap-1">
          <Input
            aria-label={t("listName")}
            placeholder={t("listName")}
            value={name}
            disabled={busy}
            onChange={(e) => setName(e.target.value)}
          />
          <Button
            size="xs"
            disabled={busy || !name.trim()}
            onClick={() => void update("create", { name })}
          >
            {t("create")}
          </Button>
        </div>
        {active && (
          <div className="flex flex-wrap gap-1">
            <Button
              size="xs"
              variant="outline"
              disabled={busy || !name.trim()}
              onClick={() => void update("rename", { id: active.id, name })}
            >
              {t("rename")}
            </Button>
            <Button
              size="xs"
              variant="outline"
              disabled={busy}
              onClick={() => void update("default", { id: active.id })}
            >
              {t("makeDefault")}
            </Button>
            <Button
              size="xs"
              variant="ghost"
              disabled={busy || model.state?.lists.length === 1}
              onClick={() => setConfirm(active.id)}
            >
              {t("delete")}
            </Button>
          </div>
        )}
        <p className="text-xs text-muted-foreground">{t("listsHint")}</p>
        <div className="flex gap-1">
          <select
            className="min-w-0 flex-1 rounded border bg-background p-1 text-xs"
            aria-label={t("moveTo")}
            value={destination}
            onChange={(e) => setDestination(e.target.value)}
          >
            <option value="">{t("moveTo")}</option>
            {model.state?.lists.map((list) => (
              <option key={list.id} value={list.id}>
                {list.name}
              </option>
            ))}
          </select>
          <Button
            size="xs"
            disabled={busy || !destination || !selected.size}
            onClick={() =>
              void update("move", {
                id: destination,
                files: selectedGitPaths(entries, selected),
              })
            }
          >
            {t("move")}
          </Button>
        </div>
        {(error || model.error) && (
          <p role="alert" className="text-xs text-destructive">
            {error ?? model.error}
          </p>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-auto p-2">
        <GitFileSelection
          entries={visible}
          selected={selected}
          onChange={setSelected}
          disabled={busy}
          onDragFile={(file, event) => {
            const files = selected.has(file) ? [...selected] : [file]
            event.dataTransfer.setData(
              "application/x-codeg-changes",
              JSON.stringify(files)
            )
            event.dataTransfer.effectAllowed = "move"
          }}
        />
      </div>
      <Dialog
        open={!!confirm}
        onOpenChange={(open) => {
          if (!open && !busy) setConfirm(null)
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("delete")}</DialogTitle>
            <DialogDescription>{t("deleteListHint")}</DialogDescription>
          </DialogHeader>
          <Button
            disabled={busy}
            onClick={() => {
              if (confirm) void update("delete", { id: confirm })
            }}
          >
            {t("delete")}
          </Button>
        </DialogContent>
      </Dialog>
    </div>
  )
}
