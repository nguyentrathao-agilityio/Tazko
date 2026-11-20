import type { BoardData, CardData, ListData } from '@/board/types'
import { fieldText, type FieldDef } from '@/card/fieldTypes'
import { toDateInput } from '@/card/dates'
import { displayName } from '@/ui/names'

export type Column = { key: string; name: string; field?: FieldDef }

export type Row = { card: CardData; list: ListData; listIndex: number }

export type Sort = { key: string | null; dir: 1 | -1 }

export function columns(board: BoardData, names: Record<string, string>): Column[] {
  const keys = ['title', 'list', 'labels', 'members', 'start', 'due', 'check', ...(board.voting ? ['votes'] : []), 'att', 'cm', 'last']
  const base: Column[] = keys.map((key) => ({ key, name: names[key] ?? key }))
  return base.concat((board.fields ?? []).map((f) => ({ key: `f:${f.id}`, name: f.name, field: f })))
}

export function shownByDefault(c: Column): boolean {
  if (c.key === 'cm' || c.key === 'last') return false
  if (c.field) return c.field.show_on_card
  return true
}

export function rows(lists: ListData[]): Row[] {
  return lists.flatMap((list, listIndex) => list.cards.map((card) => ({ card, list, listIndex })))
}

function key(r: Row, k: string): string | number | null {
  const c = r.card
  if (k === 'title') return c.title.toLowerCase()
  if (k === 'list') return r.listIndex
  if (k === 'start') return c.start_at ?? null
  if (k === 'due') return c.due_at ?? null
  if (k === 'check') return c.items ? (c.items_done ?? 0) / c.items : -1
  if (k === 'members') return c.members?.length ?? 0
  if (k === 'labels') return c.labels?.length ?? 0
  if (k === 'att') return c.attachments ?? 0
  if (k === 'votes') return c.votes ?? 0
  if (k === 'cm') return c.comments ?? 0
  if (k === 'last') return c.updated_at ?? 0
  if (k.startsWith('f:')) {
    const v = c.fields?.[k.slice(2)]
    if (v === undefined || v === '') return null
    return typeof v === 'string' ? v.toLowerCase() : v === true ? 1 : v === false ? 0 : v
  }

  return 0
}

export function sortRows(list: Row[], sort: Sort): Row[] {
  if (!sort.key) return list
  const k = sort.key
  return [...list].sort((a, b) => {
    const x = key(a, k)
    const y = key(b, k)
    if (x === null || y === null) return x === y ? 0 : x === null ? 1 : -1
    if (typeof x !== typeof y) return String(x).localeCompare(String(y)) * sort.dir
    return (x < y ? -1 : x > y ? 1 : 0) * sort.dir
  })
}

export function nextSort(sort: Sort, k: string): Sort {
  if (sort.key !== k) return { key: k, dir: 1 }
  if (sort.dir === 1) return { key: k, dir: -1 }
  return { key: null, dir: 1 }
}

function plain(r: Row, col: Column, board: BoardData): string {
  const c = r.card
  switch (col.key) {
    case 'title':
      return c.title
    case 'list':
      return r.list.title
    case 'labels':
      return (c.labels ?? []).map((id) => board.labels.find((l) => l.id === id)?.name ?? '').join('; ')
    case 'members':
      return (c.members ?? [])
        .map((id) => board.people.find((p) => p.id === id))
        .map((p) => (p ? displayName(p.name, p.email) : ''))
        .join('; ')
    case 'start':
      return toDateInput(c.start_at)
    case 'due':
      return toDateInput(c.due_at)
    case 'check':
      return c.items ? `${c.items_done ?? 0}/${c.items}` : ''
    case 'votes':
      return String(c.votes ?? 0)
    case 'att':
      return String(c.attachments ?? 0)
    case 'cm':
      return String(c.comments ?? 0)
    case 'last':
      return toDateInput(c.updated_at)
    default:
      return col.field ? fieldText(col.field, c.fields?.[String(col.field.id)]) : ''
  }
}

export function csv(list: Row[], cols: Column[], board: BoardData): string {
  const q = (raw: string) => {
    const s = /^[=+\-@\t\r]/.test(raw) && !/^-?\d+(\.\d+)?$/.test(raw) ? `'${raw}` : raw
    return `"${s.replace(/"/g, '""')}"`
  }
  return [cols.map((c) => q(c.name)).join(','), ...list.map((r) => cols.map((c) => q(plain(r, c, board))).join(','))].join('\n')
}
