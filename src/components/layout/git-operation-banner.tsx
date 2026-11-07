"use client"

import { useCallback, useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import { Button } from "@/components/ui/button"
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
import {
  gitAbortOperation,
  gitContinueOperation,
  gitOperationState,
  gitSkipOperation,
} from "@/lib/api"
import { useGitQuickActions } from "@/hooks/use-git-quick-actions"
import type { FolderDetail, GitConflictInfo } from "@/lib/types"

export function GitOperationBanner({ folder }: { folder: FolderDetail }) {
  const t = useTranslations("Folder.gitWorkbench")
  const common = useTranslations("Folder.common")
  const [state, setState] = useState<GitConflictInfo | null>(null)
  const [confirmation, setConfirmation] = useState<"abort" | "skip" | null>(
    null
  )
  const refresh = useCallback(async () => {
    try {
      setState(await gitOperationState(folder.path))
    } catch {
      /* non-git folder */
    }
  }, [folder.path])
  useEffect(() => {
    let active = true
    const read = async () => {
      try {
        const result = await gitOperationState(folder.path)
        if (active) setState(result)
      } catch {
        if (active) setState(null)
      }
    }
    void read()
    const timer = setInterval(() => void read(), 3000)
    return () => {
      active = false
      clearInterval(timer)
    }
  }, [folder.path])
  const actions = useGitQuickActions({
    folderId: folder.id,
    folderPath: folder.path,
    onCompleted: () => void refresh(),
  })
  const restore = state?.operation === "stash" || state?.operation === "shelf"
  return (
    <>
      {state && (
        <div
          role="status"
          className="space-y-2 border-b bg-amber-500/10 p-2 text-xs"
        >
          <p>{t("operationPending", { operation: state.operation })}</p>
          <div className="flex flex-wrap gap-1">
            {state.has_conflicts ? (
              <Button
                size="sm"
                variant="outline"
                disabled={actions.running}
                onClick={() => actions.reportConflict(state)}
              >
                {t("resolveConflicts")}
              </Button>
            ) : (
              <Button
                size="sm"
                disabled={actions.running}
                onClick={() =>
                  void actions.runGitTask(t("continueOperation"), () =>
                    gitContinueOperation(folder.path, state.operation)
                  )
                }
              >
                {restore ? t("finishRestore") : t("continueOperation")}
              </Button>
            )}
            {!restore && (
              <Button
                size="sm"
                variant="outline"
                disabled={actions.running}
                onClick={() => setConfirmation("abort")}
              >
                {t("abortOperation")}
              </Button>
            )}
            {["cherry-pick", "revert", "rebase"].includes(state.operation) && (
              <Button
                size="sm"
                variant="ghost"
                disabled={actions.running}
                onClick={() => setConfirmation("skip")}
              >
                {t("skipOperation")}
              </Button>
            )}
          </div>
        </div>
      )}
      <AlertDialog
        open={confirmation !== null}
        onOpenChange={(open) => {
          if (!open) setConfirmation(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {confirmation === "skip"
                ? t("skipOperation")
                : t("abortOperation")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("operationControlDescription")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{common("cancel")}</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (!state) return
                if (confirmation === "skip")
                  void actions.runGitTask(
                    t("skipOperation"),
                    () => gitSkipOperation(folder.path, state.operation),
                    (result) => {
                      if (result.conflict) {
                        actions.reportConflict(result.conflict)
                        return false
                      }
                    }
                  )
                else
                  void actions.runGitTask(t("abortOperation"), () =>
                    gitAbortOperation(folder.path, state.operation)
                  )
              }}
            >
              {common("confirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      {actions.dialogs}
    </>
  )
}
