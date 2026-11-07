"use client"
import { useEffect, useRef, useState } from "react"
import dynamic from "next/dynamic"
import type { Monaco } from "@monaco-editor/react"
import { useTranslations } from "next-intl"
import {
  gitRevisionInfo,
  gitRevisionTree,
  gitRevisionFile,
  gitRevisionCompare,
  gitRevisionCheckout,
  gitCompareFile,
} from "@/lib/api"
import type {
  GitRevisionInfo,
  GitRevisionTreeEntry,
  GitRevisionFile,
  GitRevisionComparison,
  GitFileComparison,
} from "@/lib/types"
import { toErrorMessage } from "@/lib/app-error"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { DiffViewer } from "@/components/diff/diff-viewer"
import { languageFromPath } from "@/lib/language-detect"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  useMonacoWorkspaceTheme,
  MONACO_UNICODE_HIGHLIGHT_OPTIONS,
} from "@/lib/monaco-themes"
import "@/lib/monaco-local"
const Editor = dynamic(() => import("@monaco-editor/react"), { ssr: false })
export function GitRevisionBrowser({
  path,
  initialRevision = "HEAD",
  onClose,
  onChanged,
}: {
  path: string
  initialRevision?: string
  onClose: () => void
  onChanged?: () => void
}) {
  const [monaco, setMonaco] = useState<Monaco | null>(null)
  const editorTheme = useMonacoWorkspaceTheme(monaco)
  const t = useTranslations("Folder.gitRevision")
  const [draft, setDraft] = useState(initialRevision)
  const [revision, setRevision] = useState(initialRevision)
  const [info, setInfo] = useState<GitRevisionInfo | null>(null)
  const [directory, setDirectory] = useState("")
  const [tree, setTree] = useState<GitRevisionTreeEntry[]>([])
  const [file, setFile] = useState<GitRevisionFile | null>(null)
  const [base, setBase] = useState("HEAD")
  const [comparison, setComparison] = useState<GitRevisionComparison | null>(
    null
  )
  const [diff, setDiff] = useState<GitFileComparison | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [confirm, setConfirm] = useState<{
    revision: string
    head: string
  } | null>(null)
  const seq = useRef(0)
  useEffect(() => {
    let active = true
    seq.current++
    gitRevisionInfo(path, revision)
      .then((result) => {
        if (active) {
          setInfo(result)
          setError(null)
          setFile(null)
          setComparison(null)
          setDiff(null)
          setDirectory("")
        }
      })
      .catch((err) => {
        if (active) {
          setInfo(null)
          setError(toErrorMessage(err))
        }
      })
    return () => {
      active = false
    }
  }, [path, revision])
  useEffect(() => {
    let active = true
    if (info)
      gitRevisionTree(path, info.hash, directory)
        .then((entries) => {
          if (active) setTree(entries)
        })
        .catch((err) => {
          if (active) setError(toErrorMessage(err))
        })
    return () => {
      active = false
    }
  }, [path, info, directory])
  function navigate(hash: string) {
    if (hash === revision) return
    seq.current++
    setInfo(null)
    setTree([])
    setDirectory("")
    setRevision(hash)
    setDraft(hash)
    setFile(null)
    setDiff(null)
    setComparison(null)
  }
  async function openFile(entry: GitRevisionTreeEntry) {
    if (!info) return
    if (entry.kind === "tree") {
      seq.current++
      setTree([])
      setDirectory(entry.file)
      setFile(null)
      return
    }
    if (entry.kind !== "blob") return
    const current = ++seq.current
    setError(null)
    try {
      const result = await gitRevisionFile(path, info.hash, entry.file)
      if (current === seq.current) setFile(result)
    } catch (err) {
      if (current === seq.current) setError(toErrorMessage(err))
    }
  }
  async function compare() {
    if (!info) return
    const current = ++seq.current
    setBusy(true)
    setError(null)
    try {
      const result = await gitRevisionCompare(path, base, info.hash)
      if (current === seq.current) {
        setComparison(result)
        setDiff(null)
        setFile(null)
      }
    } catch (err) {
      if (current === seq.current) setError(toErrorMessage(err))
    } finally {
      setBusy(false)
    }
  }
  async function compareFile(name: string) {
    if (!comparison) return
    const current = ++seq.current
    setDiff(null)
    setError(null)
    try {
      const result = await gitCompareFile(
        path,
        name,
        comparison.base,
        comparison.target
      )
      if (current === seq.current) setDiff(result)
    } catch (err) {
      if (current === seq.current) setError(toErrorMessage(err))
    }
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose()
      }}
    >
      <DialogContent className="flex h-[90vh] flex-col sm:max-w-7xl">
        <DialogHeader>
          <DialogTitle>{t("title")}</DialogTitle>
          <DialogDescription>
            {info?.message.split("\n")[0] ?? revision}
          </DialogDescription>
        </DialogHeader>
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault()
            navigate(draft)
          }}
        >
          <Input
            aria-label={t("revision")}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
          <Button size="sm" type="submit">
            {t("open")}
          </Button>
          <Button
            size="sm"
            type="button"
            variant="outline"
            disabled={!info || busy}
            onClick={() => {
              setBusy(true)
              void gitRevisionInfo(path, "HEAD")
                .then((head) => {
                  if (info) setConfirm({ revision: info.hash, head: head.hash })
                })
                .catch((err) => setError(toErrorMessage(err)))
                .finally(() => setBusy(false))
            }}
          >
            {t("checkout")}
          </Button>
        </form>
        {info && (
          <>
            <div className="flex flex-wrap items-center gap-2 text-xs">
              <code>{info.hash}</code>
              <span>
                {info.author} · {new Date(info.date).toLocaleString()}
              </span>
            </div>
            <div className="flex flex-wrap gap-1 text-xs">
              <span>{t("parents")}</span>
              {info.parents.map((hash) => (
                <Button
                  key={hash}
                  size="xs"
                  variant="outline"
                  onClick={() => navigate(hash)}
                >
                  {hash.slice(0, 8)}
                </Button>
              ))}
              <span>{t("children")}</span>
              {info.children.map((hash) => (
                <Button
                  key={hash}
                  size="xs"
                  variant="outline"
                  onClick={() => navigate(hash)}
                >
                  {hash.slice(0, 8)}
                </Button>
              ))}
            </div>
            <div className="flex gap-2">
              <Input
                aria-label={t("compareBase")}
                value={base}
                onChange={(e) => setBase(e.target.value)}
              />
              <Button
                size="sm"
                disabled={busy || !base}
                onClick={() => void compare()}
              >
                {t("compare")}
              </Button>
              {comparison && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => {
                    seq.current++
                    setComparison(null)
                    setDiff(null)
                  }}
                >
                  {t("tree")}
                </Button>
              )}
            </div>
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
        <div className="flex min-h-0 flex-1 gap-2">
          <div className="w-64 shrink-0 overflow-auto rounded border p-2 text-xs">
            {comparison ? (
              <>
                <p className="mb-2">
                  {comparison.base.slice(0, 8)} →{" "}
                  {comparison.target.slice(0, 8)} · {comparison.files.length}
                </p>
                {comparison.files.map((change) => (
                  <button
                    key={change.file}
                    className="block w-full break-all py-1 text-left hover:underline"
                    onClick={() => void compareFile(change.file)}
                  >
                    {change.status} {change.file}
                  </button>
                ))}
              </>
            ) : (
              <>
                <p className="break-all">/{directory}</p>
                {directory && (
                  <Button
                    size="xs"
                    variant="ghost"
                    onClick={() => {
                      seq.current++
                      setFile(null)
                      setTree([])
                      setDirectory(directory.split("/").slice(0, -1).join("/"))
                    }}
                  >
                    ..
                  </Button>
                )}
                {tree.map((entry) => (
                  <button
                    key={entry.file}
                    className="block w-full break-all py-1 text-left hover:underline"
                    disabled={entry.kind === "commit"}
                    onClick={() => void openFile(entry)}
                  >
                    {entry.kind === "tree" ? "▸ " : ""}
                    {entry.name}
                    {entry.kind === "commit" ? " (submodule)" : ""}
                  </button>
                ))}
              </>
            )}
          </div>
          <div className="flex min-w-0 flex-1 flex-col rounded border">
            {diff ? (
              <DiffViewer
                original={diff.original}
                modified={diff.modified}
                originalLabel={diff.original_label.slice(0, 12)}
                modifiedLabel={diff.modified_label.slice(0, 12)}
              />
            ) : file ? (
              <>
                <p className="border-b p-2 text-xs">
                  {file.file} · {file.size} B · {t("readOnly")}
                </p>
                {file.binary ? (
                  <p className="p-2 text-sm">{t("binary")}</p>
                ) : (
                  <Editor
                    onMount={(_editor, api) => setMonaco(api)}
                    theme={editorTheme}
                    key={`${file.hash}:${file.file}`}
                    value={file.content ?? ""}
                    language={languageFromPath(file.file)}
                    options={{
                      unicodeHighlight: MONACO_UNICODE_HIGHLIGHT_OPTIONS,
                      readOnly: true,
                      automaticLayout: true,
                      minimap: { enabled: false },
                    }}
                  />
                )}
              </>
            ) : (
              <p className="p-3 text-xs text-muted-foreground">
                {t("chooseFile")}
              </p>
            )}
          </div>
        </div>
        <Dialog
          open={!!confirm}
          onOpenChange={(open) => {
            if (!open && !busy) setConfirm(null)
          }}
        >
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{t("checkout")}</DialogTitle>
              <DialogDescription>
                {t("checkoutHint", {
                  hash: confirm?.revision.slice(0, 12) ?? "",
                })}
              </DialogDescription>
            </DialogHeader>
            <Button
              disabled={busy}
              onClick={() => {
                if (!confirm) return
                setBusy(true)
                setError(null)
                void gitRevisionCheckout(path, confirm.revision, confirm.head)
                  .then(() => {
                    setConfirm(null)
                    onChanged?.()
                  })
                  .catch((err) => setError(toErrorMessage(err)))
                  .finally(() => setBusy(false))
              }}
            >
              {t("checkout")}
            </Button>
            {error && (
              <p role="alert" className="text-xs text-destructive">
                {error}
              </p>
            )}
          </DialogContent>
        </Dialog>
      </DialogContent>
    </Dialog>
  )
}
