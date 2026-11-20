import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown; csrf?: string }

const me = {
  id: 1,
  email: 'root@example.com',
  name: 'Root Admin',
  is_super_admin: true,
  csrf_token: 'k'.repeat(64),
  companies: [],
}

const detail = {
  slug: 'acme',
  name: 'Acme Studio',
  status: 'active',
  storage_limit: 2 * 1024 ** 3,
  storage_used: 5 * 1024 ** 2,
  boards: 0,
  members: [{ id: 7, name: 'Mai Tran', email: 'mai@acme.example', role: 'owner', last_seen: null, disabled: false }],
  invites: [
    {
      id: 3,
      email: 'jo.lee@gmail.com',
      role: 'member',
      invited_by: 'Root Admin',
      created_at: 1_700_000_000,
      company_slug: 'acme',
      company_name: 'Acme Studio',
    },
  ],
  projects: [],
}

let calls: Call[] = []

function respond(url: string, method: string): Response {
  const json = (b: unknown, status = 200) => new Response(JSON.stringify(b), { status })
  if (url === '/api/v1/me') return json(me)
  if (method !== 'GET') return new Response(null, { status: 204 })
  if (url === '/api/v1/admin/companies')
    return json({
      companies: [
        {
          slug: 'acme',
          name: 'Acme Studio',
          status: 'active',
          members: 1,
          boards: 0,
          storage_used: 5 * 1024 ** 2,
          storage_limit: 2 * 1024 ** 3,
          last_activity: 1_700_000_000,
        },
      ],
    })
  if (url === '/api/v1/admin/companies/acme') return json(detail)
  if (url === '/api/v1/admin/people')
    return json({
      people: [
        { id: 7, name: 'Mai Tran', email: 'mai@acme.example', last_seen: null, is_super_admin: false, disabled: false, companies: ['Acme Studio'] },
      ],
    })
  if (url.startsWith('/api/v1/admin/audit'))
    return json({
      entries: [
        { id: 2, at: 1_700_000_000, action: 'sign_in_refused', kind: 'refused', target: 'jo.lee@gmail.com', detail: {}, actor: null, company: null },
        { id: 1, at: 1_700_000_000, action: 'company_created', kind: 'company', target: 'Acme Studio', detail: {}, actor: 'Root Admin', company: 'Acme Studio' },
      ],
    })
  if (url === '/api/v1/admin/server')
    return json({
      version: '0.1.0',
      uptime_secs: 3700,
      data_bytes: 2 * 1024 ** 2,
      companies: 1,
      public_url: 'http://localhost:8791',
      email: 'off',
      email_server: null,
      email_queue: { pending: 0, sent_24h: 0, failed_24h: 0, last_error: null },
      last_backup: { at: 1_700_000_000, ok: true, snapshot: '20231114-221320', databases: 3, files: 1, bytes: 1024 ** 2, problems: [] },
      last_restore_test: { at: 1_700_000_000, ok: false, snapshot: '20231114-221320', databases: 3, files: 1, bytes: 0, problems: ['orgs/acme/tazko.db: integrity check says corrupt'] },
    })
  return json({ error: 'not found' }, 404)
}

