import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
let role = 'owner'
let calls: Call[] = []
const me = () => ({ id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role }] })
const personal = [{ id: 3, name: 'laptop', scope: 'write', projects: null, created_at: 1_700_000_000, expires_at: null, last_used_at: null }]
const company = [{ id: 7, name: 'CI', scope: 'read', projects: [4], created_at: 1_700_000_000, expires_at: 1_800_000_000, last_used_at: 1_700_100_000, by: 'Mai Tran' }]

beforeEach(() => {
  calls = []
  role = 'owner'
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined })
      if (url === '/api/v1/me') return json(me())
      if (url === '/api/v1/me/settings') return json({})
      if (url === '/api/v1/me/tokens' && method === 'GET') return json({ tokens: personal })
      if (url === '/api/v1/me/tokens' && method === 'POST') return json({ id: 9, token: 'tzk_p_secret123' }, 201)
      if (url === '/api/v1/c/acme/tokens' && method === 'GET') return json({ tokens: company })
      if (url === '/api/v1/c/acme/tokens' && method === 'POST') return json({ id: 10, token: 'tzk_c_acme_secret456' }, 201)
      if (url === '/api/v1/c/acme/my-projects') return json({ projects: [{ id: 4, name: 'Website', description: '', color: 'c0', role: 'owner', members: [], boards: 1 }, { id: 5, name: 'Ops', description: '', color: 'c1', role: 'owner', members: [], boards: 1 }] })
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

describe('access tokens', () => {
  it('lists, creates with the secret shown once, and revokes personal tokens', async () => {
    open('/c/acme/me/tokens')
    expect(await screen.findByText('laptop')).toBeTruthy()
    expect(screen.getByRole('tab', { name: 'Access tokens' }).getAttribute('aria-selected')).toBe('true')
    expect(screen.getByText('Read and write cards')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'New token' }))
    const form = await screen.findByRole('dialog', { name: 'New personal token' })
    expect(within(form).queryByLabelText('Projects')).toBeNull()
    fireEvent.change(within(form).getByLabelText('Name'), { target: { value: 'release script' } })
    fireEvent.change(within(form).getByLabelText('Expires'), { target: { value: '0' } })
    fireEvent.click(within(form).getByRole('button', { name: 'Create token' }))
    const shown = await screen.findByRole('dialog', { name: 'Token created' })
    expect(within(shown).getByText('tzk_p_secret123')).toBeTruthy()
    expect(calls.find((c) => c.method === 'POST')?.body).toEqual({ name: 'release script', scope: 'read' })
    fireEvent.click(within(shown).getByRole('button', { name: 'Done' }))
    expect(screen.queryByText('tzk_p_secret123')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Revoke' }))
    await waitFor(() => expect(calls).toContainEqual({ url: '/api/v1/me/tokens/3', method: 'DELETE', body: undefined }))
  })

  it('gives company owners API tokens for chosen projects under Integrations', async () => {
    open('/c/acme')
    fireEvent.click(await screen.findByRole('link', { name: 'Integrations' }))
    expect(await screen.findByText('CI')).toBeTruthy()
    expect(screen.getByText(/by Mai Tran/)).toBeTruthy()
    expect(screen.getAllByText('Website').length).toBeGreaterThan(0)
    fireEvent.click(screen.getByRole('button', { name: 'New token' }))
    const form = await screen.findByRole('dialog', { name: 'New company token' })
    fireEvent.change(within(form).getByLabelText('Name'), { target: { value: 'Dashboard sync' } })
    fireEvent.change(within(form).getByLabelText('Projects'), { target: { value: '5' } })
    fireEvent.change(within(form).getByLabelText('Access'), { target: { value: 'write' } })
    fireEvent.click(within(form).getByRole('button', { name: 'Create token' }))
    expect(await screen.findByText('tzk_c_acme_secret456')).toBeTruthy()
    expect(calls.find((c) => c.method === 'POST')?.body).toEqual({ name: 'Dashboard sync', scope: 'write', projects: [5], expires_in_days: 90 })
  })

  it('keeps Integrations to company owners', async () => {
    role = 'member'
    open('/c/acme/integrations')
    expect(await screen.findByText('Integrations are for company owners')).toBeTruthy()
    expect(screen.queryByRole('link', { name: 'Integrations' })).toBeNull()
    expect(calls.some((c) => c.url === '/api/v1/c/acme/tokens')).toBe(false)
  })
})
