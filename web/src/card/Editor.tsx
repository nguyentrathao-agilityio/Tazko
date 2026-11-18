import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent, type ReactNode } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'

export type DescriptionSave = { description_html: string } | { description_markdown: string }

function run(cmd: string, arg?: string) {
  document.execCommand(cmd, false, arg)
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
    const r = document.createRange()
    r.selectNodeContents(el)
    r.collapse(false)
    const sel = window.getSelection()
    sel?.removeAllRanges()
    sel?.addRange(r)
  }, [mode])

  function remember(e: FormEvent<HTMLDivElement>) {
    html.current = e.currentTarget.innerHTML
  }

  const save = () => {
    if (mode === 'markdown') onSave({ description_markdown: markdown })
    else onSave({ description_html: box.current?.innerHTML ?? html.current })
  }

  const key = (e: KeyboardEvent) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      save()
    }

    if (e.key === 'Escape') {
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
                if (s) run('insertHTML', `<code>${s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c] ?? c)}</code>`)
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
            <div ref={box} className="rte-ed" onInput={remember} contentEditable role="textbox" aria-multiline="true" aria-label={t('card.description')} suppressContentEditableWarning />
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
