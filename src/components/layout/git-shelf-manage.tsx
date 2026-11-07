"use client"
import { useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import { gitCommitTools } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog"
export function GitShelfManage({
  path,
  reference,
  name: initialName,
  group: initialGroup,
  onClose,
  onChanged,
}: {
  path: string
  reference: string
  name: string
  group: string
  onClose: () => void
  onChanged: () => void
}) {
  const t = useTranslations("Folder.gitExtras")
  const [name, setName] = useState(initialName),
    [group, setGroup] = useState(initialGroup),
    [files, setFiles] = useState<string[]>([]),
    [selected, setSelected] = useState<Set<string>>(new Set()),
    [error, setError] = useState<string | null>(null),
    [busy, setBusy] = useState(false),
    [confirm, setConfirm] = useState(false)
  useEffect(() => {
    let active = true
    gitCommitTools(path, "shelf-paths", { shelfRef: reference })
      .then((files) => {
        if (active) setFiles(files)
      })
      .catch((e) => {
        if (active) setError(toErrorMessage(e))
      })
    return () => {
      active = false
    }
  }, [path, reference])
  async function run(restore: boolean) {
    setBusy(true)
    setError(null)
    try {
      if (restore)
        await gitCommitTools(path, "shelf-restore", {
          shelfRef: reference,
          files: [...selected],
        })
      else
        await gitCommitTools(path, "shelf-meta", {
          shelfRef: reference,
          name,
          group,
        })
      onChanged()
      onClose()
    } catch (e) {
      setError(toErrorMessage(e))
    } finally {
      setBusy(false)
      setConfirm(false)
    }
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("manageShelf")}</DialogTitle>
          <DialogDescription>{t("partialHint")}</DialogDescription>
        </DialogHeader>
        <Input
          aria-label={t("shelfName")}
          value={name}
          onChange={(e) => setName(e.target.value)}
          disabled={busy}
        />
        <Input
          aria-label={t("shelfGroup")}
          value={group}
          onChange={(e) => setGroup(e.target.value)}
          disabled={busy}
        />
        <Button disabled={busy || !name.trim()} onClick={() => void run(false)}>
          {t("saveMetadata")}
        </Button>
        <div className="max-h-64 overflow-auto">
          {files.map((file) => (
            <label key={file} className="flex gap-2 py-1 text-sm">
              <input
                type="checkbox"
                checked={selected.has(file)}
                disabled={busy}
                onChange={(e) => {
                  const next = new Set(selected)
                  if (e.target.checked) next.add(file)
                  else next.delete(file)
                  setSelected(next)
                  setConfirm(false)
                }}
              />
              {file}
            </label>
          ))}
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <Button
          disabled={busy || !selected.size}
          variant={confirm ? "destructive" : "outline"}
          onClick={() => {
            if (confirm) void run(true)
            else setConfirm(true)
          }}
        >
          {t(confirm ? "confirmPartial" : "restoreSelected")}
        </Button>
      </DialogContent>
    </Dialog>
  )
}
