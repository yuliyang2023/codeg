import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import messages from "@/i18n/messages/en.json"
import {
  DEFAULT_HISTORY_OPTIONS,
  GitHistoryFilters,
  loadHistoryOptions,
  saveHistoryOptions,
} from "./git-history-filters"

afterEach(cleanup)
beforeEach(() => localStorage.clear())
describe("history filter drafts", () => {
  it("only applies an explicit submission and clears applied options", () => {
    const onChange = vi.fn()
    const value = { ...DEFAULT_HISTORY_OPTIONS, search: "old" }
    render(
      <NextIntlClientProvider locale="en" messages={messages}>
        <GitHistoryFilters value={value} onChange={onChange} />
      </NextIntlClientProvider>
    )
    fireEvent.click(screen.getByRole("button", { name: "History filters" }))
    fireEvent.change(screen.getByPlaceholderText("Commit message"), {
      target: { value: "new" },
    })
    expect(onChange).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole("button", { name: "Apply filters" }))
    expect(onChange).toHaveBeenLastCalledWith({ ...value, search: "new" })
    fireEvent.click(screen.getByRole("button", { name: "Clear" }))
    expect(onChange).toHaveBeenLastCalledWith(DEFAULT_HISTORY_OPTIONS)
  })
  it("isolates repositories and ignores malformed saved values", () => {
    saveHistoryOptions("a", { ...DEFAULT_HISTORY_OPTIONS, search: "specific" })
    expect(loadHistoryOptions("a").search).toBe("specific")
    expect(loadHistoryOptions("b")).toEqual(DEFAULT_HISTORY_OPTIONS)
    localStorage.setItem(
      "codeg:gitlog:options:b",
      JSON.stringify({ search: 12, order: "invalid", regex: true })
    )
    expect(loadHistoryOptions("b")).toEqual({
      ...DEFAULT_HISTORY_OPTIONS,
      regex: true,
    })
  })
})
