"use client"
import { useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import {
  gitHistoryAdvanced,
  type GitHistoryAdvancedReview,
  type GitHistoryAdvancedRequest,
} from "@/lib/api"
import type { GitConflictInfo } from "@/lib/types"
import { toErrorMessage } from "@/lib/app-error"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { Input } from "@/components/ui/input"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog"
export function GitHistoryAdvancedDialog({
  path,
  commits,
  initialOperation = "cherry-pick",
  onClose,
  onCompleted,
  onConflict,
}: {
  path: string
  commits: string[]
  initialOperation?: "cherry-pick" | "revert"
  onClose: () => void
  onCompleted: () => void
  onConflict: (conflict: GitConflictInfo) => void
}) {
  const t = useTranslations("Folder.gitAdvanced")
  const [review, setReview] = useState<GitHistoryAdvancedReview | null>(null),
    [operation, setOperation] =
      useState<GitHistoryAdvancedRequest["operation"]>(initialOperation),
    [messages, setMessages] = useState<Record<string, string>>({}),
    [files, setFiles] = useState<Set<string>>(new Set()),
    [second, setSecond] = useState(""),
    [mainline, setMainline] = useState(1),
    [confirmed, setConfirmed] = useState(false),
    [published, setPublished] = useState(false),
    [busy, setBusy] = useState(false),
    [error, setError] = useState<string | null>(null),
    [result, setResult] = useState<string | null>(null)
  useEffect(() => {
    let active = true
    gitHistoryAdvanced(path, { operation: "inspect", commits })
      .then((r) => {
        if (active) {
          setReview(r)
          setMessages(
            Object.fromEntries(r.commits.map((c) => [c.hash, c.message]))
          )
        }
      })
      .catch((e) => {
        if (active) setError(toErrorMessage(e))
      })
    return () => {
      active = false
    }
  }, [path, commits])
  const rewrite = operation === "split" || operation === "reword-merges"
  const merges = review?.commits.filter((c) => c.parents.length > 1) ?? []
  async function submit() {
    if (!review) return
    setBusy(true)
    setError(null)
    try {
      const ordered = review.commits.map((c) => c.hash)
      if (operation === "cherry-pick") ordered.reverse()
      const r = await gitHistoryAdvanced(path, {
        operation,
        commits: ordered,
        expectedHead: review.head,
        allowPublished: published,
        messages,
        files: [...files],
        secondMessage: second,
        mainline: merges.length ? mainline : undefined,
      })
      onCompleted()
      if (r.conflict) {
        onConflict(r.conflict)
        onClose()
      } else if (rewrite) setResult(r.recovery_ref ?? r.head ?? "")
      else onClose()
    } catch (e) {
      setError(toErrorMessage(e))
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
          <DialogTitle>{t("title")}</DialogTitle>
          <DialogDescription>{t("description")}</DialogDescription>
        </DialogHeader>
        {!review && !error && <p role="status">{t("loading")}</p>}
        {review && (
          <>
            <select
              aria-label={t("operation")}
              className="rounded border bg-background p-2"
              disabled={busy || !!result}
              value={operation}
              onChange={(e) => {
                setOperation(
                  e.target.value as GitHistoryAdvancedRequest["operation"]
                )
                setConfirmed(false)
              }}
            >
              {(
                ["cherry-pick", "revert", "reword-merges", "split"] as const
              ).map((op) => (
                <option
                  key={op}
                  value={op}
                  disabled={
                    op === "split" &&
                    (review.commits.length !== 1 ||
                      review.commits[0].parents.length !== 1)
                  }
                >
                  {t(op)}
                </option>
              ))}
            </select>
            {!rewrite && merges.length > 0 && (
              <label className="text-sm">
                {t("mainline")}
                <Input
                  type="number"
                  min={1}
                  max={
                    review.commits.some((c) => c.parents.length <= 1)
                      ? 1
                      : Math.min(...merges.map((c) => c.parents.length))
                  }
                  value={mainline}
                  disabled={busy}
                  onChange={(e) => setMainline(Number(e.target.value))}
                />
                {merges.map((c) => (
                  <p key={c.hash} className="break-all font-mono text-xs">
                    {c.hash.slice(0, 8)}:{" "}
                    {c.parents
                      .map((p, i) => `${i + 1}=${p.slice(0, 8)}`)
                      .join(" · ")}
                  </p>
                ))}
              </label>
            )}
            <div className="min-h-0 space-y-2 overflow-auto">
              {review.commits.map((c) => (
                <div key={c.hash} className="rounded border p-2 text-xs">
                  <code>{c.hash.slice(0, 12)}</code>
                  {rewrite ? (
                    <Textarea
                      aria-label={t("message", { hash: c.hash.slice(0, 8) })}
                      value={messages[c.hash] ?? ""}
                      disabled={busy || !!result}
                      onChange={(e) =>
                        setMessages({ ...messages, [c.hash]: e.target.value })
                      }
                    />
                  ) : (
                    <p>{c.message.split("\n")[0]}</p>
                  )}
                </div>
              ))}
              {operation === "split" && (
                <>
                  <p className="text-sm">{t("splitHint")}</p>
                  {review.commits[0].files.map((file) => (
                    <label key={file} className="flex gap-2 text-xs">
                      <input
                        type="checkbox"
                        checked={files.has(file)}
                        disabled={busy || !!result}
                        onChange={(e) => {
                          const next = new Set(files)
                          if (e.target.checked) next.add(file)
                          else next.delete(file)
                          setFiles(next)
                        }}
                      />
                      {file}
                    </label>
                  ))}
                  <Textarea
                    aria-label={t("secondMessage")}
                    placeholder={t("secondMessage")}
                    value={second}
                    disabled={busy || !!result}
                    onChange={(e) => setSecond(e.target.value)}
                  />
                </>
              )}
            </div>
            {rewrite && (
              <label className="flex gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={confirmed}
                  disabled={busy || !!result}
                  onChange={(e) => setConfirmed(e.target.checked)}
                />
                {t("rewriteWarning")}
              </label>
            )}
            {rewrite && review.published && (
              <label className="flex gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={published}
                  disabled={busy || !!result}
                  onChange={(e) => setPublished(e.target.checked)}
                />
                {t("published")}
              </label>
            )}
            {!rewrite && (
              <p className="text-xs text-muted-foreground">{t("order")}</p>
            )}
            <Button
              disabled={
                busy ||
                !!result ||
                (rewrite && (!confirmed || (review.published && !published))) ||
                (operation === "split" && (!files.size || !second.trim()))
              }
              onClick={() => void submit()}
            >
              {t("apply")}
            </Button>
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
        {result && (
          <p role="status" className="break-all text-sm">
            {t("completed")} {result}
          </p>
        )}
      </DialogContent>
    </Dialog>
  )
}
