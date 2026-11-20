import { useSyncExternalStore } from 'react'
import { api } from '@/http'
import { ME_API } from '@/paths'

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
  notify_mention_app: boolean
  notify_mention_email: boolean
  notify_assigned_app: boolean
  notify_assigned_email: boolean
  notify_card_added_app: boolean
  notify_card_added_email: boolean
  notify_due_email: boolean
  email_frequency: 'instant' | 'hourly' | 'daily' | 'never'
  due_reminder: 'at' | '1h' | '1d' | '2d'
  daily_hour_utc: number
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
  notify_mention_app: true,
  notify_mention_email: true,
  notify_assigned_app: true,
  notify_assigned_email: true,
  notify_card_added_app: true,
  notify_card_added_email: false,
  notify_due_email: true,
  email_frequency: 'instant',
  due_reminder: '1d',
  daily_hour_utc: 1,
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
    const saved = await api<Settings>('PATCH', `${ME_API}/settings`, change)
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
