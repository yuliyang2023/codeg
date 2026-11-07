"use client"

import { Columns2, Rows3 } from "lucide-react"
import { useTranslations } from "next-intl"
import type { DiffViewMode } from "@/lib/diff-view-mode-prefs"
import { cn } from "@/lib/utils"

/**
 * One button for both layouts, showing the view it switches TO rather than the
 * one already on screen — the icon reads as "go here", which is also what the
 * `viewMode` labels say ("Switch to …"). A pair of buttons spent twice the
 * header's width to say the same thing.
 *
 * Exported for hosts that place it themselves (see `hideViewToggle`). Its
 * chrome is a default rather than a fixture: a host that puts it in a row of
 * its own icon buttons overrides `className`/`iconClassName` so the pair reads
 * as one control set, and the *decision* — which mode is next, which glyph and
 * which label say so — stays in one place regardless.
 */
export function ViewModeToggle({
  view,
  onSwitch,
  className,
  iconClassName,
}: {
  view: DiffViewMode
  onSwitch: (mode: DiffViewMode) => void
  className?: string
  iconClassName?: string
}) {
  const t = useTranslations("Folder.diffPreview")
  const next: DiffViewMode = view === "split" ? "unified" : "split"
  const label = t(`viewMode.${next}`)
  const Icon = next === "split" ? Columns2 : Rows3
  return (
    <button
      type="button"
      onClick={() => onSwitch(next)}
      aria-label={label}
      title={label}
      className={cn(
        "inline-flex h-5 w-5 shrink-0 items-center justify-center rounded border border-border bg-background text-muted-foreground transition-colors hover:bg-muted hover:text-foreground",
        className
      )}
    >
      <Icon className={cn("h-3 w-3", iconClassName)} />
    </button>
  )
}
