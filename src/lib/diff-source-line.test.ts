import { describe, expect, it } from "vitest"
import { diffSourceLine } from "./diff-source-line"

describe("diff source line mapping", () => {
  const changes = [
    {
      originalStartLineNumber: 2,
      originalEndLineNumber: 0,
      modifiedStartLineNumber: 3,
      modifiedEndLineNumber: 5,
    },
    {
      originalStartLineNumber: 10,
      originalEndLineNumber: 12,
      modifiedStartLineNumber: 13,
      modifiedEndLineNumber: 14,
    },
    {
      originalStartLineNumber: 20,
      originalEndLineNumber: 22,
      modifiedStartLineNumber: 21,
      modifiedEndLineNumber: 0,
    },
  ]
  it.each([
    [1, 1],
    [2, 2],
    [3, 6],
    [10, 13],
    [11, 14],
    [12, 14],
    [13, 15],
    [20, 22],
    [22, 22],
    [23, 22],
  ])("maps old line %i to current line %i", (line, expected) => {
    expect(diffSourceLine(line, "original", changes, 100)).toBe(expected)
  })
  it("clamps to an existing line, including an empty file", () => {
    expect(diffSourceLine(40, "original", [], 5)).toBe(5)
    expect(diffSourceLine(0, "modified", [], 100)).toBe(1)
    expect(diffSourceLine(10, "original", changes, 1)).toBe(1)
    expect(diffSourceLine(12, "modified", changes, 100)).toBe(12)
  })
})
