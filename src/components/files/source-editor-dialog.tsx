"use client"
import { useEffect, useState } from "react"
import dynamic from "next/dynamic"
import type { Monaco } from "@monaco-editor/react"
import type { editor as Editor } from "monaco-editor"
import { useTranslations } from "next-intl"
import { readFileForEdit, saveFileContent, gitSourceLine } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import { languageFromPath } from "@/lib/language-detect"
import { GitEditorTrace } from "./git-editor-trace"
import { Button } from "@/components/ui/button"
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
const CodeEditor = dynamic(() => import("@monaco-editor/react"), { ssr: false })
export function useSourceEditor(path: string) {
  const [target, setTarget] = useState<{ file: string; line: number } | null>(
    null
  )
  const [error, setError] = useState<string | null>(null)
  async function open(file: string, line: number, revision?: string) {
    try {
      const mapped = revision
        ? await gitSourceLine(path, file, revision, line)
        : line
      setError(null)
      setTarget({ file, line: mapped })
    } catch (err) {
      setError(toErrorMessage(err))
    }
  }
  return {
    open,
    view: (
      <>
        {error && (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        )}
        {target && (
          <SourceEditorDialog
            key={`${path}:${target.file}:${target.line}`}
            path={path}
            file={target.file}
            line={target.line}
            onClose={() => setTarget(null)}
          />
        )}
      </>
    ),
  }
}
export function SourceEditorDialog({
  path,
  file,
  line,
  onClose,
}: {
  path: string
  file: string
  line: number
  onClose: () => void
}) {
  const t = useTranslations("Folder.gitRevision")
  const [content, setContent] = useState("")
  const [saved, setSaved] = useState("")
  const [etag, setEtag] = useState<string | null>(null)
  const [readonly, setReadonly] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [editor, setEditor] = useState<Editor.IStandaloneCodeEditor | null>(
    null
  )
  const [monaco, setMonaco] = useState<Monaco | null>(null)
  const [discard, setDiscard] = useState(false)
  const editorTheme = useMonacoWorkspaceTheme(monaco)
  useEffect(() => {
    let active = true
    readFileForEdit(path, file)
      .then((result) => {
        if (active) {
          setContent(result.content)
          setSaved(result.content)
          setEtag(result.etag)
          setReadonly(result.readonly)
        }
      })
      .catch((err) => {
        if (active) setError(toErrorMessage(err))
      })
    return () => {
      active = false
    }
  }, [path, file])
  const dirty = content !== saved
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) {
          if (dirty) setDiscard(true)
          else onClose()
        }
      }}
    >
      <DialogContent className="flex h-[90vh] flex-col sm:max-w-6xl">
        <DialogHeader>
          <DialogTitle>{t("source")}</DialogTitle>
          <DialogDescription>
            {file}:{line}
          </DialogDescription>
        </DialogHeader>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <GitEditorTrace
          editor={editor}
          monaco={monaco}
          path={path}
          file={file}
          content={content}
          dirty={dirty}
        />
        <div className="min-h-0 flex-1">
          {etag && (
            <CodeEditor
              theme={editorTheme}
              value={content}
              onChange={(value) => setContent(value ?? "")}
              language={languageFromPath(file)}
              onMount={(instance, api) => {
                setEditor(instance)
                setMonaco(api)
                instance.setPosition({
                  lineNumber: Math.max(
                    1,
                    Math.min(line, instance.getModel()?.getLineCount() ?? 1)
                  ),
                  column: 1,
                })
                instance.revealLineInCenter(line)
                instance.focus()
              }}
              options={{
                unicodeHighlight: MONACO_UNICODE_HIGHLIGHT_OPTIONS,
                readOnly: readonly || busy,
                automaticLayout: true,
                minimap: { enabled: false },
              }}
            />
          )}
        </div>
        <Button
          disabled={busy || readonly || !dirty}
          onClick={() => {
            setBusy(true)
            setError(null)
            void saveFileContent(path, file, content, etag)
              .then((result) => {
                setEtag(result.etag)
                setSaved(content)
              })
              .catch((err) => setError(toErrorMessage(err)))
              .finally(() => setBusy(false))
          }}
        >
          {t("save")}
        </Button>
        {discard && (
          <div className="flex items-center gap-2 border-t p-2 text-sm">
            <span>{t("discardHint")}</span>
            <Button variant="destructive" size="sm" onClick={onClose}>
              {t("discard")}
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setDiscard(false)}
            >
              {t("cancel")}
            </Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
