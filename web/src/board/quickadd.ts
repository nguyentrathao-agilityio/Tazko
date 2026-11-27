import type { BoardPerson, Label } from '@/board/types'
import { fromInputs } from '@/card/dates'
import { isObserver } from '@/roles'
import { displayName } from '@/ui/names'

export type QuickCard = { title: string; labels: number[]; members: number[]; due_at: number | null }

export type Unknown = { kind: 'label' | 'person' | 'ambiguous' | 'date'; text: string }

export type Parsed = QuickCard & { unknown: Unknown[] }

const TOKEN = /(^|\s)(\\?)([#!@^])([^\s#!@^]+)/g

const DAYS = [
  ['sun', 'sunday'],
  ['mon', 'monday'],
  ['tue', 'tues', 'tuesday'],
  ['wed', 'wednesday'],
  ['thu', 'thur', 'thurs', 'thursday'],
  ['fri', 'friday'],
  ['sat', 'saturday'],
]

export const fold = (s: string) => s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()

export const assignable = (people: BoardPerson[]) => people.filter((p) => p.access && !isObserver(p.role))

export const labelSlug = (l: Label) => fold(l.name).replace(/\s+/g, '-')

export const handle = (p: BoardPerson) => fold(p.email.split('@')[0])

function findLabel(labels: Label[], word: string): Label | undefined {
  const w = fold(word)
  return labels.find((l) => l.name.trim() && (fold(l.name) === w || labelSlug(l) === w))
}

function findPeople(people: BoardPerson[], word: string): BoardPerson[] {
  const w = fold(word)
  const byHandle = assignable(people).filter((p) => handle(p) === w)
  if (byHandle.length) return byHandle
  return assignable(people).filter((p) => fold(p.name).split(/\s+/)[0] === w || fold(p.name).replace(/\s+/g, '') === w)
}

const pad = (n: number) => String(n).padStart(2, '0')

function noon(d: Date): number | null {
  if (d.getFullYear() < 1970 || d.getFullYear() > 9999) return null
  return fromInputs(`${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`, '12:00')
}

export function parseDate(word: string, now: Date = new Date()): number | null {
  const w = fold(word)
  const day = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  if (w === 'today') return noon(day)
  if (w === 'tomorrow' || w === 'tmr') return noon(new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1))
  const rel = /^\+(\d{1,3})([dw])$/.exec(w)
  if (rel) return noon(new Date(day.getFullYear(), day.getMonth(), day.getDate() + Number(rel[1]) * (rel[2] === 'w' ? 7 : 1)))
  const dow = DAYS.findIndex((names) => names.includes(w))
  if (dow >= 0) return noon(new Date(day.getFullYear(), day.getMonth(), day.getDate() + ((dow - day.getDay() + 7) % 7)))
  const iso = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(w)
  const dm = /^(\d{1,2})\/(\d{1,2})(?:\/(\d{4}))?$/.exec(w)
  const [y, m, d] = iso ? [Number(iso[1]), Number(iso[2]), Number(iso[3])] : dm ? [dm[3] ? Number(dm[3]) : day.getFullYear(), Number(dm[2]), Number(dm[1])] : [0, 0, 0]
  if (!y) return null
  const valid = (year: number) => {
    const date = new Date(year, m - 1, d)
    date.setFullYear(year)
    return date.getMonth() === m - 1 && date.getDate() === d ? date : null
  }
  const date = valid(y)
  if (dm && !dm[3] && date && date < day) {
    const next = valid(y + 1)
    return next ? noon(next) : null
  }
  return date ? noon(date) : null
}

export function parseLine(line: string, labels: Label[], people: BoardPerson[], now: Date = new Date()): Parsed {
  const out: Parsed = { title: '', labels: [], members: [], due_at: null, unknown: [] }
  const title = line.replace(TOKEN, (whole, space: string, escaped: string, sigil: string, word: string) => {
    if (escaped) return `${space}${sigil}${word}`
    if (sigil === '#' || sigil === '!') {
      const l = findLabel(labels, word)
      if (l) {
        if (!out.labels.includes(l.id)) out.labels.push(l.id)
        return space
      }
      if (sigil === '!') out.unknown.push({ kind: 'label', text: word })
      else if (/^\D/.test(word)) out.unknown.push({ kind: 'label', text: word })
      return whole
    }

    if (sigil === '@') {
      const hits = findPeople(people, word)
      if (hits.length === 1) {
        if (!out.members.includes(hits[0].id)) out.members.push(hits[0].id)
        return space
      }
      out.unknown.push({ kind: hits.length ? 'ambiguous' : 'person', text: word })
      return whole
    }

    const due = parseDate(word, now)
    if (due !== null) {
      out.due_at = due
      return space
    }
    out.unknown.push({ kind: 'date', text: word })
    return whole
  })
  out.title = title.replace(/\s+/g, ' ').trim()
  return out
}

export type Suggestion = { insert: string; label: string; color?: number }

export function suggest(text: string, labels: Label[], people: BoardPerson[]): Suggestion[] {
  const m = /(^|\s)([#!@^])([^\s#!@^]*)$/.exec(text)
  if (!m) return []
  const [, , sigil, typed] = m
  const w = fold(typed)
  if (sigil === '#' || sigil === '!') {
    return labels
      .filter((l) => l.name.trim() && labelSlug(l).startsWith(w))
      .slice(0, 6)
      .map((l) => ({ insert: `${sigil}${labelSlug(l)}`, label: l.name, color: l.color }))
  }

  if (sigil === '@') {
    return assignable(people)
      .filter((p) => handle(p).startsWith(w) || fold(p.name).startsWith(w))
      .slice(0, 6)
      .map((p) => ({ insert: `@${handle(p)}`, label: displayName(p.name, p.email) }))
  }

  return ['today', 'tomorrow', 'mon', 'tue', 'wed', 'thu', 'fri', '+1w']
    .filter((d) => d.startsWith(w))
    .slice(0, 6)
    .map((d) => ({ insert: `^${d}`, label: d }))
}

export function applySuggestion(text: string, s: Suggestion): string {
  return text.replace(/([#!@^])([^\s#!@^]*)$/, '') + `${s.insert} `
}
