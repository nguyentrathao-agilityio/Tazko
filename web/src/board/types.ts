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

export type ListData = { id: number; title: string; position: string }

export type BoardData = {
  id: number
  name: string
  description: string
  background: string
  restricted: boolean
  closed: boolean
  starred: boolean
  project: { id: number; name: string }
  role: ProjectRole
  can: Can
  people: BoardPerson[]
  lists: ListData[]
  archived_lists: number
}
