"use client"

import { useSyncExternalStore } from "react"

const KEY = "workspace:diff-collapse-unchanged"
const EVENT = "codeg:diff-collapse-unchanged-changed"
let fallback = false
function snapshot(): boolean {
  try {
    const value = localStorage.getItem(KEY)
    return value === null ? fallback : value === "true"
  } catch {
    return fallback
  }
}
function subscribe(notify: () => void) {
  const storage = (event: StorageEvent) => {
    if (event.key === KEY || event.key === null) notify()
  }
  window.addEventListener(EVENT, notify)
  window.addEventListener("storage", storage)
  return () => {
    window.removeEventListener(EVENT, notify)
    window.removeEventListener("storage", storage)
  }
}
function save(value: boolean) {
  fallback = value
  try {
    localStorage.setItem(KEY, String(value))
  } catch {
    // Keep the choice usable for this window when storage is unavailable.
  }
  window.dispatchEvent(new Event(EVENT))
}
export function useDiffCollapse(): [boolean, (value: boolean) => void] {
  return [useSyncExternalStore(subscribe, snapshot, () => false), save]
}
