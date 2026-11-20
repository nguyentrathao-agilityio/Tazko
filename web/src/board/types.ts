import type { FieldDef, FieldValue } from '@/card/fieldTypes'
import type { Can, ProjectRole } from '@/projects/types'

export type BoardTileData = {
  id: number
  name: string
  background: string
  restricted: boolean
  closed: boolean
  starred: boolean
  lists: number
  cards: number
  project_id: number
}

export type BoardPerson = { id: number; name: string; email: string; role: ProjectRole; access: boolean }

export type CardData = {
  id: number
  title: string
  position: string
  has_description?: boolean
  start_at?: number | null
  due_at?: number | null
  done?: boolean
  cover?: string | null
  cover_image?: number | null
  attachments?: number
  is_template?: boolean
  recurrence?: string | null
  updated_at?: number
  fields?: Record<string, FieldValue>
  comments?: number
  items?: number
  items_done?: number
  labels?: number[]
  members?: number[]
}

export type Label = { id: number; name: string; color: number }

export type ListData = { id: number; title: string; position: string; cards: CardData[] }

export type BoardData = {
  id: number
  name: string
  description: string
  background: string
  restricted: boolean
  closed: boolean
  starred: boolean
  watching?: boolean
  project: { id: number; name: string }
  role: ProjectRole
  can: Can
  people: BoardPerson[]
  lists: ListData[]
  labels: Label[]
  fields?: FieldDef[]
  archived_lists: number
  last_event?: number
}
