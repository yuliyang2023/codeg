"use client"
import { useCallback, useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import { gitConsoleList, gitConsoleClear } from "@/lib/api"
import type { FolderDetail, GitConsoleEntry } from "@/lib/types"
import { toErrorMessage } from "@/lib/app-error"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
export function GitConsoleTab({ folder }: { folder: FolderDetail }) {
  const t = useTranslations("Folder.gitTools")
  const [entries, setEntries] = useState<GitConsoleEntry[]>([])
  const [filter, setFilter] = useState("")
  const [error, setError] = useState<string | null>(null)
  const refresh = useCallback(async () => {
    try {
      setEntries(await gitConsoleList(folder.path))
      setError(null)
    } catch (err) {
      setError(toErrorMessage(err))
    }
  }, [folder.path])
  useEffect(() => {
    let active = true
    const load = async () => {
      try {
        const items = await gitConsoleList(folder.path)
        if (active) setEntries(items)
      } catch (err) {
        if (active) setError(toErrorMessage(err))
      }
    }
    void load()
    const timer = setInterval(() => void load(), 5000)
    return () => {
      active = false
      clearInterval(timer)
    }
  }, [folder.path])
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="space-y-2 border-b p-2">
        <p className="text-xs text-muted-foreground">{t("consoleHint")}</p>
        <div className="flex gap-1">
          <Button size="xs" variant="outline" onClick={() => void refresh()}>
            {t("refresh")}
          </Button>
          <Button
            size="xs"
            variant="ghost"
            onClick={() =>
              void gitConsoleClear(folder.path)
                .then(refresh)
                .catch((err) => setError(toErrorMessage(err)))
            }
          >
            {t("clear")}
          </Button>
        </div>
        <Input
          aria-label={t("searchConsole")}
          placeholder={t("searchConsole")}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        {error && (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-auto p-2">
        {entries
          .filter((entry) =>
            (entry.command.join(" ") + " " + entry.stderr)
              .toLowerCase()
              .includes(filter.toLowerCase())
          )
          .map((entry) => (
            <details key={entry.id} className="mb-2 rounded border p-2">
              <summary className="cursor-pointer break-all font-mono text-xs">
                <span
                  className={
                    entry.exit_code === 0
                      ? "text-muted-foreground"
                      : "text-destructive"
                  }
                >
                  {entry.exit_code ?? "ERR"} · {entry.duration_ms}ms
                </span>{" "}
                {entry.command.join(" ")}
              </summary>
              <p className="my-2 break-all text-xs text-muted-foreground">
                {entry.time} · {entry.repository}
              </p>
              <pre className="overflow-auto whitespace-pre-wrap break-all text-xs">
                {entry.stdout}
                {entry.stderr && "\n" + entry.stderr}
                {entry.truncated && "\n" + t("truncated")}
              </pre>
            </details>
          ))}
      </div>
    </div>
  )
}
