import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Editor } from '@/card/Editor'
import { CommentText } from '@/card/Rich'
import { looksLikeMarkdown, markdownToHtml, pastedHtml, plainToHtml } from '@/card/markdown'

afterEach(cleanup)

describe('markdown', () => {
  it('turns pasted markdown into the editor formatting', () => {
    expect(markdownToHtml('# Plan\n\n- **one**\n- `two`\n\n1. first\n\n> quoted\n\n```\na < b\n```\nline one\nline two')).toBe(
      '<h2>Plan</h2><ul><li><strong>one</strong></li><li><code>two</code></li></ul><ol><li>first</li></ol><blockquote>quoted</blockquote><pre>a &lt; b</pre><p>line one<br>line two</p>',
    )
  })

  it('escapes html and keeps only web and mail links', () => {
    expect(markdownToHtml('<img src=x onerror=alert(1)> [x](javascript:alert(1)) [site](https://example.com)')).toBe(
      '<p>&lt;img src=x onerror=alert(1)&gt; [x](javascript:alert(1)) <a href="https://example.com">site</a></p>',
    )
  })

  it('does not turn ordinary sentences into lists or quotes', () => {
    expect(markdownToHtml('shipped in\n2024. Great year\n>_< oops')).toBe('<p>shipped in<br>2024. Great year<br>&gt;_&lt; oops</p>')
    expect(markdownToHtml('steps:\n- one\n1. first')).toBe('<p>steps:</p><ul><li>one</li></ul><ol><li>first</li></ol>')
  })

  it('handles long runs of brackets quickly', () => {
    const started = performance.now()
    markdownToHtml('['.repeat(20_000))
    looksLikeMarkdown('['.repeat(20_000))
    expect(performance.now() - started).toBeLessThan(200)
  })

  it('keeps links with brackets in the address', () => {
    expect(markdownToHtml('[w](https://example.com/Foo_(bar))')).toBe('<p><a href="https://example.com/Foo_(bar)">w</a></p>')
  })

  it('keeps line breaks of pasted plain text', () => {
    expect(plainToHtml('first\nsecond\n\nnext')).toBe('<p>first<br>second</p><p>next</p>')
  })

  it('leaves formatted html from other editors to the browser', () => {
    expect(pastedHtml('<ul><li>a</li></ul>', '- a')).toBeNull()
    expect(pastedHtml('<span>one line</span>', 'one line')).toBeNull()
    expect(pastedHtml('<span style="white-space:pre-wrap">a\nb</span>', 'a\nb')).toBe('<p>a<br>b</p>')
    expect(pastedHtml('<div><span>## Title</span></div>', '## Title')).toBe('<h3>Title</h3>')
    expect(pastedHtml('<div style="white-space: pre;"><div># comment</div></div>', '# comment\nx = 1')).toBe('<pre># comment\nx = 1</pre>')
  })
})

describe('the description editor', () => {
  const exec = vi.fn()

  beforeEach(() => {
    exec.mockReset()
    Object.defineProperty(document, 'execCommand', { value: exec, configurable: true })
    Object.defineProperty(document, 'queryCommandValue', { value: () => '', configurable: true })
  })

  const editor = () => {
    render(<Editor initial="" onSave={() => {}} onCancel={() => {}} />)
    return screen.getByRole('textbox')
  }

  it('pastes plain text with its line breaks', () => {
    const box = editor()
    const data = { getData: (type: string) => (type === 'text/plain' ? 'first\nsecond' : '') }
    fireEvent.paste(box, { clipboardData: data })
    expect(exec).toHaveBeenCalledWith('insertHTML', false, '<p>first<br>second</p>')
  })

  it('turns a typed # and space into a heading', () => {
    const box = editor()
    box.innerHTML = '<p>#</p>'
    const text = box.querySelector('p')!.firstChild!
    window.getSelection()!.collapse(text, 1)
    fireEvent.keyDown(box, { key: ' ' })
    expect(exec.mock.calls.map((c) => c[0])).toEqual(expect.arrayContaining(['delete', 'formatBlock']))
    expect(exec).toHaveBeenCalledWith('formatBlock', false, 'H2')
  })

  it('turns - and space into a list on a line after a line break', () => {
    const box = editor()
    box.innerHTML = '<p>intro<br>-</p>'
    const text = box.querySelector('p')!.lastChild!
    window.getSelection()!.collapse(text, 1)
    fireEvent.keyDown(box, { key: ' ' })
    expect(exec).toHaveBeenCalledWith('insertUnorderedList', false, undefined)
  })

  it('leaves a code block when Enter is pressed on its last empty line', () => {
    const box = editor()
    box.innerHTML = '<pre>let x = 1\n</pre>'
    const text = box.querySelector('pre')!.firstChild!
    window.getSelection()!.collapse(text, text.textContent!.length)
    fireEvent.keyDown(box, { key: 'Enter' })
    expect(box.innerHTML).toBe('<pre>let x = 1</pre><p><br></p>')
  })

  it('makes text between ** bold as soon as it is closed', () => {
    const box = editor()
    box.innerHTML = '<p>see **this**</p>'
    const text = box.querySelector('p')!.firstChild!
    window.getSelection()!.collapse(text, text.textContent!.length)
    fireEvent.input(box, { data: '*' })
    expect(exec).toHaveBeenCalledWith('insertHTML', false, '<strong>this</strong>\u200b')
  })
})

describe('comments', () => {
  it('use the same markdown as descriptions', () => {
    const { container } = render(<CommentText text={'## Steps\n- open it\n- check ~~old~~\n\n3. third\nsee [docs](https://example.com)'} />)
    expect(container.querySelector('ol')?.getAttribute('start')).toBe('3')
    expect(container.querySelector('h3')?.textContent).toBe('Steps')
    expect([...container.querySelectorAll('ul li')].map((l) => l.textContent)).toEqual(['open it', 'check old'])
    expect(container.querySelector('s')?.textContent).toBe('old')
    expect(container.querySelector('a')?.getAttribute('href')).toBe('https://example.com')
  })
})
