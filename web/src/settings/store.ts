import { useSyncExternalStore } from 'react'
import { api } from '@/http'

export type Settings = {
  job_title: string
  avatar: 'google' | 'initials'
  language: 'en'
  theme: 'system' | 'light' | 'dark'
  zoom: number
  density: 'comfortable' | 'compact'
  label_style: 'names' | 'bars'
  reduce_motion: boolean
  zen: boolean
  zen_density: 'comfortable' | 'compact' | 'minimal'
  zen_quiet: boolean
  time_zone: string
  date_format: 'MDY' | 'DMY' | 'ISO'
  time_format: '12' | '24'
  week_start: 'mon' | 'sun' | 'sat'
}

export const defaultSettings: Settings = {
  job_title: '',
  avatar: 'google',
  language: 'en',
  theme: 'system',
  zoom: 100,
  density: 'comfortable',
  label_style: 'names',
  reduce_motion: false,
  zen: false,
  zen_density: 'comfortable',
  zen_quiet: true,
  time_zone: 'UTC',
  date_format: 'MDY',
  time_format: '12',
  week_start: 'mon',
}

let current: Settings = defaultSettings
const listeners = new Set<() => void>()

function emit() {
  listeners.forEach((l) => l())
}

export function initSettings(s: Partial<Settings> | Record<string, unknown> | undefined) {
  current = { ...defaultSettings, ...((s ?? {}) as Partial<Settings>) }
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
