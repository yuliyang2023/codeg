"use client"

import { useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import { Loader2 } from "lucide-react"
import { toast } from "sonner"
import { gitAmend, gitAmendInfo, gitStatus } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import type { GitAmendInfo, GitStatusEntry } from "@/lib/types"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { GitFileSelection, selectedGitPaths } from "./git-file-selection"

export function AmendCommitDialog({
  path,
  open,
  onOpenChange,
  onCompleted,
}: {
  path: string
  open: boolean
  onOpenChange: (open: boolean) => void
  onCompleted: () => void
}) {
  const t = useTranslations("Folder.gitWorkbench")
  const common = useTranslations("Folder.common")
  const [info, setInfo] = useState<GitAmendInfo | null>(null)
  const [entries, setEntries] = useState<GitStatusEntry[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [message, setMessage] = useState("")
  const [loading, setLoading] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    if (!open) return
    let cancelled = false
    setInfo(null)
    setLoading(true)
    setError(null)
    setSelected(new Set())
    Promise.all([gitAmendInfo(path), gitStatus(path, true)])
      .then(([head, files]) => {
        if (cancelled) return
        setInfo(head)
        setMessage(head.message)
        setEntries(files)
      })
      .catch((err) => {
        if (!cancelled) setError(toErrorMessage(err))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [path, open])
  async function submit() {
    if (!info || !message.trim() || busy) return
    setBusy(true)
    setError(null)
    try {
      const result = await gitAmend(
        path,
        message.trim(),
        selectedGitPaths(entries, selected),
        info.head
      )
      if (result.index_warning) toast.warning(result.index_warning)
      toast.success(t("amendCompleted"))
      onCompleted()
      onOpenChange(false)
    } catch (err) {
      setError(toErrorMessage(err))
    } finally {
      setBusy(false)
    }
  }
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!busy) onOpenChange(next)
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("amend")}</DialogTitle>
          <DialogDescription>{t("amendDescription")}</DialogDescription>
        </DialogHeader>
        {loading ? (
          <Loader2 className="size-4 animate-spin" />
        ) : (
          info && (
            <>
              <p className="font-mono text-xs">{info.head.slice(0, 12)}</p>
              {info.published && (
                <p className="text-sm text-amber-600">{t("amendPublished")}</p>
              )}
              <Textarea
                aria-label={t("amendMessage")}
                value={message}
                disabled={busy}
                onChange={(event) => setMessage(event.target.value)}
              />
              <GitFileSelection
                entries={entries}
                selected={selected}
                onChange={setSelected}
                disabled={busy}
              />
              <p className="text-xs text-muted-foreground">
                {t("amendFilesHint")}
              </p>
            </>
          )
        )}
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <DialogFooter>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => onOpenChange(false)}
          >
            {common("cancel")}
          </Button>
          <Button
            disabled={loading || busy || !info || !message.trim()}
            onClick={() => void submit()}
          >
            {busy && <Loader2 className="size-4 animate-spin" />}
            {t("amend")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
