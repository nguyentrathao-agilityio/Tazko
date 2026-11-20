export type PersonRef = { id: number; name: string; email: string } | null

export type ActivityEntry = { id: number; type: string; actor: PersonRef; payload: Record<string, unknown> | null; created_at: number }

import type { FieldValue, Reaction } from '@/card/fieldTypes'

export type Comment = { id: number; author: PersonRef; body: string; created_at: number; edited_at: number | null; reactions?: Reaction[] }

export type ChecklistItem = { id: number; text: string; done: boolean; assignee?: number | null; due_at?: number | null }

export type LinkKind = 'relates' | 'blocks' | 'blocked_by' | 'duplicates' | 'duplicated_by'

export type CardLink = { card: { id: number; title: string; done: boolean; archived: boolean }; board: { id: number; name: string }; list: string; kind: LinkKind }

export type Checklist = { id: number; title: string; items: ChecklistItem[] }

export type Attachment = { id: number; kind: 'file' | 'link'; name: string; size: number; url: string | null; image: boolean; type: string; created_at: number }

export type CardDetail = {
  id: number
  title: string
  description: string
  start_at: number | null
  due_at: number | null
  done: boolean
  cover: string | null
  list: { id: number; title: string }
  archived: boolean
  created_at: number
  created_by: PersonRef
  labels: number[]
  members: number[]
  checklists: Checklist[]
  links?: CardLink[]
  attachments: Attachment[]
  fields?: Record<string, FieldValue>
  cover_image: number | null
  is_template?: boolean
  recurrence?: string | null
  comments: Comment[]
  activity: ActivityEntry[]
  can: { edit: boolean }
}