beforeEach(() => {
  calls = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      const headers = (init.headers ?? {}) as Record<string, string>
      calls.push({ url, method, body: init.body ? JSON.parse(String(init.body)) : undefined, csrf: headers['x-csrf-token'] })
      return respond(url, method)
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

describe('admin console', () => {
  it('lists companies with members, storage and status, and opens one', async () => {
    open('/admin/companies')
    const row = (await screen.findByText('Acme Studio')).closest('tr')!
    expect(within(row).getByText('Active')).toBeTruthy()
    expect(within(row).getByText('5.0 MB / 2.0 GB')).toBeTruthy()
    fireEvent.click(row)
    expect(await screen.findByRole('tab', { name: 'Members and invites' })).toBeTruthy()
    expect(window.location.pathname).toBe('/admin/companies/acme')
    expect(screen.getByText('Personal Gmail')).toBeTruthy()
  })

  it('sends writes with the CSRF token', async () => {
    open('/admin/companies/acme')
    fireEvent.click(await screen.findByRole('button', { name: 'Suspend' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'POST')).toBe(true))
    const post = calls.find((c) => c.method === 'POST')!
    expect(post.url).toBe('/api/v1/admin/companies/acme/suspend')
    expect(post.csrf).toBe(me.csrf_token)
  })

  it('only enables Delete company once the short name is typed', async () => {
    open('/admin/companies/acme')
    fireEvent.click(await screen.findByRole('tab', { name: 'Settings' }))
    fireEvent.click(screen.getByRole('button', { name: 'Delete this company' }))
    const dialog = screen.getByRole('dialog')
    const go = within(dialog).getByRole('button', { name: 'Delete company' }) as HTMLButtonElement
    expect(go.disabled).toBe(true)
    fireEvent.change(within(dialog).getByLabelText(/to confirm/), { target: { value: 'Acme Studio' } })
    expect(go.disabled).toBe(true)
    fireEvent.change(within(dialog).getByLabelText(/to confirm/), { target: { value: 'acme' } })
    expect(go.disabled).toBe(false)
    fireEvent.click(go)
    await waitFor(() => expect(calls.find((c) => c.method === 'DELETE')?.body).toEqual({ confirm: 'acme' }))
  })

  it('shows project names and counts only, never boards', async () => {
    open('/admin/companies/acme')
    fireEvent.click(await screen.findByRole('tab', { name: 'Projects' }))
    expect(screen.getByText(/never boards/)).toBeTruthy()
    expect(screen.getByText('No projects yet.')).toBeTruthy()
  })

  it('turns super admin on from the People page', async () => {
    open('/admin/people')
    fireEvent.click(await screen.findByRole('switch', { name: 'Super admin for Mai Tran' }))
    await waitFor(() => expect(calls.find((c) => c.method === 'PATCH')?.body).toEqual({ is_super_admin: true }))
  })

  it('marks refused sign-ins in the audit log', async () => {
    open('/admin/audit')
    expect(await screen.findByText('jo.lee@gmail.com')).toBeTruthy()
    expect(screen.getByText('refused')).toBeTruthy()
    expect(screen.getAllByText('Root Admin').length).toBeGreaterThan(0)
  })

  it('confirms a role change and undoes it', async () => {
    open('/admin/companies/acme')
    fireEvent.change(await screen.findByLabelText('Role: Mai Tran'), { target: { value: 'member' } })
    expect(await screen.findByText('Mai Tran is now member')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }))
    await waitFor(() => expect(calls.filter((c) => c.method === 'PATCH')).toHaveLength(2))
    const [change, undo] = calls.filter((c) => c.method === 'PATCH')
    expect(change.body).toEqual({ role: 'member' })
    expect(undo.body).toEqual({ role: 'owner' })
  })

  it('re-invites when a revoke is undone', async () => {
    open('/admin/companies/acme')
    fireEvent.click(await screen.findByRole('button', { name: 'Revoke' }))
    expect(await screen.findByText('Invite for jo.lee@gmail.com revoked')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'POST' && c.url.endsWith('/invites'))).toBe(true))
    expect(calls.find((c) => c.method === 'POST')!.body).toEqual({ email: 'jo.lee@gmail.com', role: 'member' })
  })

  it('shows the last backup and a failed restore test with its problem', async () => {
    open('/admin/server')
    expect(await screen.findByText(/^OK · /)).toBeTruthy()
    expect(screen.getByText('Passed')).toBeTruthy()
    expect(screen.getByText('Failed')).toBeTruthy()
    expect(screen.getByText(/integrity check says corrupt/)).toBeTruthy()
  })

  it('keeps people who are not super admins out', async () => {
    me.is_super_admin = false
    open('/admin/companies')
    expect(await screen.findByText('Only super admins can open the admin console.')).toBeTruthy()
    me.is_super_admin = true
  })
})
