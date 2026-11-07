"use client"

import { useTranslations } from "next-intl"
import { Checkbox } from "@/components/ui/checkbox"
import { ScrollArea } from "@/components/ui/scroll-area"
import type { GitStatusEntry } from "@/lib/types"

export function selectedGitPaths(
  entries: GitStatusEntry[],
  selected: ReadonlySet<string>
): string[] {
  return [
    ...new Set(
      entries
        .filter((entry) => selected.has(entry.file))
        .flatMap((entry) => {
          const separator = entry.file.lastIndexOf(" -> ")
          return /[RC]/.test(entry.status) && separator >= 0
            ? [entry.file.slice(0, separator), entry.file.slice(separator + 4)]
            : [entry.file]
        })
    ),
  ]
}

export function GitFileSelection({
  entries,
  selected,
  onChange,
  disabled = false,
  onDragFile,
}: {
  entries: GitStatusEntry[]
  selected: ReadonlySet<string>
  onChange: (files: Set<string>) => void
  disabled?: boolean
  onDragFile?: (file: string, event: React.DragEvent<HTMLLabelElement>) => void
}) {
  const t = useTranslations("Folder.gitWorkbench")
  const all = entries.length > 0 && selected.size === entries.length
  return (
    <div className="space-y-2">
      <label className="flex items-center gap-2 text-sm">
        <Checkbox
          checked={all ? true : selected.size ? "indeterminate" : false}
          disabled={disabled || entries.length === 0}
          onCheckedChange={(checked) =>
            onChange(new Set(checked ? entries.map((entry) => entry.file) : []))
          }
        />
        {t("selectedFiles", { selected: selected.size, total: entries.length })}
      </label>
      <ScrollArea className="max-h-56 rounded-md border">
        <div className="space-y-1 p-2">
          {entries.length === 0 ? (
            <p className="text-xs text-muted-foreground">
              {t("noChangedFiles")}
            </p>
          ) : (
            entries.map((entry) => (
              <label
                key={entry.file}
                draggable={!!onDragFile && !disabled}
                onDragStart={(event) => onDragFile?.(entry.file, event)}
                className="flex min-w-0 items-center gap-2 py-1 text-xs"
              >
                <Checkbox
                  checked={selected.has(entry.file)}
                  disabled={disabled}
                  aria-label={entry.file}
                  onCheckedChange={(checked) => {
                    const next = new Set(selected)
                    if (checked) next.add(entry.file)
                    else next.delete(entry.file)
                    onChange(next)
                  }}
                />
                <span className="shrink-0 font-mono text-muted-foreground">
                  {entry.status}
                </span>
                <span className="truncate" title={entry.file}>
                  {entry.file}
                </span>
              </label>
            ))
          )}
        </div>
      </ScrollArea>
    </div>
  )
}
