import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'
import { ageLevel, isStale } from '@/card/aging'

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
const DAY = 86_400
const NOW = 1_900_000_000
const me = { id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }] }
let aging = true
let commented = false
const card = (id: number, title: string, quiet: number, extra: Record<string, unknown> = {}) => ({ id, title, position: String(id), labels: [], members: [], updated_at: NOW - quiet * DAY, ...extra })
const board = () => ({
  id: 7,
  name: 'Sprint 15',
  description: '',
  background: 'sea',
  restricted: false,
  closed: false,
  starred: false,
  project: { id: 4, name: 'Website' },
  voting: false,
  aging,
  aging_days: 7,
  role: 'member',
  can: { edit: true, manage: false, invite: false, create_board: true },
  people: [{ id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'member', access: true }],
  lists: [
    {
      id: 11,
      title: 'To do',
      position: 'V',
      cards: [card(21, 'Fresh', 1), card(22, 'A week quiet', commented ? 0 : 8), card(23, 'Two weeks quiet', 15), card(24, 'A month quiet', 30), card(25, 'Done long ago', 30, { done: true })],
    },
  ],
  labels: [],
  archived_lists: 0,
})
const detail = () => ({ id: 22, title: 'A week quiet', description: '', start_at: null, due_at: null, done: false, cover: null, cover_image: null, attachments: [], list: { id: 11, title: 'To do' }, archived: false, created_at: NOW - 30 * DAY, created_by: null, labels: [], members: [], checklists: [], comments: [], activity: [], can: { edit: true }, links: [], voters: [] })

describe('aging levels', () => {
  it('fades in three steps and never fades done cards', () => {
    const on = { on: true, days: 7 }
    expect([1, 7, 14, 21].map((d) => ageLevel({ updated_at: NOW - d * DAY }, on, NOW))).toEqual([0, 1, 2, 3])
    expect(ageLevel({ updated_at: NOW - 30 * DAY, done: true }, on, NOW)).toBe(0)
    expect(ageLevel({ updated_at: NOW - 30 * DAY }, { on: false, days: 7 }, NOW)).toBe(0)
    expect(isStale({ updated_at: NOW - 7 * DAY }, 7, NOW)).toBe(true)
    expect(isStale({ updated_at: NOW - 7 * DAY, done: true }, 7, NOW)).toBe(false)
  })
})

describe('aging on the board', () => {
  beforeEach(() => {
    aging = true
    commented = false
    vi.useFakeTimers({ toFake: ['Date'], now: NOW * 1000 })
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init: RequestInit = {}) => {
        const method = init.method ?? 'GET'
        if (url === '/api/v1/me') return json(me)
        if (url === '/api/v1/c/acme/boards/7') return json(board())
        if (url === '/api/v1/c/acme/boards/7/cards/22') return json(detail())
        if (method === 'POST' && url.endsWith('/cards/22/comments')) {
          commented = true
          return json({ id: 1 }, 201)
        }
        if (method !== 'GET') return new Response(null, { status: 204 })
        return json({ error: 'not found' }, 404)
      }),
    )
  })

  afterEach(() => {
    cleanup()
    vi.useRealTimers()
    vi.unstubAllGlobals()
    window.history.pushState(null, '', '/')
  })

  const open = (path: string) => {
    window.history.pushState(null, '', path)
    render(
      <Toaster>
        <App />
      </Toaster>,
    )
  }
  const level = (title: string) => (screen.getAllByText(title).map((e) => e.closest('.cd')).find(Boolean) as HTMLElement).className.match(/age(\d)/)?.[1] ?? '0'

  it('fades quiet cards and a comment brings one back', async () => {
    open('/c/acme/b/7/c/22')
    await screen.findByText('Fresh')
    expect(['Fresh', 'Two weeks quiet', 'A month quiet', 'Done long ago'].map(level)).toEqual(['0', '2', '3', '0'])
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    expect(within(dialog).getByText('Stale · 8 days quiet')).toBeTruthy()
    expect(level('A week quiet')).toBe('1')
    fireEvent.change(await within(dialog).findByLabelText('Comment'), { target: { value: 'Back on it' } })
    fireEvent.click(within(dialog).getAllByRole('button', { name: 'Save' })[0])
    await waitFor(() => expect(level('A week quiet')).toBe('0'))
  })

  it('shows every card at full strength when the project turns aging off', async () => {
    aging = false
    open('/c/acme/b/7')
    await screen.findByText('Fresh')
    expect(['A week quiet', 'Two weeks quiet', 'A month quiet'].map(level)).toEqual(['0', '0', '0'])
  })

  it('filters stale cards and counts them on the dashboard', async () => {
    open('/c/acme/b/7')
    await screen.findByText('Fresh')
    fireEvent.click(screen.getByRole('button', { name: /^Filter/ }))
    fireEvent.click(await screen.findByLabelText('Stale cards (quiet for 7+ days)'))
    await waitFor(() => expect(screen.queryByText('Fresh')).toBeNull())
    expect(screen.getByText('A month quiet')).toBeTruthy()
    expect(screen.queryByText('Done long ago')).toBeNull()
    cleanup()
    open('/c/acme/b/7/dashboard')
    const stale = await screen.findByText('Stale', { selector: '.dstat small' })
    expect(stale.nextElementSibling?.textContent).toBe('3')
  })
})
