import type { editor } from "monaco-editor"

/** Map an old-side line to the nearest surviving line on the new side. */
export function diffSourceLine(
  line: number,
  side: "original" | "modified",
  changes: readonly Pick<
    editor.ILineChange,
    | "originalStartLineNumber"
    | "originalEndLineNumber"
    | "modifiedStartLineNumber"
    | "modifiedEndLineNumber"
  >[],
  modifiedLineCount: number
): number {
  const clamp = (value: number) =>
    Math.max(1, Math.min(value, modifiedLineCount))
  if (side === "modified") return clamp(line)
  let offset = 0
  for (const change of changes) {
    const oldCount =
      change.originalEndLineNumber === 0
        ? 0
        : change.originalEndLineNumber - change.originalStartLineNumber + 1
    const newCount =
      change.modifiedEndLineNumber === 0
        ? 0
        : change.modifiedEndLineNumber - change.modifiedStartLineNumber + 1
    if (oldCount === 0) {
      if (line <= change.originalStartLineNumber) break
    } else {
      if (line < change.originalStartLineNumber) break
      if (line <= change.originalEndLineNumber) {
        return clamp(
          newCount === 0
            ? change.modifiedStartLineNumber + 1
            : change.modifiedStartLineNumber +
                Math.min(line - change.originalStartLineNumber, newCount - 1)
        )
      }
    }
    offset += newCount - oldCount
  }
  return clamp(line + offset)
}
