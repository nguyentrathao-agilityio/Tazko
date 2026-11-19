import type { CompanyRole } from '@/roles'

export type CompanyRow = {
  slug: string
  name: string
  status: 'active' | 'suspended'
  members: number
  boards: number
  storage_used: number
  storage_limit: number | null
  last_activity: number
}

export type Member = {
  id: number
  name: string
  email: string
  role: CompanyRole
  last_seen: number | null
  disabled: boolean
}

export type Invite = {
  id: number
  email: string
  role: CompanyRole
  invited_by: string | null
  created_at: number
  company_slug: string
  company_name: string
}

export type ProjectRow = { name: string; members: number; boards: number; owners: string[] }

export type CompanyDetail = {
  slug: string
  name: string
  status: 'active' | 'suspended'
  storage_limit: number | null
  storage_used: number
  boards: number
  members: Member[]
  invites: Invite[]
  projects: ProjectRow[]
}

export type PersonRow = {
  id: number
  name: string
  email: string
  last_seen: number | null
  is_super_admin: boolean
  disabled: boolean
  companies: string[]
}

export type AuditEntry = {
  id: number
  at: number
  action: string
  kind: 'refused' | 'company' | 'invites' | 'people'
  target: string | null
  detail: Record<string, unknown>
  actor: string | null
  company: string | null
}

export type RunStatus = {
  at: number
  ok: boolean
  snapshot: string | null
  databases: number
  files: number
  bytes: number
  problems: string[]
}

export type ServerInfo = {
  version: string
  uptime_secs: number
  data_bytes: number
  companies: number
  public_url: string
  email: 'on' | 'off'
  last_backup: RunStatus | null
  last_restore_test: RunStatus | null
}
