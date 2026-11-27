import type { ReactNode } from 'react'
import { parseBlocks, tokens } from '@/card/markdown'

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
  return tokens(text).flatMap((tk, i): ReactNode[] => {
    const k = `${key}-${i}`
    switch (tk.kind) {
      case 'text':
        return plain(tk.text, k, men)
      case 'strong':
        return [<strong key={k}>{tk.text}</strong>]
      case 'strike':
        return [<s key={k}>{tk.text}</s>]
      case 'code':
        return [<code key={k}>{tk.text}</code>]
      case 'em':
        return [<em key={k}>{tk.text}</em>]
      default:
        return [
          <a key={k} href={tk.href} target="_blank" rel="noopener noreferrer nofollow">
            {tk.text}
          </a>,
        ]
    }
  })
}

export function CommentText({ text, names = [] }: { text: string; names?: string[] }) {
  const men = mentionPattern(names)
  const lines = (items: string[], key: string) =>
    items.map((line, i) => (
      <span key={i}>
        {i > 0 && <br />}
        {inline(line, `${key}-${i}`, men)}
      </span>
    ))
  return (
    <>
      {parseBlocks(text).map((b, i) => {
        const key = String(i)
        switch (b.kind) {
          case 'heading':
            return b.level === 2 ? <h2 key={key}>{inline(b.text, key, men)}</h2> : <h3 key={key}>{inline(b.text, key, men)}</h3>
          case 'bullets':
            return <ul key={key}>{b.items.map((item, j) => <li key={j}>{inline(item, `${key}-${j}`, men)}</li>)}</ul>
          case 'numbers':
            return (
              <ol key={key} start={b.start}>
                {b.items.map((item, j) => (
                  <li key={j}>{inline(item, `${key}-${j}`, men)}</li>
                ))}
              </ol>
            )
          case 'quote':
            return <blockquote key={key}>{lines(b.lines, key)}</blockquote>
          case 'code':
            return <pre key={key}>{b.text}</pre>
          default:
            return <p key={key}>{lines(b.lines, key)}</p>
        }
      })}
    </>
  )
}
