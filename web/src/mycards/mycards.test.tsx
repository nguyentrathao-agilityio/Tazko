import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'
import { groupCards } from '@/mycards/groups'
import type { MyCard } from '@/mycards/types'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
const NOW = 1_800_000_000
const DAY = 86_400
const me = {
  id: 1,
  email: 'mai@example.com',
  name: 'Mai Tran',
  is_super_admin: false,
  csrf_token: 'c'.repeat(64),
  companies: [
    { slug: 'acme', name: 'Acme Studio', role: 'member' },
    { slug: 'north', name: 'Northwind', role: 'member' },
  ],
}
const acme = { slug: 'acme', name: 'Acme Studio' }
const mine = (id: number, title: string, extra: Partial<MyCard> = {}): MyCard => ({
  id,
  title,
  done: false,
  due_at: null,
  list: 'To do',
  board: { id: 7, name: 'Sprint 15' },
  project: { id: 4, name: 'Website' },
  items: 0,
  items_done: 0,
  can_edit: true,
  labels: [],
  company: acme,
  ...extra,
})
const here = [
  mine(21, 'Write copy', { due_at: NOW - DAY, labels: [{ id: 31, name: 'Bug', color: 3 }] }),
  mine(22, 'Audit nav', { due_at: NOW + 3600, items: 3, items_done: 1 }),
  mine(23, 'Plan launch', { project: { id: 5, name: 'Ops' }, board: { id: 8, name: 'Ops board' } }),
  mine(24, 'Choose fonts', { done: true, can_edit: false }),
]
const there = mine(31, 'Lab report', { company: { slug: 'north', name: 'Northwind' }, board: { id: 2, name: 'Lab' }, project: { id: 1, name: 'Labs' }, due_at: NOW + 3 * DAY })

const item = {
  id: 91,
  text: 'Write release notes',
  done: false,
  due_at: NOW + 3 * DAY,
  checklist: 5,
  card: { id: 23, title: 'Plan launch' },
  board: { id: 8, name: 'Ops board' },
  project: { id: 5, name: 'Ops' },
  can_edit: true,
  company: acme,
}

let calls: Call[] = []

beforeEach(() => {
  calls = []
  vi.useFakeTimers({ toFake: ['Date'], now: NOW * 1000 })
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined })
      if (url === '/api/v1/me') return json(me)
      if (url === '/api/v1/me/cards?company=acme') return json({ cards: here, items: [item] })
      if (url === '/api/v1/me/cards') return json({ cards: [...here, there], items: [item] })
      if (method === 'PATCH') return new Response(null, { status: 204 })
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

function open() {
  window.history.pushState(null, '', '/c/acme/my-cards')
  render(
    <Toaster>
      <App />
    </Toaster>,
  )
}

const groups = () => [...document.querySelectorAll('.sechd')].map((h) => h.textContent)

describe('my cards groups', () => {
  it('orders groups by urgency and puts done cards last', () => {
    expect(groupCards([...here, there], NOW).map(([g, l]) => [g, l.map((c) => c.id)])).toEqual([
      ['overdue', [21]],
      ['day', [22]],
      ['week', [31]],
      ['none', [23]],
      ['done', [24]],
    ])
  })
})

describe('my cards page', () => {
  it('shows my cards in this company, grouped by due date', async () => {
    open()
    expect(await screen.findByText('Write copy')).toBeTruthy()
    expect(screen.getByRole('link', { name: 'My cards' }).className).toContain('on')
    expect(screen.getByRole('tab', { name: 'Assigned to me (4)' })).toBeTruthy()
    expect(groups()).toEqual(['Overdue · 1', 'Next 24 hours · 1', 'No date · 1', 'Completed · 1'])
    expect(within(screen.getByText('Write copy').closest('.mc') as HTMLElement).getByText('Website › Sprint 15 › To do')).toBeTruthy()
    expect(screen.queryByText('Lab report')).toBeNull()
    expect((within(screen.getByText('Choose fonts').closest('.mc') as HTMLElement).getByRole('checkbox') as HTMLButtonElement).disabled).toBe(true)
  })

  it('filters by title, project and completion', async () => {
    open()
    await screen.findByText('Write copy')
    fireEvent.click(screen.getByLabelText('Hide completed'))
    expect(screen.queryByText('Choose fonts')).toBeNull()
    fireEvent.change(screen.getByLabelText('Project'), { target: { value: 'acme:5' } })
    expect(groups()).toEqual(['No date · 1'])
    fireEvent.change(screen.getByLabelText('Project'), { target: { value: '' } })
    fireEvent.change(screen.getByLabelText('Filter by title'), { target: { value: 'audit' } })
    expect(groups()).toEqual(['Next 24 hours · 1'])
  })

  it('takes in every company with the switch and opens cards in their own company', async () => {
    open()
    await screen.findByText('Write copy')
    fireEvent.click(screen.getByLabelText('All companies'))
    const row = (await screen.findByText('Lab report')).closest('.mc') as HTMLElement
    expect(within(row).getByText('Northwind › Labs › Lab › To do')).toBeTruthy()
    expect(screen.getByText(/in all your companies/)).toBeTruthy()
    fireEvent.click(row)
    await waitFor(() => expect(window.location.pathname).toBe('/c/north/b/2/c/31'))
  })

  it('marks a card complete without opening it', async () => {
    open()
    const row = (await screen.findByText('Audit nav')).closest('.mc') as HTMLElement
    fireEvent.click(within(row).getByRole('checkbox', { name: 'Mark complete' }))
    await waitFor(() => expect(calls.find((c) => c.method === 'PATCH')).toEqual({ url: '/api/v1/c/acme/boards/7/cards/22', method: 'PATCH', body: { done: true } }))
    expect(window.location.pathname).toBe('/c/acme/my-cards')
    fireEvent.keyDown(row, { key: 'Enter' })
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/b/7/c/22'))
  })

  it('lists checklist items assigned to me with their due dates', async () => {
    open()
    await screen.findByText('Write copy')
    fireEvent.click(screen.getByRole('tab', { name: 'Checklist items (1)' }))
    const row = (await screen.findByText('Write release notes')).closest('.mc') as HTMLElement
    expect(within(row).getByText('in Plan launch · Ops board')).toBeTruthy()
    expect(groups()).toEqual(['This week · 1'])
    fireEvent.click(within(row).getByRole('checkbox', { name: 'Mark item done' }))
    await waitFor(() => expect(calls).toContainEqual({ url: '/api/v1/c/acme/boards/8/items/91', method: 'PATCH', body: { done: true } }))
    fireEvent.click(row)
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/b/8/c/23'))
  })
})
