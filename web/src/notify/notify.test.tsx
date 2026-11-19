import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { useState } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MentionBox } from '@/card/MentionBox'
import { mentionQuery } from '@/card/mention'
import { Bell } from '@/notify/Bell'
import { companyOf } from '@/notify/notes'

class FakeSocket {
  static OPEN = 1
  static last: FakeSocket | null = null
  readyState = 1
  url: string
  onmessage: ((e: { data: string }) => void) | null = null
  onclose: (() => void) | null = null
  onopen: (() => void) | null = null
  constructor(url: string) {
    this.url = url
    FakeSocket.last = this
  }
  send() {}
  close() {}
  push(v: unknown) {
    this.onmessage?.({ data: JSON.stringify(v) })
  }
}

const notes = [
  { id: 3, kind: 'mention', actor: { id: 2, name: 'Jo Lee', email: 'jo@example.com' }, card: { id: 21, title: 'Write copy' }, board: { id: 7, name: 'Sprint 15' }, created_at: 1_700_000_000, read: false, due_at: null },
  { id: 2, kind: 'due', actor: null, card: { id: 22, title: 'Pricing table' }, board: { id: 7, name: 'Sprint 15' }, created_at: 1_699_999_000, read: false, due_at: 1_700_050_000 },
  { id: 1, kind: 'assigned', actor: { id: 2, name: 'Jo Lee', email: 'jo@example.com' }, card: { id: 23, title: 'Nav' }, board: { id: 7, name: 'Sprint 15' }, created_at: 1_699_990_000, read: true, due_at: null },
]
let calls: { url: string; method: string }[] = []

beforeEach(() => {
  calls = []
  vi.stubGlobal('WebSocket', FakeSocket)
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      calls.push({ url, method: init.method ?? 'GET' })
      if (url === '/api/v1/c/acme/notifications') return new Response(JSON.stringify({ notifications: notes, unread: 2 }), { status: 200 })
      return new Response(null, { status: 204 })
    }),
  )
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.history.pushState(null, '', '/')
})

describe('notification bell', () => {
  it('shows the live unread count and opens the card from a notification', async () => {
    render(<Bell slug="acme" />)
    expect(FakeSocket.last?.url).toBe('ws://localhost:3000/api/v1/c/acme/notifications/live'.replace('localhost:3000', window.location.host))
    act(() => FakeSocket.last?.push({ type: 'unread', count: 2 }))
    const bell = screen.getByRole('button', { name: 'Notifications, 2 unread' })
    expect(within(bell).getByText('2')).toBeTruthy()
    fireEvent.click(bell)
    const pop = await screen.findByRole('dialog', { name: 'Notifications' })
    await within(pop).findByText('Write copy')
    expect(pop.textContent).toContain('Jo Lee mentioned you in a comment on Write copy')
    expect(pop.textContent).toContain('Due soon: Pricing table')
    fireEvent.click(within(pop).getByRole('button', { name: 'Unread (2)' }))
    expect(within(pop).queryByText('Nav')).toBeNull()
    const seen = vi.fn()
    window.addEventListener('popstate', seen)
    fireEvent.click(within(pop).getByText('Write copy').closest('button')!)
    expect(window.location.pathname).toBe('/c/acme/b/7/c/21')
    expect(seen).toHaveBeenCalled()
    await waitFor(() => expect(calls.some((c) => c.method === 'POST' && c.url === '/api/v1/c/acme/notifications/3/read')).toBe(true))
    window.removeEventListener('popstate', seen)
    act(() => FakeSocket.last?.push({ type: 'unread', count: 0 }))
    expect(screen.getByRole('button', { name: 'Notifications' })).toBeTruthy()
  })

  it('marks everything read', async () => {
    render(<Bell slug="acme" />)
    act(() => FakeSocket.last?.push({ type: 'unread', count: 2 }))
    fireEvent.click(screen.getByRole('button', { name: 'Notifications, 2 unread' }))
    const pop = await screen.findByRole('dialog', { name: 'Notifications' })
    await within(pop).findByText('Write copy')
    fireEvent.click(within(pop).getByRole('button', { name: 'Mark all read' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'POST' && c.url === '/api/v1/c/acme/notifications/read')).toBe(true))
  })

  it('knows the company from the address', () => {
    expect(companyOf('/c/acme/b/7')).toBe('acme')
    expect(companyOf('/c/acme')).toBe('acme')
    expect(companyOf('/settings')).toBeNull()
  })
})

const people = [
  { id: 2, name: 'Jo Lee', email: 'jo@example.com' },
  { id: 3, name: '', email: 'ana@example.com' },
  { id: 4, name: 'Joan Park', email: 'joan@example.com' },
]

function Box({ onSubmit = () => undefined }: { onSubmit?: () => void }) {
  const [v, setV] = useState('')
  return <MentionBox value={v} people={people} label="Comment" placeholder="" onChange={setV} onSubmit={onSubmit} />
}

describe('mentions in the comment box', () => {
  it('finds the word after @', () => {
    expect(mentionQuery('hi @jo', 6)).toEqual({ start: 3, query: 'jo' })
    expect(mentionQuery('mail@jo', 7)).toBeNull()
    expect(mentionQuery('@', 1)).toEqual({ start: 0, query: '' })
  })

  it('suggests people who can see the card and inserts the name', () => {
    render(<Box />)
    const box = screen.getByLabelText('Comment') as HTMLTextAreaElement
    fireEvent.change(box, { target: { value: 'thanks @jo' } })
    const list = screen.getByRole('listbox', { name: 'People to mention' })
    expect(within(list).getAllByRole('option').map((o) => o.textContent)).toEqual(['JLJo Lee', 'JPJoan Park'])
    fireEvent.keyDown(box, { key: 'ArrowDown' })
    fireEvent.keyDown(box, { key: 'Enter' })
    expect(box.value).toBe('thanks @Joan Park ')
    expect(screen.queryByRole('listbox')).toBeNull()
    fireEvent.change(box, { target: { value: 'thanks @Joan Park and @an' } })
    fireEvent.click(screen.getByRole('option', { name: /ana@example.com/ }))
    expect(box.value).toBe('thanks @Joan Park and @ana@example.com ')
  })

  it('sends with Ctrl+Enter and Escape closes the list', () => {
    const sent = vi.fn()
    render(<Box onSubmit={sent} />)
    const box = screen.getByLabelText('Comment') as HTMLTextAreaElement
    fireEvent.change(box, { target: { value: '@j' } })
    fireEvent.keyDown(box, { key: 'Escape' })
    expect(screen.queryByRole('listbox')).toBeNull()
    fireEvent.keyDown(box, { key: 'Enter', ctrlKey: true })
    expect(sent).toHaveBeenCalled()
  })
})
