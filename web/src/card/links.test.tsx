import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
const me = { id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }] }
let role = 'member'
let calls: Call[] = []
const board = () => ({
  id: 7,
  name: 'Sprint 15',
  description: '',
  background: 'sea',
  restricted: false,
  closed: false,
  starred: false,
  project: { id: 4, name: 'Website' },
  role,
  can: { edit: role !== 'observer', manage: false, invite: false, create_board: true },
  people: [{ id: 1, name: 'Mai Tran', email: 'mai@example.com', role, access: true }],
  lists: [
    {
      id: 11,
      title: 'To do',
      position: 'V',
      cards: [
        { id: 21, title: 'Build UI', position: 'V', labels: [], members: [], links: 2, blocked: true },
        { id: 22, title: 'Write docs', position: 'W', labels: [], members: [], links: 1, blocked: false },
        { id: 23, title: 'Plain', position: 'X', labels: [], members: [] },
      ],
    },
  ],
  labels: [],
  archived_lists: 0,
})
const card = () => ({
  id: 21,
  title: 'Build UI',
  description: '',
  start_at: null,
  due_at: null,
  done: false,
  cover: null,
  cover_image: null,
  attachments: [],
  list: { id: 11, title: 'To do' },
  archived: false,
  created_at: 1_700_000_000,
  created_by: null,
  labels: [],
  members: [],
  checklists: [],
  comments: [],
  activity: [],
  can: { edit: role !== 'observer' },
  links: [
    { card: { id: 31, title: 'Build API', done: false, archived: false }, board: { id: 8, name: 'Backend' }, list: 'Doing', kind: 'blocked_by' },
    { card: { id: 22, title: 'Write docs', done: true, archived: false }, board: { id: 7, name: 'Sprint 15' }, list: 'To do', kind: 'relates' },
  ],
})

beforeEach(() => {
  calls = []
  role = 'member'
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined })
      if (url === '/api/v1/me') return json(me)
      if (url === '/api/v1/c/acme/boards/7') return json(board())
      if (url === '/api/v1/c/acme/boards/7/cards/21') return json(card())
      if (url.startsWith('/api/v1/c/acme/boards/7/cards/21/link-targets')) return json({ cards: [{ card: { id: 23, title: 'Plain' }, board: { id: 7, name: 'Sprint 15' }, list: 'To do' }] })
      if (method !== 'GET') return new Response(null, { status: 204 })
      return json({ error: 'not found' }, 404)
    }),
  )
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.history.pushState(null, '', '/')
})

function open(path: string) {
  window.history.pushState(null, '', path)
  render(
    <Toaster>
      <App />
    </Toaster>,
  )
}

describe('card links', () => {
  it('shows Blocked on a blocked tile and a link count on others', async () => {
    open('/c/acme/b/7')
    const ui = (await screen.findByText('Build UI')).closest('.cd') as HTMLElement
    expect(within(ui).getByTitle('Blocked by an open card').textContent).toBe('Blocked')
    const docs = screen.getByText('Write docs').closest('.cd') as HTMLElement
    expect(within(docs).getByTitle('Linked cards').textContent).toBe('1')
    expect(within(screen.getByText('Plain').closest('.cd') as HTMLElement).queryByTitle('Linked cards')).toBeNull()
  })

  it('lists links on the card, links another card and removes a link', async () => {
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    const section = await within(dialog).findByRole('region', { name: 'Linked cards' })
    const rows = [...section.querySelectorAll('.lkr')].map((r) => r.textContent)
    expect(rows[0]).toContain('Is blocked by')
    expect(rows[0]).toContain('Backend · Doing')
    expect(rows[1]).toContain('Done')
    expect(section.querySelector('.pill.bad')?.textContent).toBe('Is blocked by')

    fireEvent.click(within(section).getByRole('button', { name: 'Add' }))
    const pop = await screen.findByRole('dialog', { name: 'Link a card' })
    fireEvent.change(within(pop).getByLabelText('This card'), { target: { value: 'blocks' } })
    fireEvent.click(await within(pop).findByRole('button', { name: /Plain/ }))
    await waitFor(() => expect(calls).toContainEqual({ url: '/api/v1/c/acme/boards/7/cards/21/links', method: 'POST', body: { card: 23, kind: 'blocks' } }))

    fireEvent.click(within(section).getByRole('button', { name: 'Remove the link to Write docs' }))
    await waitFor(() => expect(calls).toContainEqual({ url: '/api/v1/c/acme/boards/7/cards/21/links/22', method: 'DELETE', body: undefined }))

    fireEvent.click(within(section).getByRole('button', { name: 'Build API' }))
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/b/8/c/31'))
  })

  it('gives observers links to read but not change', async () => {
    role = 'observer'
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    const section = await within(dialog).findByRole('region', { name: 'Linked cards' })
    expect(within(section).queryByRole('button', { name: 'Add' })).toBeNull()
    expect(within(section).queryByRole('button', { name: /^Remove the link/ })).toBeNull()
  })
})
