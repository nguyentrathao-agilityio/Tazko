import type { BoardData, CardData, ListData } from '@/board/types'
import { fieldText, type FieldDef } from '@/card/fieldTypes'
import { shortDate } from '@/card/look'
import { displayName } from '@/ui/names'

export type ColumnKey = string

export type Column = { key: ColumnKey; name: string; field?: FieldDef }

export type Row = { card: CardData; list: ListData; listIndex: number }

export type Sort = { key: ColumnKey | null; dir: 1 | -1 }

export function columns(board: BoardData, names: Record<string, string>): Column[] {
  const base: Column[] = ['title', 'list', 'labels', 'members', 'start', 'due', 'check', 'att', 'cm', 'last'].map((key) => ({ key, name: names[key] ?? key }))
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

function key(r: Row, k: ColumnKey): string | number {
  const c = r.card
  if (k === 'title') return c.title.toLowerCase()
  if (k === 'list') return r.listIndex
  if (k === 'start') return c.start_at ?? Number.MAX_SAFE_INTEGER
  if (k === 'due') return c.due_at ?? Number.MAX_SAFE_INTEGER
  if (k === 'check') return c.items ? (c.items_done ?? 0) / c.items : -1
  if (k === 'members') return c.members?.length ?? 0
  if (k === 'labels') return c.labels?.length ?? 0
  if (k === 'att') return c.attachments ?? 0
  if (k === 'cm') return c.comments ?? 0
  if (k === 'last') return c.updated_at ?? 0
  if (k.startsWith('f:')) {
    const v = c.fields?.[k.slice(2)]
    if (v === undefined || v === '') return '~'
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
    return (x < y ? -1 : x > y ? 1 : 0) * sort.dir
  })
}

export function nextSort(sort: Sort, k: ColumnKey): Sort {
  if (sort.key !== k) return { key: k, dir: 1 }
  if (sort.dir === 1) return { key: k, dir: -1 }
  return { key: null, dir: 1 }
}

export function plain(r: Row, col: Column, board: BoardData): string {
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
      return c.start_at ? shortDate(c.start_at) : ''
    case 'due':
      return c.due_at ? shortDate(c.due_at) : ''
    case 'check':
      return c.items ? `${c.items_done ?? 0}/${c.items}` : ''
    case 'att':
      return String(c.attachments ?? 0)
    case 'cm':
      return String(c.comments ?? 0)
    case 'last':
      return c.updated_at ? shortDate(c.updated_at) : ''
    default:
      return col.field ? fieldText(col.field, c.fields?.[String(col.field.id)]) : ''
  }
}

export function csv(list: Row[], cols: Column[], board: BoardData): string {
  const q = (s: string) => `"${s.replace(/"/g, '""')}"`
  return [cols.map((c) => q(c.name)).join(','), ...list.map((r) => cols.map((c) => q(plain(r, c, board))).join(','))].join('\n')
}

export function dayValue(unix: number | null | undefined): string {
  if (!unix) return ''
  const d = new Date(unix * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

export function fromDay(value: string, keep: number | null | undefined): number | null {
  if (!value) return null
  const [y, m, d] = value.split('-').map(Number)
  const old = keep ? new Date(keep * 1000) : null
  const at = new Date(y, m - 1, d, old ? old.getHours() : 12, old ? old.getMinutes() : 0)
  return Math.floor(at.getTime() / 1000)
}
