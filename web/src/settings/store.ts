import { useSyncExternalStore } from 'react'
import { api } from '@/http'

export type Settings = {
  zen: boolean
  zen_density: 'comfortable' | 'compact' | 'minimal'
  zen_quiet: boolean
  [key: string]: unknown
}

export const defaultSettings: Settings = { zen: false, zen_density: 'comfortable', zen_quiet: true }

let current: Settings = defaultSettings
const listeners = new Set<() => void>()

function emit() {
  listeners.forEach((l) => l())
}

export function initSettings(s: Partial<Settings> | undefined) {
  current = { ...defaultSettings, ...(s ?? {}) }
  emit()
}

export function getSettings(): Settings {
  return current
}

export function useSettings(): Settings {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => current,
  )
}

export async function saveSettings(change: Partial<Settings>): Promise<void> {
  const before = current
  current = { ...current, ...change }
  emit()
  try {
    const saved = await api<Settings>('PATCH', '/api/v1/me/settings', change)
    if (saved) {
      current = { ...current, ...saved }
      emit()
    }
  } catch (e) {
    current = before
    emit()
    throw e
  }
}
