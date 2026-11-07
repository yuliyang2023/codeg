"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import {
  Archive,
  ArchiveRestore,
  Eye,
  Loader2,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react"
import { useTranslations } from "next-intl"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { useGitQuickActions } from "@/hooks/use-git-quick-actions"
import { useImeGuard } from "@/hooks/use-ime-guard"
import {
  gitCommitTools,
  type GitCommitToolsState,
  gitShelfApply,
  gitShelfDiff,
  gitShelfDrop,
  gitShelfList,
  gitShelfSave,
  gitStatus,
} from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import type { FolderDetail, GitShelfEntry, GitStatusEntry } from "@/lib/types"
import { GitShelfManage } from "./git-shelf-manage"
import { GitFileSelection, selectedGitPaths } from "./git-file-selection"

export function GitShelfTab({ folder }: { folder: FolderDetail }) {
  const tExtras = useTranslations("Folder.gitExtras")
  const [metadata, setMetadata] = useState<GitCommitToolsState>({
    messages: [],
    shelves: {},
  })
  const [manage, setManage] = useState<GitShelfEntry | null>(null)
  const [group, setGroup] = useState("")
  const t = useTranslations("Folder.gitWorkbench")
  const common = useTranslations("Folder.common")
  const ime = useImeGuard()
  const [entries, setEntries] = useState<GitShelfEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saveOpen, setSaveOpen] = useState(false)
  const [message, setMessage] = useState("")
  const [files, setFiles] = useState<GitStatusEntry[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [filesLoading, setFilesLoading] = useState(false)
  const [filesError, setFilesError] = useState<string | null>(null)
  useEffect(() => {
    if (!saveOpen) return
    let active = true
    setFilesLoading(true)
    setFilesError(null)
    setSelected(new Set())
    gitStatus(folder.path, true)
      .then((entries) => {
        if (active) {
          setFiles(entries)
          setSelected(new Set(entries.map((entry) => entry.file)))
        }
      })
      .catch((error) => {
        if (active) setFilesError(toErrorMessage(error))
      })
      .finally(() => {
        if (active) setFilesLoading(false)
      })
    return () => {
      active = false
    }
  }, [folder.path, saveOpen])
  const [dropTarget, setDropTarget] = useState<GitShelfEntry | null>(null)
  const [preview, setPreview] = useState<GitShelfEntry | null>(null)
  const [diff, setDiff] = useState<string | null>(null)
  const previewRequest = useRef(0)
  const listRequest = useRef(0)

  const refresh = useCallback(async () => {
    const request = ++listRequest.current
    setLoading(true)
    setError(null)
    try {
      const [list, meta] = await Promise.all([
        gitShelfList(folder.path),
        gitCommitTools(folder.path, "state"),
      ])
      if (request === listRequest.current) setMetadata(meta)
      if (request === listRequest.current) setEntries(list)
    } catch (err) {
      if (request === listRequest.current) setError(toErrorMessage(err))
    } finally {
      if (request === listRequest.current) setLoading(false)
    }
  }, [folder.path])

  useEffect(() => {
    void refresh()
    const invalidate = () => {
      listRequest.current++
      previewRequest.current++
    }
    return () => {
      invalidate()
    }
  }, [refresh])

  const actions = useGitQuickActions({
    folderId: folder.id,
    folderPath: folder.path,
    onCompleted: () => void refresh(),
  })

  async function showDiff(entry: GitShelfEntry) {
    const request = ++previewRequest.current
    setPreview(entry)
    setDiff(null)
    try {
      const result = await gitShelfDiff(folder.path, entry.ref_name)
      if (request === previewRequest.current) setDiff(result)
    } catch (err) {
      if (request !== previewRequest.current) return
      setPreview(null)
      toast.error(toErrorMessage(err))
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center justify-between border-b p-2">
        <Button
          size="sm"
          variant="ghost"
          disabled={actions.running}
          onClick={() => setSaveOpen(true)}
        >
          <Plus className="size-4" />
          {t("saveShelf")}
        </Button>
        <Button
          size="icon-sm"
          variant="ghost"
          aria-label={t("refresh")}
          disabled={loading || actions.running}
          onClick={() => void refresh()}
        >
          <RefreshCw className="size-4" />
        </Button>
      </div>
      <p className="px-3 py-2 text-xs text-muted-foreground">
        {t("shelfHint")}
      </p>
      <select
        aria-label={tExtras("shelfGroup")}
        value={group}
        onChange={(e) => setGroup(e.target.value)}
        className="m-2 rounded border bg-background p-1 text-xs"
      >
        <option value="">{tExtras("allGroups")}</option>
        {[
          ...new Set(
            entries
              .map((e) => metadata.shelves[e.ref_name]?.group ?? "")
              .filter(Boolean)
          ),
        ].map((g) => (
          <option key={g} value={g}>
            {g}
          </option>
        ))}
      </select>
      <ScrollArea className="min-h-0 flex-1">
        <div className="space-y-2 p-2">
          {loading ? (
            <Loader2
              className="m-auto size-4 animate-spin"
              aria-label={t("loading")}
            />
          ) : error ? (
            <p role="alert" className="p-2 text-xs text-destructive">
              {error}
            </p>
          ) : entries.length === 0 ? (
            <div className="flex flex-col items-center gap-2 p-6 text-sm text-muted-foreground">
              <Archive className="size-5" />
              {t("emptyShelf")}
            </div>
          ) : (
            entries
              .filter(
                (entry) =>
                  !group || metadata.shelves[entry.ref_name]?.group === group
              )
              .map((entry) => (
                <div key={entry.ref_name} className="rounded-lg border p-3">
                  <p
                    className="truncate text-sm font-medium"
                    title={
                      metadata.shelves[entry.ref_name]?.name ?? entry.message
                    }
                  >
                    {metadata.shelves[entry.ref_name]?.name ?? entry.message}
                  </p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {metadata.shelves[entry.ref_name]?.group} · {entry.date}
                  </p>
                  <div className="mt-2 flex flex-wrap gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={actions.running}
                      onClick={() => setManage(entry)}
                    >
                      {tExtras("manageShelf")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => void showDiff(entry)}
                    >
                      <Eye className="size-3.5" />
                      {t("preview")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={actions.running}
                      onClick={() =>
                        void actions.runGitTask(
                          t("restoreShelf"),
                          () => gitShelfApply(folder.path, entry.ref_name),
                          (result) => {
                            if (result?.conflict?.has_conflicts) {
                              actions.reportConflict(result.conflict)
                              return false
                            }
                          }
                        )
                      }
                    >
                      <ArchiveRestore className="size-3.5" />
                      {t("restoreShelf")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      className="ml-auto text-destructive"
                      aria-label={t("deleteShelf")}
                      disabled={actions.running}
                      onClick={() => setDropTarget(entry)}
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  </div>
                </div>
              ))
          )}
        </div>
      </ScrollArea>
      {manage && (
        <GitShelfManage
          path={folder.path}
          reference={manage.ref_name}
          name={metadata.shelves[manage.ref_name]?.name ?? manage.message}
          group={metadata.shelves[manage.ref_name]?.group ?? ""}
          onClose={() => setManage(null)}
          onChanged={() => void refresh()}
        />
      )}
      <Dialog
        open={saveOpen}
        onOpenChange={(open) => {
          if (!actions.running) setSaveOpen(open)
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("saveShelf")}</DialogTitle>
            <DialogDescription>{t("saveShelfDescription")}</DialogDescription>
          </DialogHeader>
          <form
            onSubmit={(event) => {
              event.preventDefault()
              if (
                actions.running ||
                !message.trim() ||
                filesLoading ||
                filesError ||
                selected.size === 0
              )
                return
              void actions.runGitTask(t("saveShelf"), async () => {
                await gitShelfSave(
                  folder.path,
                  message.trim(),
                  selectedGitPaths(files, selected)
                )
                setSaveOpen(false)
                setMessage("")
              })
            }}
          >
            <Label htmlFor="shelf-name">{t("shelfName")}</Label>
            <Input
              id="shelf-name"
              className="mt-2"
              autoFocus
              value={message}
              disabled={actions.running}
              onChange={(event) => setMessage(event.target.value)}
              {...ime.props}
              onKeyDown={(event) => {
                if (event.key === "Enter" && ime.isComposing(event))
                  event.preventDefault()
              }}
            />
            <div className="mt-3">
              {filesLoading ? (
                <Loader2 className="size-4 animate-spin" />
              ) : filesError ? (
                <p role="alert" className="text-sm text-destructive">
                  {filesError}
                </p>
              ) : (
                <GitFileSelection
                  entries={files}
                  selected={selected}
                  onChange={setSelected}
                  disabled={actions.running}
                />
              )}
            </div>
            <DialogFooter className="mt-4">
              <Button
                type="button"
                variant="outline"
                disabled={actions.running}
                onClick={() => setSaveOpen(false)}
              >
                {common("cancel")}
              </Button>
              <Button
                type="submit"
                disabled={
                  actions.running ||
                  !message.trim() ||
                  filesLoading ||
                  !!filesError ||
                  selected.size === 0
                }
              >
                {actions.running && <Loader2 className="size-4 animate-spin" />}
                {t("saveShelf")}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <AlertDialog
        open={dropTarget !== null}
        onOpenChange={(open) => {
          if (!open) setDropTarget(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("deleteShelf")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("deleteShelfDescription", { name: dropTarget?.message ?? "" })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{common("cancel")}</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (dropTarget)
                  void actions.runGitTask(t("deleteShelf"), () =>
                    gitShelfDrop(folder.path, dropTarget.ref_name)
                  )
              }}
            >
              {t("deleteShelf")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <Dialog
        open={preview !== null}
        onOpenChange={(open) => {
          if (!open) {
            previewRequest.current++
            setPreview(null)
          }
        }}
      >
        <DialogContent className="flex h-[80vh] max-w-[90vw] flex-col sm:max-w-5xl">
          <DialogHeader>
            <DialogTitle>{t("preview")}</DialogTitle>
            <DialogDescription>{preview?.message}</DialogDescription>
          </DialogHeader>
          <div className="min-h-0 flex-1 overflow-auto rounded-md border bg-muted/30 p-3">
            {diff === null ? (
              <Loader2
                className="size-4 animate-spin"
                aria-label={t("loading")}
              />
            ) : (
              <pre className="text-xs">{diff}</pre>
            )}
          </div>
        </DialogContent>
      </Dialog>
      {actions.dialogs}
    </div>
  )
}
