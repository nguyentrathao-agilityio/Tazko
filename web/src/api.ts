import { setCsrf } from '@/http'
import { initSettings } from '@/settings/store'

export type Company = { slug: string; name: string; role: 'owner' | 'member' }

export type Me = {
  id: number
  email: string
  name: string
  is_super_admin: boolean
  csrf_token: string
  companies: Company[]
  settings?: Record<string, unknown>
}

export class SignInNeeded extends Error {}

export async function getMe(signal?: AbortSignal): Promise<Me> {
  const res = await fetch('/api/v1/me', { signal, headers: { accept: 'application/json' } })
  if (res.status === 401) throw new SignInNeeded()
  if (!res.ok) throw new Error(`me ${res.status}`)
  const me = (await res.json()) as Me
  setCsrf(me.csrf_token)
  initSettings(me.settings)
  return me
}

export async function signOut(csrf: string): Promise<void> {
  const res = await fetch('/auth/signout', { method: 'POST', headers: { 'x-csrf-token': csrf } })
  if (!res.ok && res.status !== 401) throw new Error(`sign out ${res.status}`)
}

export type RefusalNote = { email: string; reason: string }

export async function getRefusal(signal?: AbortSignal): Promise<RefusalNote | null> {
  const res = await fetch('/auth/refusal', { signal, headers: { accept: 'application/json' } })
  if (!res.ok) return null
  return (await res.json()) as RefusalNote
}

export function signInHref(next?: string): string {
  return next && next !== '/' ? `/auth/google/start?next=${encodeURIComponent(next)}` : '/auth/google/start'
}
