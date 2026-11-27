export type Block =
  | { kind: 'heading'; level: 2 | 3; text: string }
  | { kind: 'bullets'; items: string[] }
  | { kind: 'numbers'; items: string[]; start: number }
  | { kind: 'quote'; lines: string[] }
  | { kind: 'code'; text: string }
  | { kind: 'para'; lines: string[] }

const BULLET = /^\s*[-*+]\s+(.*)$/
const NUMBER = /^\s*(\d{1,9})[.)]\s+(.*)$/
const HEADING = /^(#{1,6})\s+(.*)$/
const QUOTE = /^>(?: (.*)|$)/
const FENCE = /^```/

function interrupts(line: string): boolean {
  const n = NUMBER.exec(line)
  return FENCE.test(line) || HEADING.test(line) || BULLET.test(line) || QUOTE.test(line) || (n !== null && n[1] === '1')
}

export function parseBlocks(text: string): Block[] {
  const lines = text.replace(/\r\n?/g, '\n').split('\n')
  const out: Block[] = []
  let i = 0
  const run = (re: RegExp, group: number) => {
    const items: string[] = []
    let m: RegExpExecArray | null
    while (i < lines.length && (m = re.exec(lines[i]))) {
      items.push(m[group] ?? '')
      i++
    }
    return items
  }
  while (i < lines.length) {
    const line = lines[i]
    if (!line.trim()) {
      i++
      continue
    }

    if (FENCE.test(line)) {
      const body: string[] = []
      i++
      while (i < lines.length && !FENCE.test(lines[i])) body.push(lines[i++])
      i++
      out.push({ kind: 'code', text: body.join('\n') })
      continue
    }

    const h = HEADING.exec(line)
    if (h) {
      out.push({ kind: 'heading', level: h[1].length === 1 ? 2 : 3, text: h[2] })
      i++
      continue
    }

    const n = NUMBER.exec(line)
    if (n) out.push({ kind: 'numbers', start: Number(n[1]), items: run(NUMBER, 2) })
    else if (BULLET.test(line)) out.push({ kind: 'bullets', items: run(BULLET, 1) })
    else if (QUOTE.test(line)) out.push({ kind: 'quote', lines: run(QUOTE, 1) })
    else {
      const para = [lines[i++]]
      while (i < lines.length && lines[i].trim() && !interrupts(lines[i])) para.push(lines[i++])
      out.push({ kind: 'para', lines: para })
    }
  }
  return out
}

const LINK = String.raw`\[[^\][\n]+\]\((?:https?:\/\/|mailto:)(?:[^()\s]|\([^()\s]*\))+\)`

export const INLINE = new RegExp(String.raw`(\*\*[^*\n]+\*\*|~~[^~\n]+~~|` + '`[^`\\n]+`' + String.raw`|\*[^*\n]+\*|${LINK}|https?:\/\/[^\s<]+[^\s<.,;:!?)\]'"])`, 'g')

export function looksLikeMarkdown(text: string): boolean {
  return text.split('\n').some((l) => FENCE.test(l) || HEADING.test(l) || BULLET.test(l) || NUMBER.test(l) || /^> /.test(l)) || new RegExp(String.raw`\*\*[^*\n]+\*\*|` + '`[^`\\n]+`|' + LINK).test(text)
}

export type Token = { kind: 'text' | 'strong' | 'strike' | 'code' | 'em' | 'link'; text: string; href?: string }

export function tokens(text: string): Token[] {
  const out: Token[] = []
  let last = 0
  for (const m of text.matchAll(INLINE)) {
    const at = m.index ?? 0
    if (at > last) out.push({ kind: 'text', text: text.slice(last, at) })
    const s = m[0]
    if (s.startsWith('**')) out.push({ kind: 'strong', text: s.slice(2, -2) })
    else if (s.startsWith('~~')) out.push({ kind: 'strike', text: s.slice(2, -2) })
    else if (s.startsWith('`')) out.push({ kind: 'code', text: s.slice(1, -1) })
    else if (s.startsWith('*')) out.push({ kind: 'em', text: s.slice(1, -1) })
    else if (s.startsWith('[')) {
      const split = s.indexOf('](')
      out.push({ kind: 'link', text: s.slice(1, split), href: s.slice(split + 2, -1) })
    } else out.push({ kind: 'link', text: s, href: s })
    last = at + s.length
  }
  if (last < text.length) out.push({ kind: 'text', text: text.slice(last) })
  return out
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c] ?? c)
}

const TAGS: Record<Token['kind'], string> = { text: '', strong: 'strong', strike: 's', code: 'code', em: 'em', link: 'a' }

export function inlineHtml(text: string): string {
  return tokens(text)
    .map((t) => {
      const body = escapeHtml(t.text)
      if (t.kind === 'text') return body
      if (t.kind === 'link') return `<a href="${escapeHtml(t.href ?? '')}">${body}</a>`
      return `<${TAGS[t.kind]}>${body}</${TAGS[t.kind]}>`
    })
    .join('')
}

export function markdownToHtml(text: string): string {
  return parseBlocks(text)
    .map((b) => {
      switch (b.kind) {
        case 'heading':
          return `<h${b.level}>${inlineHtml(b.text)}</h${b.level}>`
        case 'bullets':
        case 'numbers':
          return `<${b.kind === 'bullets' ? 'ul' : 'ol'}>${b.items.map((i) => `<li>${inlineHtml(i)}</li>`).join('')}</${b.kind === 'bullets' ? 'ul' : 'ol'}>`
        case 'quote':
          return `<blockquote>${b.lines.map(inlineHtml).join('<br>')}</blockquote>`
        case 'code':
          return `<pre>${escapeHtml(b.text)}</pre>`
        default:
          return `<p>${b.lines.map(inlineHtml).join('<br>')}</p>`
      }
    })
    .join('')
}

export function plainToHtml(text: string): string {
  return text
    .replace(/\r\n?/g, '\n')
    .split(/\n{2,}/)
    .filter((p) => p.trim())
    .map((p) => `<p>${p.split('\n').map(inlineHtml).join('<br>')}</p>`)
    .join('')
}

const BLOCK_TAGS = /<(p|div|br|li|h[1-6]|blockquote|pre|tr)\b/i
const FORMATTED = /<(ul|ol|h[1-6]|strong|b|em|i|blockquote|table)\b/i
const CODE_EDITOR = /white-space:\s*pre\s*[;"]/i

export function pastedHtml(html: string, text: string): string | null {
  if (CODE_EDITOR.test(html) && text.includes('\n')) return `<pre>${escapeHtml(text.replace(/\r\n?/g, '\n'))}</pre>`
  if (!FORMATTED.test(html) && looksLikeMarkdown(text)) return markdownToHtml(text)
  if (!BLOCK_TAGS.test(html) && text.includes('\n')) return plainToHtml(text)
  return null
}
