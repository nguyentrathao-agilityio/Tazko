import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'
import type { BoardPerson, Label, ListData } from '@/board/types'
import { boardStats } from '@/dashboard/stats'
import { dueBucket } from '@/card/look'

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
const NOW = 1_800_000_000
const DAY = 86_400
const me = { id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }] }
const card = (id: number, title: string, extra: Record<string, unknown> = {}) => ({ id, title, position: String(id), labels: [], members: [], ...extra })
const people = [
  { id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'member', access: true },
  { id: 2, name: 'Jo Lee', email: 'jo@example.com', role: 'member', access: true },
]
const labels = [
  { id: 31, name: 'Bug', color: 3 },
  { id: 32, name: 'Design', color: 1 },
]
const lists = () => [
  { id: 11, title: 'To do', position: 'V', cards: [card(21, 'Write copy', { due_at: NOW - DAY, labels: [31], members: [1] }), card(22, 'Audit nav', { due_at: NOW + 3600, members: [1, 2] }), card(23, 'Plan', { due_at: NOW + 3 * DAY })] },
  { id: 12, title: 'Doing', position: 'W', cards: [card(24, 'Hero', { due_at: NOW + 30 * DAY, labels: [31, 32] })] },
  { id: 13, title: 'Done', position: 'X', cards: [card(25, 'Choose fonts', { done: true, due_at: NOW - DAY, members: [2] }), card(26, 'Logo', { done: true })] },
]
const board = () => ({ id: 7, name: 'Sprint 15', description: '', background: 'sea', restricted: false, closed: false, starred: false, project: { id: 4, name: 'Website' }, role: 'member', can: { edit: true, manage: false, invite: false, create_board: true }, people, lists: lists(), labels, archived_lists: 0 })

describe('dashboard numbers', () => {
  it('adds up to the board in every chart', () => {
    const ls = lists() as unknown as ListData[]
    const s = boardStats(ls, people as BoardPerson[], labels as Label[], (b) => b, 'Unassigned', NOW)
    const total = ls.reduce((n, l) => n + l.cards.length, 0)
    expect(s.cards).toBe(total)
    expect(s.lists.map((b) => b.value)).toEqual([3, 1, 2])
    expect(s.lists.reduce((n, b) => n + b.value, 0)).toBe(total)
    expect(s.due.reduce((n, b) => n + b.value, 0)).toBe(total)
    expect(Object.fromEntries(s.due.map((b) => [b.key, b.value]))).toEqual({ overdue: 1, day: 1, week: 1, later: 1, none: 0, done: 2 })
    expect(s.done).toBe(2)
    expect(s.overdue).toBe(1)
    expect(s.soon).toBe(1)
    expect(s.members.map((b) => [b.name, b.value])).toEqual([
      ['Mai Tran', 2],
      ['Jo Lee', 2],
      ['Unassigned', 3],
    ])
    expect(s.labels.map((b) => [b.name, b.value])).toEqual([
      ['Bug', 2],
      ['Design', 1],
    ])
  })

  it('puts a done card under complete even when it is late', () => {
    expect(dueBucket(NOW - DAY, true, NOW)).toBe('done')
    expect(dueBucket(null, false, NOW)).toBe('none')
  })
})

describe('dashboard view', () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['Date'], now: NOW * 1000 })
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        if (url === '/api/v1/me') return json(me)
        if (url === '/api/v1/c/acme/boards/7') return json(board())
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

  const value = (section: string, row: string) => {
    const s = screen.getByRole('region', { name: section })
    return within(s).getByRole('img', { name: new RegExp(`^${row}: `) }).getAttribute('aria-label')
  }

  it('shows the board numbers and follows the filter', async () => {
    window.history.pushState(null, '', '/c/acme/b/7/dashboard')
    render(
      <Toaster>
        <App />
      </Toaster>,
    )
    expect(await screen.findByRole('tab', { name: 'Dashboard', selected: true })).toBeTruthy()
    const stat = (name: string) => screen.getByText(name, { selector: '.dstat small' }).nextElementSibling?.textContent
    expect(stat('Cards')).toBe('6')
    expect(stat('Complete')).toBe('2')
    expect(stat('Overdue')).toBe('1')
    expect(stat('Due in 2 days')).toBe('1')
    expect(value('Cards per list', 'To do')).toBe('To do: 3')
    expect(value('Cards per member', 'Unassigned')).toBe('Unassigned: 3')
    expect(value('Cards per label', 'Bug')).toBe('Bug: 2')
    expect(value('Due dates', 'Next 24 hours')).toBe('Next 24 hours: 1')

    fireEvent.keyDown(window, { key: 'q' })
    expect(await screen.findByText('Dashboard uses your current filter (2 cards)')).toBeTruthy()
    expect(stat('Cards')).toBe('2')
    expect(value('Cards per list', 'Done')).toBe('Done: 0')
  })
})
