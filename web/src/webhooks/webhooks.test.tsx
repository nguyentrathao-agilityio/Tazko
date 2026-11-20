import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
let calls: Call[] = []
const EVENTS = ['card.created', 'card.updated', 'card.moved', 'card.completed', 'card.archived', 'comment.created', 'member.added', 'board.created']
const me = () => ({ id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'owner' }] })
const hooks = [
  { id: 1, url: 'https://hooks.example.com/tazko', events: ['card.created', 'card.moved'], projects: null, project_names: null, active: true, created_at: 1_700_000_000, by: 'Mai Tran', last: { status: 'delivered', code: 200, at: 1_700_000_100 } },
  { id: 2, url: 'https://ci.example.com/hook', events: ['comment.created'], projects: [4], project_names: ['Website'], active: true, created_at: 1_700_000_000, by: 'Mai Tran', last: { status: 'pending', code: 502, at: 1_700_000_200 } },
  { id: 3, url: 'https://old.example.com/hook', events: ['card.created'], projects: null, project_names: null, active: false, created_at: 1_700_000_000, by: 'Mai Tran', last: { status: null, code: null, at: null } },
]
const deliveries = [
  { id: 11, event: 'comment.created', status: 'failed', attempts: 7, code: 502, error: 'receiver answered 502', ms: 5003, created_at: 1_700_000_000, finished_at: 1_700_000_200, next_attempt_at: 1_700_000_200 },
  { id: 10, event: 'comment.created', status: 'delivered', attempts: 1, code: 200, error: null, ms: 98, created_at: 1_699_999_000, finished_at: 1_699_999_000, next_attempt_at: 1_699_999_000 },
]

beforeEach(() => {
  calls = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined })
      if (url === '/api/v1/me') return json(me())
      if (url === '/api/v1/me/settings') return json({})
      if (url === '/api/v1/c/acme/tokens') return json({ tokens: [] })
      if (url === '/api/v1/c/acme/webhooks' && method === 'GET') return json({ webhooks: hooks, events: EVENTS })
      if (url === '/api/v1/c/acme/webhooks' && method === 'POST') return json({ ...hooks[0], id: 9, secret: 'whsec_abc123' }, 201)
      if (url === '/api/v1/c/acme/webhooks/2/deliveries') return json({ deliveries })
      if (url === '/api/v1/c/acme/my-projects') return json({ projects: [{ id: 4, name: 'Website', description: '', color: 'c0', role: 'owner', members: [], boards: 1 }] })
      if (method !== 'GET') return new Response(null, { status: method === 'POST' ? 202 : 204 })
      return json({ error: 'not found' }, 404)
    }),
  )
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.history.pushState(null, '', '/')
})

async function openHooks() {
  window.history.pushState(null, '', '/c/acme/integrations')
  render(
    <Toaster>
      <App />
    </Toaster>,
  )
  fireEvent.click(await screen.findByRole('tab', { name: 'Webhooks' }))
  await screen.findByText('https://hooks.example.com/tazko')
}

describe('webhooks', () => {
  it('shows health, pauses, and lists deliveries with redeliver for failures', async () => {
    await openHooks()
    expect(screen.getByText('Healthy')).toBeTruthy()
    expect(screen.getByText('Failing')).toBeTruthy()
    expect(screen.getByText('Paused')).toBeTruthy()
    expect(screen.getByText(/1 events · Website/)).toBeTruthy()

    fireEvent.click(screen.getAllByRole('switch', { name: 'Active' })[0])
    await waitFor(() => expect(calls.some((c) => c.method === 'PATCH' && c.url === '/api/v1/c/acme/webhooks/1')).toBe(true))
    expect(calls.find((c) => c.method === 'PATCH')?.body).toEqual({ active: false })

    fireEvent.click(screen.getAllByRole('button', { name: 'Deliveries' })[1])
    expect(await screen.findByText('502')).toBeTruthy()
    const again = screen.getAllByRole('button', { name: 'Redeliver' })
    expect(again).toHaveLength(1)
    fireEvent.click(again[0])
    await waitFor(() => expect(calls.some((c) => c.method === 'POST' && c.url === '/api/v1/c/acme/webhooks/2/deliveries/11/redeliver')).toBe(true))
    fireEvent.click(screen.getByRole('button', { name: 'Send test event' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'POST' && c.url === '/api/v1/c/acme/webhooks/2/test')).toBe(true))
  })

  it('adds a webhook and shows the signing secret once', async () => {
    await openHooks()
    fireEvent.click(screen.getByRole('button', { name: 'Add webhook' }))
    const form = await screen.findByRole('dialog', { name: 'Add webhook' })
    for (const box of within(form).getAllByRole('checkbox') as HTMLInputElement[]) if (box.checked) fireEvent.click(box)
    fireEvent.change(within(form).getByLabelText('Address'), { target: { value: 'https://example.com/in' } })
    fireEvent.click(within(form).getByRole('button', { name: 'Add webhook' }))
    expect(await within(form).findByText('Choose at least one event')).toBeTruthy()
    fireEvent.click(within(form).getByLabelText('comment.created'))
    fireEvent.change(within(form).getByLabelText('Projects'), { target: { value: '4' } })
    fireEvent.click(within(form).getByRole('button', { name: 'Add webhook' }))
    const made = await screen.findByRole('dialog', { name: 'Webhook added' })
    expect(within(made).getByText('whsec_abc123')).toBeTruthy()
    expect(calls.find((c) => c.method === 'POST' && c.url === '/api/v1/c/acme/webhooks')?.body).toEqual({ url: 'https://example.com/in', events: ['comment.created'], projects: [4] })
  })
})
