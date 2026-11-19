import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'
import type { ListData } from '@/board/types'
import { csv, fromDay, nextSort, rows, sortRows } from '@/table/table'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

let calls: Call[] = []
let role = 'member'
const me = { id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }] }
const card = (id: number, title: string, extra: Record<string, unknown> = {}) => ({ id, title, position: String(id), labels: [], members: [], ...extra })
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
  people: [
    { id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'member', access: true },
    { id: 2, name: 'Jo Lee', email: 'jo@example.com', role: 'member', access: true },
  ],
  lists: [
    { id: 11, title: 'To do', position: 'V', cards: [card(21, 'Write copy', { due_at: 1_800_000_000, labels: [31] }), card(22, 'Audit nav', { members: [2] })] },
    { id: 12, title: 'Done', position: 'W', cards: [card(23, 'Choose fonts', { done: true, fields: { '81': 'High' } })] },
  ],
  labels: [{ id: 31, name: 'Bug', color: 3 }],
  fields: [{ id: 81, name: 'Priority', type: 'select', options: ['Low', 'High'], show_on_card: true }],
  archived_lists: 0,
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
      if (method !== 'GET') return method === 'POST' ? json({ cards: [] }, 201) : new Response(null, { status: 204 })
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

const titles = () => [...document.querySelectorAll('.tb .tl')].map((b) => b.textContent)
const writes = () => calls.filter((c) => c.method !== 'GET')

describe('table helpers', () => {
  it('sorts a copy and cycles ascending, descending, off', () => {
    const lists = board().lists as unknown as ListData[]
    const r = rows(lists)
    const s = sortRows(r, { key: 'title', dir: 1 })
    expect(s.map((x) => x.card.title)).toEqual(['Audit nav', 'Choose fonts', 'Write copy'])
    expect(r.map((x) => x.card.title)).toEqual(['Write copy', 'Audit nav', 'Choose fonts'])
    expect(nextSort({ key: null, dir: 1 }, 'title')).toEqual({ key: 'title', dir: 1 })
    expect(nextSort({ key: 'title', dir: 1 }, 'title')).toEqual({ key: 'title', dir: -1 })
    expect(nextSort({ key: 'title', dir: -1 }, 'title')).toEqual({ key: null, dir: 1 })
  })

  it('keeps the time of day when a date changes and writes quoted CSV', () => {
    const at = fromDay('2026-12-01', new Date(2026, 10, 3, 15, 30).getTime() / 1000)!
    expect(new Date(at * 1000).getHours()).toBe(15)
    const b = board() as never
    const text = csv(rows(board().lists as unknown as ListData[]), [{ key: 'title', name: 'Card' }, { key: 'labels', name: 'Labels' }], b)
    expect(text.split('\n')[0]).toBe('"Card","Labels"')
    expect(text.split('\n')[1]).toBe('"Write copy","Bug"')
  })
})

describe('table view', () => {
  it('sorts and groups without changing the board order', async () => {
    open('/c/acme/b/7/table')
    await screen.findByRole('tab', { name: 'Table', selected: true })
    await waitFor(() => expect(titles()).toEqual(['Write copy', 'Audit nav', 'Choose fonts']))
    fireEvent.click(screen.getByRole('columnheader', { name: /Card/ }))
    expect(titles()).toEqual(['Audit nav', 'Choose fonts', 'Write copy'])
    fireEvent.click(screen.getByRole('columnheader', { name: /Card/ }))
    expect(titles()).toEqual(['Write copy', 'Choose fonts', 'Audit nav'])
    fireEvent.keyDown(screen.getByRole('columnheader', { name: /Due/ }), { key: 'Enter' })
    fireEvent.click(screen.getByRole('button', { name: 'Group by list' }))
    expect([...document.querySelectorAll('.tb tr.gh')].map((r) => r.textContent)).toEqual(['To do · 2', 'Done · 1'])
    expect(writes()).toEqual([])
    fireEvent.click(screen.getByRole('tab', { name: 'Board' }))
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/b/7'))
    const lists = await screen.findAllByRole('region')
    expect(within(lists[0]).getAllByRole('button', { name: /Write copy|Audit nav/ }).map((b) => b.getAttribute('aria-label'))).toEqual(['Write copy', 'Audit nav'])
    expect(writes()).toEqual([])
  })

  it('edits in place: due date, list, field and labels', async () => {
    open('/c/acme/b/7/table')
    const due = await screen.findByLabelText('Due date of Write copy')
    fireEvent.change(due, { target: { value: '2026-12-24' } })
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/cards/21') && c.method === 'PATCH')).toBeTruthy())
    const body = calls.find((c) => c.url.endsWith('/cards/21') && c.method === 'PATCH')!.body as { due_at: number }
    expect(new Date(body.due_at * 1000).getDate()).toBe(24)
    fireEvent.change(screen.getByLabelText('List of Audit nav'), { target: { value: '12' } })
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/cards/22/move'))?.body).toEqual({ list_id: 12, after: 23 }))
    fireEvent.change(screen.getByLabelText('Priority: Choose fonts'), { target: { value: 'Low' } })
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/cards/23/fields/81'))?.body).toEqual({ value: 'Low' }))
    fireEvent.click(screen.getByRole('button', { name: 'Edit labels of Audit nav' }))
    fireEvent.click(within(screen.getByRole('dialog', { name: 'Labels' })).getAllByRole('button', { name: /Bug/ })[0])
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/cards/22/labels/31'))?.body).toEqual({ on: true }))
  })

  it('selects rows for bulk actions, chooses columns and copies CSV', async () => {
    const write = vi.fn(async () => undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: write }, configurable: true })
    open('/c/acme/b/7/table')
    fireEvent.click(await screen.findByLabelText('Select Write copy'))
    fireEvent.click(screen.getByLabelText('Select Audit nav'))
    expect(screen.getByText('2 selected')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Assign to me' }))
    await waitFor(() => expect(calls.filter((c) => /\/cards\/2[12]\/members\/1$/.test(c.url)).length).toBe(2))
    fireEvent.click(screen.getByRole('button', { name: 'Columns' }))
    fireEvent.click(within(screen.getByRole('dialog', { name: 'Columns' })).getByLabelText('Labels'))
    expect(screen.queryByRole('columnheader', { name: 'Labels' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Copy as CSV' }))
    await waitFor(() => expect(write).toHaveBeenCalled())
    expect(String((write.mock.calls[0] as unknown[])[0]).split('\n')).toHaveLength(4)
  })

  it('gives observers read-only rows', async () => {
    role = 'observer'
    open('/c/acme/b/7/table')
    await screen.findByText('Write copy')
    expect(screen.queryByLabelText('Due date of Write copy')).toBeNull()
    expect(screen.queryByRole('button', { name: 'Edit labels of Write copy' })).toBeNull()
    expect(screen.queryByLabelText('New card title')).toBeNull()
  })
})
