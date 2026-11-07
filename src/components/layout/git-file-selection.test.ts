import { describe, expect, it } from "vitest"
import { selectedGitPaths } from "./git-file-selection"

describe("selected Git file paths", () => {
  it("includes both sides of a checked rename while leaving other paths alone", () => {
    expect(
      selectedGitPaths(
        [
          { status: "R ", file: "old.txt -> new.txt" },
          { status: "M", file: "other.txt" },
          { status: "??", file: "literal -> name.txt" },
        ],
        new Set(["old.txt -> new.txt", "literal -> name.txt"])
      )
    ).toEqual(["old.txt", "new.txt", "literal -> name.txt"])
  })
})
