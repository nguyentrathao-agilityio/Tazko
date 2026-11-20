import type { ProjectRole } from '@/roles'
export type { ProjectRole }

export type Person = { id: number; name: string; email: string; role: string }

export type MyProject = {
  id: number
  name: string
  description: string
  color: string
  role: ProjectRole
  members: Person[]
  boards: number
}

export type Can = { edit: boolean; manage: boolean; invite: boolean; create_board: boolean }

export type ProjectView = {
  id: number
  name: string
  description: string
  color: string
  background: string
  members_create_boards: boolean
  members_invite: boolean
  voting?: boolean
  role: ProjectRole
  member_count: number
  can: Can
}

export type ProjectMember = Person & { last_active: number | null; you: boolean }

export type ProjectInvite = { id: number; email: string; role: ProjectRole; invited_by: string | null; created_at: number }

export type ManagedProject = {
  id: number
  name: string
  status: 'active' | 'archived'
  owners: Person[]
  members: number
  boards: number
}
