import { displayName } from '@/ui/names'

export type Person = { id: number; name: string; email: string }

const MAX = 6

export function mentionQuery(text: string, caret: number): { start: number; query: string } | null {
  const before = text.slice(0, caret)
  const m = /(^|[\s(])@([^@\s][^@\n]{0,30}|)$/.exec(before)
  if (!m) return null
  const query = m[2]
  if (query.split(' ').length > 3 || query.endsWith(' ')) return null
  return { start: before.length - query.length - 1, query }
}

export function matches(people: Person[], query: string): Person[] {
  const q = query.trim().toLowerCase()
  return people
    .filter((p) => !q || displayName(p.name, p.email).toLowerCase().includes(q) || p.email.toLowerCase().startsWith(q))
    .slice(0, MAX)
}
