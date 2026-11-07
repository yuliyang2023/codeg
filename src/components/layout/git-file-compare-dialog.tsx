"use client"
import { useState } from "react"
import { useTranslations } from "next-intl"
import { gitCompareFile } from "@/lib/api"
import type { GitFileComparison } from "@/lib/types"
import { toErrorMessage } from "@/lib/app-error"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { DiffViewer } from "@/components/diff/diff-viewer"
import { languageFromPath } from "@/lib/language-detect"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
export function GitFileCompareDialog({
  path,
  file,
  revision,
  onClose,
}: {
  path: string
  file: string
  revision?: string
  onClose: () => void
}) {
  const t = useTranslations("Folder.gitTools")
  const [base, setBase] = useState(revision ?? "HEAD")
  const [target, setTarget] = useState("WORKTREE")
  const [content, setContent] = useState<GitFileComparison | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent className="flex h-[85vh] flex-col sm:max-w-6xl">
        <DialogHeader>
          <DialogTitle>{t("compareFile")}</DialogTitle>
          <DialogDescription>
            {file} · {t("compareHint")}
          </DialogDescription>
        </DialogHeader>
        <form
          className="flex flex-wrap gap-2"
          onSubmit={(e) => {
            e.preventDefault()
            setBusy(true)
            setError(null)
            setContent(null)
            void gitCompareFile(path, file, base, target)
              .then(setContent)
              .catch((err) => setError(toErrorMessage(err)))
              .finally(() => setBusy(false))
          }}
        >
          <Input
            aria-label={t("baseRevision")}
            className="min-w-0 flex-1"
            value={base}
            onChange={(e) => setBase(e.target.value)}
          />
          <Input
            aria-label={t("targetRevision")}
            className="min-w-0 flex-1"
            value={target}
            onChange={(e) => setTarget(e.target.value)}
          />
          <Button type="submit" size="sm" disabled={busy || !base || !target}>
            {t("compare")}
          </Button>
        </form>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="min-h-0 flex-1">
          {content && (
            <DiffViewer
              original={content.original}
              modified={content.modified}
              originalLabel={content.original_label}
              modifiedLabel={content.modified_label}
              language={languageFromPath(file)}
            />
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
