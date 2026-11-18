import type { ReactNode } from 'react'

const pattern = /(\*\*[^*\n]+\*\*|`[^`\n]+`|\*[^*\n]+\*|https?:\/\/[^\s<]+[^\s<.,;:!?)\]'"])/g

function inline(text: string, key: string): ReactNode[] {
  const out: ReactNode[] = []
  let last = 0
  let i = 0
  for (const m of text.matchAll(pattern)) {
    const at = m.index ?? 0
    if (at > last) out.push(text.slice(last, at))
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
  if (last < text.length) out.push(text.slice(last))
  return out
}

export function CommentText({ text }: { text: string }) {
  const lines = text.split('\n')
  return (
    <>
      {lines.map((line, i) => (
        <span key={i}>
          {inline(line, String(i))}
          {i < lines.length - 1 && '\n'}
        </span>
      ))}
    </>
  )
}
