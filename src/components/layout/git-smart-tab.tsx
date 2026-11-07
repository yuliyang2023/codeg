"use client"
import { useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import {
  gitListAllBranches,
  gitSmartRun,
  gitSmartState,
  gitSmartResume,
} from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import type { FolderDetail, GitSmartState } from "@/lib/types"
import { useGitQuickActions } from "@/hooks/use-git-quick-actions"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

export function GitSmartTab({ folder }: { folder: FolderDetail }) {
  const t = useTranslations("Folder.gitTools")
  const [operation, setOperation] = useState("checkout")
  const [branches, setBranches] = useState<string[]>([])
  const [target, setTarget] = useState("")
  const [pending, setPending] = useState<GitSmartState | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [message, setMessage] = useState("")
  const [confirm, setConfirm] = useState<"run" | "manual" | null>(null)
  const actions = useGitQuickActions({
    folderId: folder.id,
    folderPath: folder.path,
  })
  useEffect(() => {
    let active = true
    Promise.all([gitListAllBranches(folder.path), gitSmartState(folder.path)])
      .then(([list, state]) => {
        if (active) {
          setBranches(list.local)
          setTarget(list.local[0] ?? "")
          setPending(state)
        }
      })
      .catch((err) => {
        if (active) setError(toErrorMessage(err))
      })
    const timer = setInterval(
      () =>
        void gitSmartState(folder.path)
          .then((state) => {
            if (active) setPending(state)
          })
          .catch(() => {}),
      3000
    )
    return () => {
      active = false
      clearInterval(timer)
    }
  }, [folder.path])
  async function run(mode: "run" | "resume" | "manual") {
    if (busy) return
    setBusy(true)
    setError(null)
    setConfirm(null)
    try {
      const result =
        mode === "run"
          ? await gitSmartRun(folder.path, operation, target)
          : await gitSmartResume(folder.path, mode === "manual")
      setMessage(result.message)
      if (result.conflict?.has_conflicts)
        actions.reportConflict(result.conflict)
      if (result.operation_failed) setError(result.message)
      setPending(await gitSmartState(folder.path))
    } catch (err) {
      setError(toErrorMessage(err))
      setPending(await gitSmartState(folder.path).catch(() => null))
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="space-y-3 overflow-auto p-3">
      <p className="text-xs text-muted-foreground">{t("smartHint")}</p>
      {pending ? (
        <div className="space-y-2 rounded border p-2">
          <p className="text-sm">
            {pending.operation} · {pending.target} · {pending.stage}
          </p>
          <p className="break-all font-mono text-xs">{pending.saved_ref}</p>
          {pending.message && (
            <p className="whitespace-pre-wrap text-xs">{pending.message}</p>
          )}
          <div className="flex flex-wrap gap-1">
            <Button
              size="xs"
              disabled={busy || pending.stage !== "operation"}
              onClick={() => void run("resume")}
            >
              {t("restoreSaved")}
            </Button>
            <Button
              size="xs"
              variant="outline"
              disabled={busy}
              onClick={() => setConfirm("manual")}
            >
              {t("finishManual")}
            </Button>
          </div>
        </div>
      ) : (
        <>
          <select
            aria-label={t("smartOperation")}
            className="w-full rounded border bg-background p-2 text-sm"
            value={operation}
            onChange={(e) => setOperation(e.target.value)}
          >
            {(["checkout", "merge", "rebase", "pull"] as const).map((item) => (
              <option key={item} value={item}>
                {t(item)}
              </option>
            ))}
          </select>
          {operation !== "pull" && (
            <select
              aria-label={t("targetBranch")}
              className="w-full rounded border bg-background p-2 text-sm"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
            >
              {branches.map((branch) => (
                <option key={branch} value={branch}>
                  {branch}
                </option>
              ))}
            </select>
          )}
          <Button
            size="sm"
            disabled={busy || (operation !== "pull" && !target)}
            onClick={() => setConfirm("run")}
          >
            {t("runSmart")}
          </Button>
        </>
      )}
      {error && (
        <p
          role="alert"
          className="text-xs text-destructive whitespace-pre-wrap"
        >
          {error}
        </p>
      )}
      {message && (
        <pre className="whitespace-pre-wrap break-all text-xs">{message}</pre>
      )}
      <Dialog
        open={!!confirm}
        onOpenChange={(open) => {
          if (!open) setConfirm(null)
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {confirm === "run" ? t("runSmart") : t("finishManual")}
            </DialogTitle>
            <DialogDescription>
              {confirm === "run"
                ? t("smartConfirm", {
                    operation,
                    target: operation === "pull" ? "upstream" : target,
                  })
                : t("manualConfirm")}
            </DialogDescription>
          </DialogHeader>
          <Button
            disabled={busy}
            onClick={() => void run(confirm === "run" ? "run" : "manual")}
          >
            {t("confirm")}
          </Button>
        </DialogContent>
      </Dialog>
      {actions.dialogs}
    </div>
  )
}
