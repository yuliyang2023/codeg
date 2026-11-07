"use client"
import { useEffect, useState } from "react"
import { useTranslations } from "next-intl"
import { Input } from "@/components/ui/input"
import { gitCommitTools, type GitCommitOptions } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
export function GitCommitOptionsPanel({
  path,
  value,
  onChange,
  onMessage,
}: {
  path: string
  value: GitCommitOptions
  onChange: (value: GitCommitOptions) => void
  onMessage: (value: string) => void
}) {
  const t = useTranslations("Folder.gitExtras")
  const [messages, setMessages] = useState<string[]>([])
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let active = true
    gitCommitTools(path, "state")
      .then((s) => {
        if (active) setMessages(s.messages)
      })
      .catch((e) => {
        if (active) setError(toErrorMessage(e))
      })
    return () => {
      active = false
    }
  }, [path])
  return (
    <details className="mb-2 rounded border p-2 text-xs">
      <summary>{t("commitOptions")}</summary>
      <div className="mt-2 space-y-2">
        {error && <p role="alert">{error}</p>}
        <select
          aria-label={t("messageHistory")}
          className="w-full rounded border bg-background p-1"
          value=""
          onChange={(e) => onMessage(e.target.value)}
        >
          <option value="" disabled>
            {t("messageHistory")}
          </option>
          {messages.map((m, i) => (
            <option key={i} value={m}>
              {m.split("\n")[0]}
            </option>
          ))}
        </select>
        <Input
          aria-label={t("author")}
          placeholder="Name <email>"
          value={value.author ?? ""}
          onChange={(e) =>
            onChange({ ...value, author: e.target.value || undefined })
          }
        />
        <label className="flex gap-2">
          <input
            type="checkbox"
            checked={value.signoff ?? false}
            onChange={(e) => onChange({ ...value, signoff: e.target.checked })}
          />
          {t("signoff")}
        </label>
        <select
          aria-label={t("signing")}
          className="w-full rounded border bg-background p-1"
          value={value.signing ?? "default"}
          onChange={(e) =>
            onChange({
              ...value,
              signing: e.target.value as GitCommitOptions["signing"],
            })
          }
        >
          {(["default", "signed", "unsigned"] as const).map((s) => (
            <option key={s} value={s}>
              {t(s)}
            </option>
          ))}
        </select>
        {value.signing === "signed" && (
          <Input
            aria-label={t("signingKey")}
            placeholder={t("signingKey")}
            value={value.signingKey ?? ""}
            onChange={(e) => onChange({ ...value, signingKey: e.target.value })}
          />
        )}
        <label className="flex gap-2">
          <input
            type="checkbox"
            checked={value.skipHooks ?? false}
            onChange={(e) =>
              onChange({ ...value, skipHooks: e.target.checked })
            }
          />
          {t("skipHooks")}
        </label>
      </div>
    </details>
  )
}
