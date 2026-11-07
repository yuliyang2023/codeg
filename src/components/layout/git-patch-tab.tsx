"use client"
import { useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import {
  gitStatus,
  gitPatchCreate,
  gitPatchPreview,
  gitPatchApply,
} from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import type { FolderDetail, GitStatusEntry, GitPatchPreview } from "@/lib/types"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Textarea } from "@/components/ui/textarea"
import { GitFileSelection, selectedGitPaths } from "./git-file-selection"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

export function downloadPatch(text: string, name = "changes.patch") {
  const url = URL.createObjectURL(
    new Blob([text], { type: "text/plain;charset=utf-8" })
  )
  const link = document.createElement("a")
  link.href = url
  link.download = name
  link.click()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}
export function GitPatchTab({ folder }: { folder: FolderDetail }) {
  const t = useTranslations("Folder.gitTools")
  const [files, setFiles] = useState<GitStatusEntry[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [patch, setPatch] = useState("")
  const [reverse, setReverse] = useState(false)
  const [stage, setStage] = useState(false)
  const [preview, setPreview] = useState<GitPatchPreview | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [confirm, setConfirm] = useState(false)
  const [message, setMessage] = useState("")
  useEffect(() => {
    let active = true
    gitStatus(folder.path, true)
      .then((entries) => {
        if (active) setFiles(entries)
      })
      .catch((err) => {
        if (active) setError(toErrorMessage(err))
      })
    return () => {
      active = false
    }
  }, [folder.path])
  async function task(action: () => Promise<void>) {
    setBusy(true)
    setError(null)
    setMessage("")
    try {
      await action()
    } catch (err) {
      setError(toErrorMessage(err))
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="h-full space-y-3 overflow-auto p-2">
      <p className="text-xs text-muted-foreground">{t("patchHint")}</p>
      <GitFileSelection
        entries={files}
        selected={selected}
        onChange={setSelected}
        disabled={busy}
      />
      <Button
        size="xs"
        disabled={busy || !selected.size}
        onClick={() =>
          void task(async () => {
            setPatch(
              await gitPatchCreate(
                folder.path,
                selectedGitPaths(files, selected)
              )
            )
            setPreview(null)
          })
        }
      >
        {t("createPatch")}
      </Button>
      <label className="block text-xs">
        {t("importPatch")}
        <input
          type="file"
          accept=".patch,.diff,text/plain"
          disabled={busy}
          onChange={(e) => {
            const file = e.target.files?.[0]
            if (file)
              void task(async () => {
                if (file.size > 10 * 1024 * 1024)
                  throw new Error(t("patchTooLarge"))
                setPatch(await file.text())
                setPreview(null)
              })
          }}
        />
      </label>
      <Textarea
        aria-label={t("patchText")}
        className="min-h-40 font-mono text-xs"
        value={patch}
        disabled={busy}
        onChange={(e) => {
          setPatch(e.target.value)
          setPreview(null)
        }}
      />
      <div className="flex flex-wrap gap-2 text-xs">
        <label className="flex items-center gap-1">
          <Checkbox
            checked={reverse}
            disabled={busy}
            onCheckedChange={(v) => {
              setReverse(!!v)
              setPreview(null)
            }}
          />
          {t("reverse")}
        </label>
        <label className="flex items-center gap-1">
          <Checkbox
            checked={stage}
            disabled={busy}
            onCheckedChange={(v) => {
              setStage(!!v)
              setPreview(null)
            }}
          />
          {t("stagePatch")}
        </label>
      </div>
      <div className="flex flex-wrap gap-1">
        <Button
          size="xs"
          variant="outline"
          disabled={!patch || busy}
          onClick={() => downloadPatch(patch)}
        >
          {t("exportPatch")}
        </Button>
        <Button
          size="xs"
          disabled={!patch || busy}
          onClick={() =>
            void task(async () =>
              setPreview(
                await gitPatchPreview(folder.path, patch, reverse, stage)
              )
            )
          }
        >
          {t("checkPatch")}
        </Button>
        <Button
          size="xs"
          disabled={!preview?.applicable || busy}
          onClick={() => setConfirm(true)}
        >
          {t("applyPatch")}
        </Button>
      </div>
      {preview && (
        <pre className="whitespace-pre-wrap break-all text-xs">
          {preview.summary}
          {preview.error && "\n" + preview.error}
        </pre>
      )}
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
      {message && (
        <p role="status" className="text-xs">
          {message}
        </p>
      )}
      <Dialog open={confirm} onOpenChange={setConfirm}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("applyPatch")}</DialogTitle>
            <DialogDescription>{t("patchConfirm")}</DialogDescription>
          </DialogHeader>
          <Button
            disabled={busy}
            onClick={() => {
              setConfirm(false)
              void task(async () => {
                await gitPatchApply(folder.path, patch, reverse, stage)
                setPreview(null)
                setFiles(await gitStatus(folder.path, true))
                setMessage(t("patchApplied"))
              })
            }}
          >
            {t("applyPatch")}
          </Button>
        </DialogContent>
      </Dialog>
    </div>
  )
}
