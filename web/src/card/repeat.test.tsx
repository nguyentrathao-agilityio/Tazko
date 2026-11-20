import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'
import { DatesPop } from '@/card/Pops'
import { describe as describeEvent } from '@/card/activity'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
const DUE = 1_900_000_000
const me = { id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }] }
const board = () => ({
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
  people: [{ id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'member', access: true }],
  lists: [{ id: 11, title: 'To do', position: 'V', cards: [{ id: 21, title: 'Weekly report', position: 'V', labels: [], members: [], due_at: DUE, recurrence: 'weekly' }, { id: 22, title: 'One-off', position: 'W', labels: [], members: [] }] }],
  labels: [],
  archived_lists: 0,
})

let calls: Call[] = []

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.history.pushState(null, '', '/')
})

describe('repeat in the dates popover', () => {
  it('offers the five rules and saves the choice with the dates', () => {
    const onSave = vi.fn()
    render(<DatesPop start={null} due={DUE} recurrence="weekly" onSave={onSave} />)
    const select = screen.getByLabelText('Repeat') as HTMLSelectElement
    expect([...select.options].map((o) => o.textContent)).toEqual(['Does not repeat', 'Repeats every day', 'Repeats every weekday', 'Repeats every week', 'Repeats every 2 weeks', 'Repeats every month'])
    expect(select.value).toBe('weekly')
    fireEvent.change(select, { target: { value: 'monthly' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    expect(onSave.mock.calls[0][0].recurrence).toBe('monthly')
    fireEvent.change(select, { target: { value: '' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    expect(onSave.mock.calls[1][0].recurrence).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Remove' }))
    expect(onSave.mock.calls[2][0]).toEqual({ start_at: null, due_at: null, recurrence: null })
  })

  it('needs a due date before a card can repeat', () => {
    const onSave = vi.fn()
    render(<DatesPop start={null} due={null} recurrence={null} onSave={onSave} />)
    expect((screen.getByLabelText('Repeat') as HTMLSelectElement).disabled).toBe(true)
  })

  it('describes a repeat in the activity', () => {
    const entry = { id: 1, type: 'card_repeated', payload: { rule: 'weekly', from: DUE, to: DUE + 7 * 86_400 }, created_at: 0, actor: null }
    expect(describeEvent(entry as never, { people: [], labels: [], lists: [] } as never)).toMatch(/^completed this card; it repeats every week, next due /)
  })
})

describe('completing a repeating card', () => {
  beforeEach(() => {
    calls = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init: RequestInit = {}) => {
        const method = init.method ?? 'GET'
        const body = typeof init.body === 'string' ? JSON.parse(init.body) : undefined
        calls.push({ url, method, body })
        if (url === '/api/v1/me') return json(me)
        if (url === '/api/v1/c/acme/boards/7') return json(board())
        if (url === '/api/v1/c/acme/boards/7/cards/21' && method === 'PATCH') return json({ repeated: { rule: 'weekly', from: DUE, to: DUE + 7 * 86_400 } })
        if (method !== 'GET') return new Response(null, { status: 204 })
        return json({ error: 'not found' }, 404)
      }),
    )
    window.history.pushState(null, '', '/c/acme/b/7')
    render(
      <Toaster>
        <App />
      </Toaster>,
    )
  })

  it('shows the repeat badge, says when it is next due and undoes', async () => {
    const tile = (await screen.findByText('Weekly report')).closest('.cd') as HTMLElement
    expect(within(tile).getByTitle('Repeats every week')).toBeTruthy()
    expect(within((screen.getByText('One-off').closest('.cd') as HTMLElement)).queryByTitle(/^Repeats/)).toBeNull()

    const box = within(tile).getByRole('checkbox', { name: 'Mark complete' })
    fireEvent.click(box)
    fireEvent.click(box)
    await waitFor(() => expect(calls).toContainEqual({ url: '/api/v1/c/acme/boards/7/cards/21', method: 'PATCH', body: { done: true, tz_offset: -new Date().getTimezoneOffset() } }))
    expect(calls.filter((c) => c.method === 'PATCH').length).toBe(1)
    expect(await screen.findByText(/^Done\. Repeats every week\. Next due /)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }))
    await waitFor(() => expect(calls).toContainEqual({ url: '/api/v1/c/acme/boards/7/cards/21/repeat/undo', method: 'POST', body: undefined }))
  })

  it('completes a card that does not repeat without a message', async () => {
    const tile = (await screen.findByText('One-off')).closest('.cd') as HTMLElement
    fireEvent.click(within(tile).getByRole('checkbox', { name: 'Mark complete' }))
    await waitFor(() => expect(calls.some((c) => c.url === '/api/v1/c/acme/boards/7/cards/22' && c.method === 'PATCH')).toBe(true))
    expect(screen.queryByText(/^Done\. Repeats/)).toBeNull()
  })
})
