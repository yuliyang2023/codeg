"use client"

import { useState } from "react"
import { useTranslations } from "next-intl"
import { Filter, Search } from "lucide-react"
import type { GitHistoryOptions } from "@/lib/types"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Checkbox } from "@/components/ui/checkbox"

export const DEFAULT_HISTORY_OPTIONS: GitHistoryOptions = {
  search: "",
  regex: false,
  caseSensitive: false,
  since: "",
  until: "",
  file: "",
  order: "topo",
  firstParent: false,
  noMerges: false,
  revision: "",
}

export function GitHistoryFilters({
  value,
  onChange,
}: {
  value: GitHistoryOptions
  onChange: (value: GitHistoryOptions) => void
}) {
  const t = useTranslations("Folder.gitHistory")
  const [expanded, setExpanded] = useState(false)
  const [draft, setDraft] = useState(value)
  const active = Object.keys(DEFAULT_HISTORY_OPTIONS).some(
    (key) =>
      value[key as keyof GitHistoryOptions] !==
      DEFAULT_HISTORY_OPTIONS[key as keyof GitHistoryOptions]
  )
  return (
    <div className="shrink-0 border-b border-border/50 p-2">
      <div className="flex items-center gap-1">
        <Button
          size="icon-xs"
          variant={active ? "secondary" : "ghost"}
          aria-label={t("filters")}
          onClick={() => setExpanded(!expanded)}
        >
          <Filter className="size-3.5" />
        </Button>
        <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
          {active
            ? [
                value.search,
                value.revision,
                value.file,
                value.since,
                value.until,
              ]
                .filter(Boolean)
                .join(" · ") || t("filtered")
            : t("allHistory")}
        </span>
        {active && (
          <Button
            variant="ghost"
            size="xs"
            onClick={() => {
              setDraft(DEFAULT_HISTORY_OPTIONS)
              onChange(DEFAULT_HISTORY_OPTIONS)
            }}
          >
            {t("clear")}
          </Button>
        )}
      </div>
      {expanded && (
        <form
          className="mt-2 space-y-2"
          onSubmit={(event) => {
            event.preventDefault()
            onChange(draft)
          }}
        >
          <Input
            aria-label={t("search")}
            placeholder={t("search")}
            value={draft.search}
            onChange={(e) => setDraft({ ...draft, search: e.target.value })}
          />
          <div className="flex flex-wrap gap-3 text-xs">
            {(
              ["regex", "caseSensitive", "firstParent", "noMerges"] as const
            ).map((key) => (
              <label key={key} className="flex items-center gap-1">
                <Checkbox
                  checked={draft[key]}
                  onCheckedChange={(v) => setDraft({ ...draft, [key]: !!v })}
                />
                {t(key)}
              </label>
            ))}
          </div>
          <Input
            aria-label={t("revision")}
            placeholder={t("revision")}
            value={draft.revision}
            onChange={(e) => setDraft({ ...draft, revision: e.target.value })}
          />
          <Input
            aria-label={t("file")}
            placeholder={t("file")}
            value={draft.file}
            onChange={(e) => setDraft({ ...draft, file: e.target.value })}
          />
          <div className="grid grid-cols-2 gap-2">
            {(["since", "until"] as const).map((key) => (
              <label
                key={key}
                className="min-w-0 text-xs text-muted-foreground"
              >
                {t(key)}
                <Input
                  type="date"
                  aria-label={t(key)}
                  value={draft[key]}
                  onChange={(e) =>
                    setDraft({ ...draft, [key]: e.target.value })
                  }
                />
              </label>
            ))}
          </div>
          <label className="flex items-center justify-between gap-2 text-xs">
            {t("order")}
            <select
              className="min-w-0 rounded border bg-background p-1"
              aria-label={t("order")}
              value={draft.order}
              onChange={(e) =>
                setDraft({ ...draft, order: e.target.value as "topo" | "date" })
              }
            >
              <option value="topo">{t("topo")}</option>
              <option value="date">{t("date")}</option>
            </select>
          </label>
          <Button type="submit" size="xs" className="w-full">
            <Search className="size-3" />
            {t("apply")}
          </Button>
        </form>
      )}
    </div>
  )
}

export function loadHistoryOptions(path: string | null): GitHistoryOptions {
  if (!path || typeof window === "undefined") return DEFAULT_HISTORY_OPTIONS
  try {
    const value = JSON.parse(
      localStorage.getItem("codeg:gitlog:options:" + path) ?? "null"
    )
    if (!value || typeof value !== "object") return DEFAULT_HISTORY_OPTIONS
    const result = { ...DEFAULT_HISTORY_OPTIONS }
    for (const key of Object.keys(result) as (keyof GitHistoryOptions)[]) {
      if (typeof value[key] === typeof result[key])
        Object.assign(result, { [key]: value[key] })
    }
    if (result.order !== "topo" && result.order !== "date")
      result.order = "topo"
    return result
  } catch {
    return DEFAULT_HISTORY_OPTIONS
  }
}

export function saveHistoryOptions(
  path: string | null,
  value: GitHistoryOptions
) {
  if (!path) return
  try {
    localStorage.setItem("codeg:gitlog:options:" + path, JSON.stringify(value))
  } catch {
    /* Best effort. */
  }
}
