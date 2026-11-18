import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

const me = { id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }] }

const board = {
  id: 7,
  name: 'Sprint 15',
  description: '',
  background: 'sea',
  restricted: false,
  closed: false,
  starred: false,
  project: { id: 4, name: 'Website' },
  role: 'member',
  can: { edit: true, manage: false, invite: false, create_board: true },
  people: [
    { id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'member', access: true },
    { id: 2, name: 'Jo Lee', email: 'jo@example.com', role: 'member', access: true },
  ],
  lists: [
    {
      id: 11,
      title: 'To do',
      position: 'V',
      cards: [
        { id: 21, title: 'Mine', position: 'V', members: [1], labels: [] },
        { id: 22, title: 'Theirs', position: 'k', members: [2], labels: [31] },
        { id: 23, title: 'Nobody', position: 'r', members: [], labels: [] },
      ],
    },
  ],
  labels: [{ id: 31, name: 'Bug', color: 3 }],
  archived_lists: 0,
}

let urls: string[] = []

beforeEach(() => {
  urls = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string) => {
      urls.push(url)
      if (url === '/api/v1/me') return json(me)
      if (url === '/api/v1/c/acme/boards/7') return json(board)
      if (url.startsWith('/api/v1/c/acme/search'))
        return json({
          boards: [],
          cards: [{ id: 22, title: 'Theirs', board: { id: 7, name: 'Sprint 15', background: 'sea' }, list: 'To do', due_at: null, done: false, archived: false, project: 'Website' }],
          total: 1,
        })
      if (url === '/api/v1/c/acme/boards/7/cards/22') return json({ error: 'not found' }, 404)
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

describe('search and filter', () => {
  it('opens search with / and goes to the chosen card', async () => {
    open('/c/acme/b/7')
    await screen.findByRole('region', { name: 'To do' })
    fireEvent.keyDown(window, { key: '/' })
    const box = screen.getByRole('searchbox', { name: 'Search' })
    fireEvent.change(box, { target: { value: '#bug the' } })
    await waitFor(() => expect(urls.some((u) => u.startsWith('/api/v1/c/acme/search?q=%23bug%20the'))).toBe(true))
    expect(await screen.findByRole('option', { name: /Theirs/ })).toBeTruthy()
    fireEvent.keyDown(box, { key: 'Enter' })
    expect(window.location.pathname).toBe('/c/acme/b/7/c/22')
  })

  it('does not open search while typing', async () => {
    open('/c/acme/b/7')
    const region = await screen.findByRole('region', { name: 'To do' })
    fireEvent.click(within(region).getByRole('button', { name: 'Add a card' }))
    fireEvent.keyDown(within(region).getByLabelText('Card title'), { key: '/' })
    expect(screen.queryByRole('searchbox', { name: 'Search' })).toBeNull()
  })

  it('Q shows only my cards and the filter bar counts them', async () => {
    open('/c/acme/b/7')
    const region = await screen.findByRole('region', { name: 'To do' })
    expect(within(region).getByText('3')).toBeTruthy()
    act(() => {
      fireEvent.keyDown(window, { key: 'q' })
    })
    expect(within(region).queryByRole('button', { name: 'Theirs' })).toBeNull()
    expect(within(region).getByRole('button', { name: 'Mine' })).toBeTruthy()
    expect(within(region).getByText('1/3')).toBeTruthy()
    expect(screen.getByText('cards match your filter', { exact: false }).textContent).toContain('1 cards match your filter')
    fireEvent.click(screen.getByRole('button', { name: 'Clear filter' }))
    expect(within(region).getByRole('button', { name: 'Theirs' })).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    fireEvent.click(screen.getByRole('checkbox', { name: 'Bug' }))
    expect(within(region).getByRole('button', { name: 'Theirs' })).toBeTruthy()
    expect(within(region).queryByRole('button', { name: 'Nobody' })).toBeNull()
  })
})
