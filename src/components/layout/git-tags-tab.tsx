"use client"

import { useCallback, useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import { gitListRemotes, gitTagAction, gitTagCreate, gitTags } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import type { FolderDetail, GitTag } from "@/lib/types"
import { useGitQuickActions } from "@/hooks/use-git-quick-actions"
import { useWorkspaceActions } from "@/contexts/workspace-context"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

type Operation = "delete" | "checkout" | "merge" | "push" | "delete-remote"
export function GitTagsTab({ folder }: { folder: FolderDetail }) {
  const t = useTranslations("Folder.gitTags")
  const common = useTranslations("Folder.common")
  const { openCommitDiff } = useWorkspaceActions()
  const [tags, setTags] = useState<GitTag[]>([])
  const [remotes, setRemotes] = useState<string[]>([])
  const [remote, setRemote] = useState("")
  const [filter, setFilter] = useState("")
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [create, setCreate] = useState(false)
  const [name, setName] = useState("")
  const [revision, setRevision] = useState("HEAD")
  const [message, setMessage] = useState("")
  const [target, setTarget] = useState<{
    tag: GitTag
    operation: Operation
  } | null>(null)
  const [busy, setBusy] = useState(false)
  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const [entries, configured] = await Promise.all([
        gitTags(folder.path),
        gitListRemotes(folder.path),
      ])
      setTags(entries)
      setRemotes(configured.map((r) => r.name))
      setRemote((previous) =>
        configured.some((r) => r.name === previous)
          ? previous
          : (configured[0]?.name ?? "")
      )
    } catch (err) {
      setError(toErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }, [folder.path])
  useEffect(() => {
    void refresh()
  }, [refresh])
  const actions = useGitQuickActions({
    folderId: folder.id,
    folderPath: folder.path,
    onCompleted: () => void refresh(),
  })
  async function submit() {
    if (busy) return
    setBusy(true)
    setError(null)
    try {
      if (create) {
        await gitTagCreate(folder.path, name.trim(), revision.trim(), message)
        setCreate(false)
        setName("")
        setMessage("")
      } else if (target) {
        const result = await gitTagAction(
          folder.path,
          target.tag,
          target.operation,
          remote
        )
        setTarget(null)
        if (result.conflict) actions.reportConflict(result.conflict)
      }
      await refresh()
    } catch (err) {
      setError(toErrorMessage(err))
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex flex-wrap gap-1 border-b p-2">
        <Button
          size="xs"
          variant="ghost"
          onClick={() => {
            setError(null)
            setCreate(true)
          }}
        >
          {t("create")}
        </Button>
        <Button
          size="xs"
          variant="ghost"
          disabled={loading || busy}
          onClick={() => void refresh()}
        >
          {common("refresh")}
        </Button>
        <Input
          className="h-7"
          aria-label={t("search")}
          placeholder={t("search")}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
      {error && !create && !target && (
        <p role="alert" className="p-2 text-xs text-destructive">
          {error}
        </p>
      )}
      {loading && (
        <p className="p-2 text-xs text-muted-foreground">{t("loading")}</p>
      )}
      <ScrollArea className="min-h-0 flex-1">
        <div className="space-y-2 p-2">
          {tags
            .filter((tag) =>
              tag.name.toLowerCase().includes(filter.toLowerCase())
            )
            .map((tag) => (
              <div key={tag.name} className="space-y-2 rounded border p-2">
                <p className="break-all text-sm font-medium">{tag.name}</p>
                <p
                  className="truncate text-xs text-muted-foreground"
                  title={tag.message}
                >
                  {tag.annotated ? t("annotated") : t("lightweight")} ·{" "}
                  {tag.commit.slice(0, 8)} {tag.message}
                </p>
                <div className="flex flex-wrap gap-1">
                  <Button
                    size="xs"
                    variant="ghost"
                    onClick={() =>
                      void openCommitDiff(tag.commit, undefined, tag.name, {
                        folderId: folder.id,
                      }).catch((err) => setError(toErrorMessage(err)))
                    }
                  >
                    {t("view")}
                  </Button>
                  {(
                    [
                      "checkout",
                      "merge",
                      "push",
                      "delete-remote",
                      "delete",
                    ] as const
                  ).map((operation) => (
                    <Button
                      key={operation}
                      size="xs"
                      variant="ghost"
                      disabled={
                        busy ||
                        actions.running ||
                        ((operation === "push" ||
                          operation === "delete-remote") &&
                          remotes.length === 0)
                      }
                      onClick={() => {
                        setError(null)
                        setTarget({ tag, operation })
                      }}
                    >
                      {t(operation)}
                    </Button>
                  ))}
                </div>
              </div>
            ))}
          {!loading && tags.length === 0 && (
            <p className="text-xs text-muted-foreground">{t("empty")}</p>
          )}
        </div>
      </ScrollArea>
      <Dialog
        open={create || target !== null}
        onOpenChange={(open) => {
          if (!open && !busy) {
            setCreate(false)
            setTarget(null)
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {create ? t("create") : t(target?.operation ?? "create")}
            </DialogTitle>
            <DialogDescription>
              {create
                ? t("createHint")
                : t(
                    target?.operation === "checkout"
                      ? "detachedHint"
                      : "confirmHint",
                    { name: target?.tag.name ?? "" }
                  )}
            </DialogDescription>
          </DialogHeader>
          {create && (
            <>
              <Input
                aria-label={t("name")}
                placeholder={t("name")}
                value={name}
                disabled={busy}
                onChange={(e) => setName(e.target.value)}
              />
              <Input
                aria-label={t("revision")}
                placeholder={t("revision")}
                value={revision}
                disabled={busy}
                onChange={(e) => setRevision(e.target.value)}
              />
              <Input
                aria-label={t("message")}
                placeholder={t("message")}
                value={message}
                disabled={busy}
                onChange={(e) => setMessage(e.target.value)}
              />
            </>
          )}
          {(target?.operation === "push" ||
            target?.operation === "delete-remote") && (
            <label className="flex items-center justify-between text-sm">
              {t("remote")}
              <select
                className="rounded border bg-background p-1"
                value={remote}
                disabled={busy}
                onChange={(e) => setRemote(e.target.value)}
              >
                {remotes.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
          )}
          {error && (
            <p
              role="alert"
              className="text-xs text-destructive whitespace-pre-wrap"
            >
              {error}
            </p>
          )}
          <DialogFooter>
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => {
                setCreate(false)
                setTarget(null)
              }}
            >
              {common("cancel")}
            </Button>
            <Button
              disabled={busy || (create && (!name.trim() || !revision.trim()))}
              onClick={() => void submit()}
            >
              {busy
                ? t("loading")
                : create
                  ? t("create")
                  : t(target?.operation ?? "create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      {actions.dialogs}
    </div>
  )
}
