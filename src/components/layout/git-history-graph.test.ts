import { describe, expect, it } from "vitest"
import { buildHistoryGraph } from "./git-history-graph"

describe("history graph", () => {
  const entries = [
    { full_hash: "merge", parents: ["main", "topic"] },
    { full_hash: "topic", parents: ["root"] },
    { full_hash: "main", parents: ["root"] },
    { full_hash: "root", parents: [] },
  ]
  it("forks and rejoins at the actual merge parents", () => {
    const rows = buildHistoryGraph(entries)
    expect(rows[0].after.map((l) => l.hash)).toEqual(["main", "topic"])
    expect(rows[1].lane).toBe(1)
    expect(rows[2].after.map((l) => l.hash)).toEqual(["root"])
    expect(rows[3].after).toEqual([])
  })
  it("does not invent edges across a filtered-out commit", () => {
    const rows = buildHistoryGraph([
      { full_hash: "new", parents: ["hidden"] },
      { full_hash: "old", parents: [] },
    ], true)
    expect(rows[0].parents).toEqual([])
    expect(rows[1].before.map((l) => l.hash)).toEqual(["old"])
  })
  it("keeps lane assignment when a new page follows pending parents", () => {
    const first = buildHistoryGraph(entries.slice(0, 2))
    const all = buildHistoryGraph(entries)
    expect(all.slice(0, 2)).toEqual(first)
  })
})
