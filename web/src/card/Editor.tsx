import { useEffect, useRef, useState, type ClipboardEvent, type FormEvent, type KeyboardEvent, type ReactNode } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { ESCAPE, ENTER } from '@/keys/keyNames'
import { escapeHtml, pastedHtml } from '@/card/markdown'

export type DescriptionSave = { description_html: string } | { description_markdown: string }

function run(cmd: string, arg?: string) {
  document.execCommand(cmd, false, arg)
}

const MARK = '\u200b'

const BLOCKS = new Set(['P', 'DIV', 'LI', 'H2', 'H3', 'BLOCKQUOTE', 'PRE'])

const SHORTCUTS: Record<string, () => void> = {
  '#': () => run('formatBlock', 'H2'),
  '-': () => run('insertUnorderedList'),
  '*': () => run('insertUnorderedList'),
  '1.': () => run('insertOrderedList'),
  '>': () => run('formatBlock', 'BLOCKQUOTE'),
}

function shortcutFor(marker: string): (() => void) | undefined {
  return /^#{2,6}$/.test(marker) ? () => run('formatBlock', 'H3') : SHORTCUTS[marker]
}

type Caret = { sel: Selection; block: HTMLElement; line: Range; after: Range }

function caret(root: HTMLElement): Caret | null {
  const sel = window.getSelection()
  if (!sel || !sel.isCollapsed || !sel.anchorNode || !root.contains(sel.anchorNode)) return null
  let node: Node = sel.anchorNode
  while (node !== root && !(node instanceof HTMLElement && BLOCKS.has(node.tagName))) node = node.parentNode ?? root
  const block = node as HTMLElement
  const line = document.createRange()
  line.setStart(block, 0)
  line.setEnd(sel.anchorNode, sel.anchorOffset)
  const breaks = [...block.querySelectorAll('br')].filter((br) => line.intersectsNode(br))
  if (breaks.length) line.setStartAfter(breaks[breaks.length - 1])
  const after = document.createRange()
  after.setStart(sel.anchorNode, sel.anchorOffset)
  after.setEnd(block, block.childNodes.length)
  return { sel, block, line, after }
}

function leaveBlock({ sel, block, line, after }: Caret): boolean {
  if (!['BLOCKQUOTE', 'PRE'].includes(block.tagName) || after.toString()) return false
  const text = line.toString()
  if (block.tagName === 'PRE' ? text !== '' && !text.endsWith('\n') : text !== '') return false
  for (let last = block.lastChild; last; last = block.lastChild) {
    if (last.nodeType === Node.TEXT_NODE && last.textContent?.endsWith('\n')) {
      last.textContent = last.textContent.slice(0, -1)
      break
    }
    if (last.nodeName !== 'BR' && last.textContent) break
    last.remove()
  }
  const p = document.createElement('p')
  p.appendChild(document.createElement('br'))
  block.after(p)
  if (!block.textContent) block.remove()
  sel.collapse(p, 0)
  return true
}

function shortcut(root: HTMLElement, typed: string): boolean {
  const at = caret(root)
  if (!at) return false
  if (typed === ENTER && leaveBlock(at)) return true
  if (at.block.tagName === 'PRE') return false
  const marker = at.line.toString()
  const act = typed === ' ' ? shortcutFor(marker) : marker === '```' ? () => run('formatBlock', 'PRE') : undefined
  if (!act) return false
  at.sel.removeAllRanges()
  at.sel.addRange(at.line)
  run('delete')
  act()
  return true
}

