import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

let superAdmin = false
let calls: Call[] = []
const me = () => ({ id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: superAdmin, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'owner' }] })
const project = { id: 4, name: 'Website relaunch', description: '', color: 'c1', background: 'sea', members_create_boards: true, members_invite: false, role: 'owner', member_count: 2, can: { edit: true, manage: true, invite: true, create_board: true } }
const templates = () => ({
  company: 'Acme Studio',
  can_offer: superAdmin,
  templates: [
    { id: 'c:3', name: 'Product launch', description: 'Lists and starter cards', background: 'sea', admin: false, by: 'Acme Studio', lists: [{ title: 'Ideas', cards: ['Hero copy', 'Pricing page'] }, { title: 'Done', cards: [] }], labels: 9, fields: 1 },
    { id: 'a:1', name: 'Retrospective', description: 'What went well', background: 'rose', admin: true, by: 'Tazko admin', lists: [{ title: 'Went well', cards: [] }], labels: 8, fields: 0 },
  ],
})

beforeEach(() => {
  calls = []
  superAdmin = false
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined })
      if (url === '/api/v1/me') return json(me())
      if (url === '/api/v1/c/acme/projects/4') return json(project)
      if (url === '/api/v1/c/acme/templates') return json(templates())
      if (url === '/api/v1/c/acme/projects/4/boards' && method === 'POST') return json({ id: 42 }, 201)
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

describe('board templates page', () => {
  it('lists company and admin templates, previews one and creates a board from it', async () => {
    open('/c/acme/p/4/templates')
    const launch = await screen.findByRole('article', { name: 'Product launch' })
    expect(within(launch).getByText('2 lists · 2 starter cards · by Acme Studio')).toBeTruthy()
    expect(within(screen.getByRole('article', { name: 'Retrospective' })).getByText('Admin')).toBeTruthy()
    fireEvent.click(screen.getByRole('tab', { name: 'Offered by Tazko admin' }))
    expect(screen.queryByRole('article', { name: 'Product launch' })).toBeNull()
    fireEvent.click(screen.getByRole('tab', { name: 'Acme Studio' }))
    expect(screen.queryByRole('article', { name: 'Retrospective' })).toBeNull()
    fireEvent.click(within(screen.getByRole('article', { name: 'Product launch' })).getByRole('button', { name: 'Preview' }))
    const preview = screen.getByRole('dialog', { name: 'Product launch' })
    expect(within(preview).getByText('Pricing page')).toBeTruthy()
    expect(within(preview).getByText('No starter cards')).toBeTruthy()
    fireEvent.click(within(preview).getByRole('button', { name: 'Close' }))
    fireEvent.click(within(screen.getByRole('article', { name: 'Product launch' })).getByRole('button', { name: 'Use template' }))
    const dialog = screen.getByRole('dialog', { name: 'Create a board from Product launch' })
    fireEvent.change(within(dialog).getByLabelText('Board title'), { target: { value: 'Spring launch' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create board' }))
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/b/42'))
    expect(calls.find((c) => c.method === 'POST')?.body).toEqual({ name: 'Spring launch', template: 'c:3' })
    expect(screen.queryByRole('button', { name: 'Offer to every company' })).toBeNull()
  })

  it('lets a super admin offer a company template to every company', async () => {
    superAdmin = true
    open('/c/acme/p/4/templates')
    const launch = await screen.findByRole('article', { name: 'Product launch' })
    fireEvent.click(within(launch).getByRole('button', { name: 'Offer to every company' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'PUT' && c.url === '/api/v1/c/acme/templates/3/offer')).toBe(true))
    fireEvent.click(within(screen.getByRole('article', { name: 'Retrospective' })).getByRole('button', { name: 'Stop offering' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'DELETE' && c.url === '/api/v1/c/acme/admin-templates/1')).toBe(true))
  })
})
