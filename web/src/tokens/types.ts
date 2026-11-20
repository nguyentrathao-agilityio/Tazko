export type Scope = 'read' | 'write' | 'manage'

export type TokenRow = {
  id: number
  name: string
  scope: Scope
  projects: number[] | { company: string; project: number }[] | null
  created_at: number
  expires_at: number | null
  last_used_at: number | null
  by?: string
}

export const SCOPES: Scope[] = ['read', 'write', 'manage']

export const EXPIRY = [30, 90, 365, 0] as const
