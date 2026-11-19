import type { ReactNode } from 'react'

const pattern = /(\*\*[^*\n]+\*\*|`[^`\n]+`|\*[^*\n]+\*|https?:\/\/[^\s<]+[^\s<.,;:!?)\]'"])/g

function escape(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function mentionPattern(names: string[]): RegExp | null {
  const list = [...new Set(names.map((n) => n.trim()).filter((n) => n.length >= 2))].sort((a, b) => b.length - a.length)
  if (!list.length) return null
  return new RegExp(`(^|[\\s(])(@(?:${list.map(escape).join('|')}))(?![\\p{L}\\p{N}_@-])`, 'giu')
}

function plain(text: string, key: string, men: RegExp | null): ReactNode[] {
  if (!men) return [text]
  const out: ReactNode[] = []
  let last = 0
  let i = 0
  for (const m of text.matchAll(men)) {
    const at = (m.index ?? 0) + m[1].length
    if (at > last) out.push(text.slice(last, at))
    out.push(
      <span key={`${key}-m${i++}`} className="men">
        {m[2]}
      </span>,
    )
    last = at + m[2].length
  }
  if (last < text.length) out.push(text.slice(last))
  return out
}

function inline(text: string, key: string, men: RegExp | null): ReactNode[] {
  const out: ReactNode[] = []
  let last = 0
  let i = 0
  for (const m of text.matchAll(pattern)) {
    const at = m.index ?? 0
    if (at > last) out.push(...plain(text.slice(last, at), `${key}-p${i}`, men))
    const s = m[0]
    const k = `${key}-${i++}`
    if (s.startsWith('**')) out.push(<strong key={k}>{s.slice(2, -2)}</strong>)
    else if (s.startsWith('`')) out.push(<code key={k}>{s.slice(1, -1)}</code>)
    else if (s.startsWith('*')) out.push(<em key={k}>{s.slice(1, -1)}</em>)
    else
      out.push(
        <a key={k} href={s} target="_blank" rel="noopener noreferrer nofollow">
          {s}
        </a>,
      )
    last = at + s.length
  }
  if (last < text.length) out.push(...plain(text.slice(last), `${key}-end`, men))
  return out
}

export function CommentText({ text, names = [] }: { text: string; names?: string[] }) {
  const men = mentionPattern(names)
  const lines = text.split('\n')
  return (
    <>
      {lines.map((line, i) => (
        <span key={i}>
          {inline(line, String(i), men)}
          {i < lines.length - 1 && '\n'}
        </span>
      ))}
    </>
  )
}
