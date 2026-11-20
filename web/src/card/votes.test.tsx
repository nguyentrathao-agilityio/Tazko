import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
const me = { id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }] }
let role = 'member'
let voting = true
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
  voting,
  role,
  can: { edit: role !== 'observer', manage: false, invite: false, create_board: true },
  people: [
    { id: 1, name: 'Mai Tran', email: 'mai@example.com', role, access: true },
    { id: 2, name: 'Jo Lee', email: 'jo@example.com', role: 'member', access: true },
  ],
  lists: [
    {
      id: 11,
      title: 'To do',
      position: 'V',
      cards: [
        { id: 21, title: 'Dark mode', position: 'V', labels: [], members: [], votes: 3, voted: true },
        { id: 22, title: 'Export CSV', position: 'W', labels: [], members: [], votes: 1, voted: false },
        { id: 23, title: 'Nobody wants this', position: 'X', labels: [], members: [] },
      ],
    },
  ],
  labels: [],
  archived_lists: 0,
})
const card = () => ({
  id: 22,
  title: 'Export CSV',
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
  links: [],
  voted: false,
  voters: [{ id: 2, name: 'Jo Lee', email: 'jo@example.com' }],
})

beforeEach(() => {
  calls = []
  role = 'member'
  voting = true
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined })
      if (url === '/api/v1/me') return json(me)
      if (url === '/api/v1/c/acme/boards/7') return json(board())
      if (url === '/api/v1/c/acme/boards/7/cards/22') return json(card())
      if (method !== 'GET') return new Response(null, { status: 204 })
      return json({ error: 'not found' }, 404)
    }),
  )
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.sessionStorage.clear()
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

const tileOf = (title: string) => screen.getByText(title).closest('.cd') as HTMLElement
const votes = () => calls.filter((c) => c.url.endsWith('/vote'))

describe('voting', () => {
  it('votes from the tile, and voting again removes the vote', async () => {
    open('/c/acme/b/7')
    await screen.findByText('Dark mode')
    const mine = within(tileOf('Dark mode')).getByRole('button', { name: 'Remove my vote' })
    expect(mine.textContent).toBe('3')
    expect(mine.className).toContain('me')
    fireEvent.click(mine)
    fireEvent.click(within(tileOf('Export CSV')).getByRole('button', { name: 'Vote' }))
    await waitFor(() => expect(votes().map((c) => [c.url, c.body])).toEqual([
      ['/api/v1/c/acme/boards/7/cards/21/vote', { on: false }],
      ['/api/v1/c/acme/boards/7/cards/22/vote', { on: true }],
    ]))
    expect(within(tileOf('Nobody wants this')).queryByRole('button', { name: /vote/i })).toBeNull()
  })

  it('a quick second click takes the vote back, and Space on the badge votes instead of completing', async () => {
    open('/c/acme/b/7')
    await screen.findByText('Export CSV')
    const badge = within(tileOf('Export CSV')).getByRole('button', { name: 'Vote' })
    fireEvent.click(badge)
    await waitFor(() => expect(votes().length).toBe(1))
    fireEvent.click(badge)
    await waitFor(() => expect(votes().map((c) => c.body)).toEqual([{ on: true }, { on: false }]))
    badge.focus()
    fireEvent.keyDown(badge, { key: ' ' })
    expect(calls.some((c) => c.method === 'PATCH')).toBe(false)
  })

  it('votes with V on the selected card and from the quick menu', async () => {
    open('/c/acme/b/7')
    await screen.findByText('Dark mode')
    fireEvent.keyDown(window, { key: 'j' })
    fireEvent.keyDown(window, { key: 'v' })
    await waitFor(() => expect(votes()[0]).toEqual({ url: '/api/v1/c/acme/boards/7/cards/21/vote', method: 'PUT', body: { on: false } }))
    fireEvent.keyDown(window, { key: 'j' })
    fireEvent.keyDown(window, { key: 'e' })
    const quick = await screen.findByRole('dialog', { name: 'Quick edit' })
    fireEvent.click(within(quick).getByRole('button', { name: /^Vote/ }))
    await waitFor(() => expect(votes()[1]).toEqual({ url: '/api/v1/c/acme/boards/7/cards/22/vote', method: 'PUT', body: { on: true } }))
  })

  it('shows who voted on the card and votes from there', async () => {
    open('/c/acme/b/7/c/22')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    const btn = await within(dialog).findByRole('button', { name: /^Vote · 1/ })
    expect(within(dialog).getByTitle('Jo Lee')).toBeTruthy()
    fireEvent.click(btn)
    await waitFor(() => expect(votes()[0]?.body).toEqual({ on: true }))
  })

  it('agrees on the tile, in the table and on the dashboard', async () => {
    open('/c/acme/b/7/table')
    await screen.findByText('Dark mode')
    const cells = [...document.querySelectorAll('.tb tbody tr')].map((r) => [r.querySelector('.tl')?.textContent, r.querySelector('.votebtn')?.textContent])
    expect(cells.filter((c) => c[0])).toEqual([
      ['Dark mode', '3'],
      ['Export CSV', '1'],
      ['Nobody wants this', '0'],
    ])
    fireEvent.click(screen.getByRole('columnheader', { name: /Votes/ }))
    fireEvent.click(screen.getByRole('columnheader', { name: /Votes/ }))
    expect([...document.querySelectorAll('.tb .tl')].map((x) => x.textContent)[0]).toBe('Dark mode')
    cleanup()
    open('/c/acme/b/7/dashboard')
    const most = await screen.findByRole('region', { name: 'Most votes' })
    expect([...most.querySelectorAll('.hb')].map((r) => [r.querySelector('.n')?.textContent, r.querySelector('.v')?.textContent])).toEqual([
      ['Dark mode', '3'],
      ['Export CSV', '1'],
    ])
  })

  it('hides voting everywhere when the project turns it off', async () => {
    voting = false
    open('/c/acme/b/7')
    await screen.findByText('Dark mode')
    expect(within(tileOf('Dark mode')).queryByRole('button', { name: /vote/i })).toBeNull()
    fireEvent.keyDown(window, { key: 'j' })
    fireEvent.keyDown(window, { key: 'v' })
    expect(votes()).toEqual([])
    cleanup()
    open('/c/acme/b/7/dashboard')
    await screen.findByRole('region', { name: 'Cards per list' })
    expect(screen.queryByRole('region', { name: 'Most votes' })).toBeNull()
  })

  it('lets observers see votes without voting', async () => {
    role = 'observer'
    open('/c/acme/b/7')
    await screen.findByText('Dark mode')
    expect((within(tileOf('Dark mode')).getByRole('button', { name: 'Remove my vote' }) as HTMLButtonElement).disabled).toBe(true)
  })
})
