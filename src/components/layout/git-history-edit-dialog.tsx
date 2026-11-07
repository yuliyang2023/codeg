"use client"

import { useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import { toast } from "sonner"
import { gitHistoryEdit, gitHistoryPlan, gitUndoCommit } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import type {
  GitConflictInfo,
  GitHistoryPlan,
  GitHistoryStep,
} from "@/lib/types"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Textarea } from "@/components/ui/textarea"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

export function GitHistoryEditDialog({
  path,
  base,
  undo,
  onClose,
  onCompleted,
  onConflict,
}: {
  path: string
  base: string
  undo: boolean
  onClose: () => void
  onCompleted: () => void
  onConflict: (conflict: GitConflictInfo) => void
}) {
  const t = useTranslations("Folder.gitHistoryEdit")
  const common = useTranslations("Folder.common")
  const [plan, setPlan] = useState<GitHistoryPlan | null>(null)
  const [steps, setSteps] = useState<GitHistoryStep[]>([])
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [confirmed, setConfirmed] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let active = true
    gitHistoryPlan(path, base)
      .then((result) => {
        if (active) {
          setPlan(result)
          setSteps(
            result.commits.map((c) => ({
              hash: c.hash,
              action: "pick",
              message: c.message,
            }))
          )
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
  }, [path, base])
  function move(index: number, direction: number) {
    setSteps((previous) => {
      const result = [...previous]
      const next = index + direction
      if (next < 0 || next >= result.length) return previous
      ;[result[index], result[next]] = [result[next], result[index]]
      return result
    })
  }
  const changed =
    !!plan &&
    steps.some(
      (step, index) =>
        step.hash !== plan.commits[index]?.hash || step.action !== "pick"
    )
  async function submit() {
    if (!plan || busy) return
    setBusy(true)
    setError(null)
    try {
      if (undo) {
        await gitUndoCommit(path, plan.head, confirmed)
        toast.success(t("undoCompleted"))
      } else {
        const result = await gitHistoryEdit(path, plan, steps, confirmed)
        if (result.conflict) {
          toast.warning(t("pending"))
          if (result.conflict.has_conflicts) onConflict(result.conflict)
        } else
          toast.success(t("completed"), { description: result.recovery_ref })
      }
      onCompleted()
      onClose()
    } catch (err) {
      setError(toErrorMessage(err))
    } finally {
      setBusy(false)
    }
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose()
      }}
    >
      <DialogContent className="flex max-h-[85vh] flex-col sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{undo ? t("undo") : t("title")}</DialogTitle>
          <DialogDescription>
            {undo ? t("undoHint") : t("description")}
          </DialogDescription>
        </DialogHeader>
        {loading && (
          <p role="status" className="text-sm text-muted-foreground">
            {t("loading")}
          </p>
        )}
        {plan && (
          <>
            <p className="break-all font-mono text-xs text-muted-foreground">
              HEAD {plan.head.slice(0, 12)}
            </p>
            {!undo && (
              <div className="min-h-0 space-y-2 overflow-auto">
                {steps.map((step, index) => (
                  <div key={step.hash} className="space-y-2 rounded border p-2">
                    <div className="flex items-center gap-2">
                      <code className="text-xs">{step.hash.slice(0, 8)}</code>
                      <select
                        className="rounded border bg-background p-1 text-xs"
                        aria-label={t("action", {
                          hash: step.hash.slice(0, 8),
                        })}
                        disabled={busy}
                        value={step.action}
                        onChange={(e) =>
                          setSteps((previous) =>
                            previous.map((s) =>
                              s.hash === step.hash
                                ? {
                                    ...s,
                                    action: e.target
                                      .value as GitHistoryStep["action"],
                                  }
                                : s
                            )
                          )
                        }
                      >
                        {(
                          ["pick", "reword", "squash", "fixup", "drop"] as const
                        ).map((action) => (
                          <option key={action} value={action}>
                            {t(action)}
                          </option>
                        ))}
                      </select>
                      <Button
                        className="ml-auto"
                        size="icon-xs"
                        variant="ghost"
                        disabled={busy || index === 0}
                        aria-label={t("up", { hash: step.hash.slice(0, 8) })}
                        onClick={() => move(index, -1)}
                      >
                        ↑
                      </Button>
                      <Button
                        size="icon-xs"
                        variant="ghost"
                        disabled={busy || index === steps.length - 1}
                        aria-label={t("down", { hash: step.hash.slice(0, 8) })}
                        onClick={() => move(index, 1)}
                      >
                        ↓
                      </Button>
                    </div>
                    {step.action === "reword" ? (
                      <Textarea
                        aria-label={t("message", {
                          hash: step.hash.slice(0, 8),
                        })}
                        disabled={busy}
                        value={step.message ?? ""}
                        onChange={(e) =>
                          setSteps((previous) =>
                            previous.map((s) =>
                              s.hash === step.hash
                                ? { ...s, message: e.target.value }
                                : s
                            )
                          )
                        }
                      />
                    ) : (
                      <p className="truncate text-xs text-muted-foreground">
                        {
                          plan.commits
                            .find((c) => c.hash === step.hash)
                            ?.message.split("\n")[0]
                        }
                      </p>
                    )}
                  </div>
                ))}
              </div>
            )}
            {plan.published && (
              <label className="flex items-start gap-2 text-sm text-amber-600">
                <Checkbox
                  checked={confirmed}
                  disabled={busy}
                  onCheckedChange={(value) => setConfirmed(!!value)}
                />
                {t("published")}
              </label>
            )}
            {undo && !plan.parent && (
              <p className="text-sm text-destructive">{t("rootUndo")}</p>
            )}
          </>
        )}
        {error && (
          <p
            role="alert"
            className="whitespace-pre-wrap text-sm text-destructive"
          >
            {error}
          </p>
        )}
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={onClose}>
            {common("cancel")}
          </Button>
          <Button
            disabled={
              loading ||
              busy ||
              !plan ||
              (!undo && !changed) ||
              (undo && !plan?.parent) ||
              (plan?.published && !confirmed)
            }
            onClick={() => void submit()}
          >
            {busy ? t("loading") : undo ? t("undo") : t("apply")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