const WRAPS: [RegExp, string][] = [
  [/\*\*([^*]+)\*\*$/, 'strong'],
  [/~~([^~]+)~~$/, 's'],
  [/(?<!`)`([^`]+)`$/, 'code'],
]

function wrapInline(root: HTMLElement) {
  const sel = window.getSelection()
  const node = sel?.anchorNode
  if (!sel || !node || node.nodeType !== Node.TEXT_NODE || !root.contains(node)) return
  const text = (node.textContent ?? '').slice(0, sel.anchorOffset)
  for (const [re, tag] of WRAPS) {
    const m = re.exec(text)
    if (!m) continue
    const r = document.createRange()
    r.setStart(node, sel.anchorOffset - m[0].length)
    r.setEnd(node, sel.anchorOffset)
    sel.removeAllRanges()
    sel.addRange(r)
    run('insertHTML', `<${tag}>${escapeHtml(m[1])}</${tag}>${MARK}`)
    return
  }
}

export function Editor({ initial, onSave, onCancel }: { initial: string; onSave: (body: DescriptionSave) => void; onCancel: () => void }) {
  const [mode, setMode] = useState<'rich' | 'markdown'>('rich')
  const [markdown, setMarkdown] = useState('')
  const [linking, setLinking] = useState(false)
  const [url, setUrl] = useState('')
  const box = useRef<HTMLDivElement>(null)
  const [range, setRange] = useState<Range | null>(null)
  const html = useRef(initial)

  useEffect(() => {
    const el = box.current
    if (!el || mode !== 'rich') return
    el.innerHTML = html.current
    el.focus()
    run('defaultParagraphSeparator', 'p')
    const r = document.createRange()
    r.selectNodeContents(el)
    r.collapse(false)
    const sel = window.getSelection()
    sel?.removeAllRanges()
    sel?.addRange(r)
  }, [mode])

  function remember(e: FormEvent<HTMLDivElement>) {
    const data = (e.nativeEvent as InputEvent).data
    if (data === '*' || data === '`' || data === '~') wrapInline(e.currentTarget)
    html.current = e.currentTarget.innerHTML
  }

  function typing(e: KeyboardEvent<HTMLDivElement>) {
    if ((e.key === ' ' || (e.key === ENTER && !e.shiftKey && !e.ctrlKey && !e.metaKey)) && shortcut(e.currentTarget, e.key)) {
      e.preventDefault()
      html.current = e.currentTarget.innerHTML
    }
  }

  function paste(e: ClipboardEvent<HTMLDivElement>) {
    const out = pastedHtml(e.clipboardData.getData('text/html'), e.clipboardData.getData('text/plain'))
    if (out === null) return
    e.preventDefault()
    run('insertHTML', out)
    html.current = e.currentTarget.innerHTML
  }

  const save = () => {
    if (mode === 'markdown') onSave({ description_markdown: markdown })
    else onSave({ description_html: (box.current?.innerHTML ?? html.current).replaceAll(MARK, '') })
  }

  const key = (e: KeyboardEvent) => {
    if (e.key === ENTER && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      save()
    }

    if (e.key === ESCAPE) {
      e.stopPropagation()
      onCancel()
    }
  }

  const tool = (label: string, content: ReactNode, act: () => void) => (
    <button
      type="button"
      className="rtb"
      aria-label={label}
      title={label}
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => {
        box.current?.focus()
        act()
      }}
    >
      {content}
    </button>
  )

  const block = (tag: string) => {
    const cur = String(document.queryCommandValue('formatBlock') || '').toLowerCase()
    run('formatBlock', cur === tag.toLowerCase() ? 'P' : tag)
  }

  const applyLink = () => {
    const u = url.trim()
    if (!/^(https?:\/\/|mailto:)/i.test(u)) return
    box.current?.focus()
    const sel = window.getSelection()
    if (range && sel) {
      sel.removeAllRanges()
      sel.addRange(range)
    }
    if (sel && sel.toString()) run('createLink', u)
    else {
      const a = document.createElement('a')
      a.href = u
      a.textContent = u
      range?.insertNode(a)
    }
    setLinking(false)
    setUrl('')
  }

  return (
    <div onKeyDown={key}>
      <div className="tabs2" role="tablist" style={{ marginBottom: 8 }}>
        <button type="button" role="tab" aria-selected={mode === 'rich'} className={mode === 'rich' ? 'on' : ''} onClick={() => setMode('rich')}>
          {t('editor.rich')}
        </button>
        <button type="button" role="tab" aria-selected={mode === 'markdown'} className={mode === 'markdown' ? 'on' : ''} onClick={() => setMode('markdown')}>
          {t('editor.markdown')}
        </button>
      </div>
      <div className="rte">
        {mode === 'rich' && (
          <>
            <div className="rtt" role="toolbar" aria-label={t('editor.formatting')}>
              {tool(t('editor.bold'), <b>B</b>, () => run('bold'))}
              {tool(t('editor.italic'), <i>I</i>, () => run('italic'))}
              {tool(t('editor.strike'), <s>S</s>, () => run('strikeThrough'))}
              <span className="rts" />
              {tool(t('editor.heading'), 'H', () => block('H3'))}
              {tool(t('editor.quote'), '“', () => block('BLOCKQUOTE'))}
              <span className="rts" />
              {tool(t('editor.bullets'), <Icon name="list" />, () => run('insertUnorderedList'))}
              {tool(t('editor.numbers'), '1.', () => run('insertOrderedList'))}
              <span className="rts" />
              {tool(t('editor.code'), '</>', () => {
                const s = window.getSelection()?.toString()
                if (s) run('insertHTML', `<code>${escapeHtml(s)}</code>`)
              })}
              {tool(t('editor.link'), <Icon name="link" />, () => {
                const s = window.getSelection()
                setRange(s && s.rangeCount ? s.getRangeAt(0).cloneRange() : null)
                setLinking(true)
              })}
              {tool(t('editor.clear'), 'Tx', () => {
                run('removeFormat')
                run('formatBlock', 'P')
              })}
            </div>
            {linking && (
              <div className="rtl">
                <input type="text" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://" aria-label={t('editor.linkAddress')} autoFocus />
                <button type="button" className="btn sm primary" onClick={applyLink}>
                  {t('editor.apply')}
                </button>
                <button type="button" className="btn sm" onClick={() => setLinking(false)}>
                  {t('common.cancel')}
                </button>
              </div>
            )}
            <div ref={box} className="rte-ed" onInput={remember} onKeyDown={typing} onPaste={paste} contentEditable role="textbox" aria-multiline="true" aria-label={t('card.description')} suppressContentEditableWarning />
          </>
        )}
        {mode === 'markdown' && (
          <textarea value={markdown} onChange={(e) => setMarkdown(e.target.value)} aria-label={t('editor.markdownLabel')} placeholder={t('editor.markdownHint')} autoFocus />
        )}
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <button type="button" className="btn primary sm" onClick={save}>
          {t('boardMenu.save')}
        </button>
        <button type="button" className="btn sm" onClick={onCancel}>
          {t('common.cancel')}
        </button>
        <small className="muted">{t('editor.saveHint')}</small>
      </div>
    </div>
  )
}
