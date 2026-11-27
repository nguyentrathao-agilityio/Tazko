import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown }

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

const me = {
  id: 1,
  email: 'mai@example.com',
  name: 'Mai Tran',
  is_super_admin: false,
  csrf_token: 'c'.repeat(64),
  companies: [{ slug: 'acme', name: 'Acme Studio', role: 'owner' }],
}

const project = (role: string) => ({
  id: 4,
  name: 'Website relaunch',
  description: 'New site',
  color: 'c1',
  background: 'sea',
  members_create_boards: true,
  members_invite: false,
  role,
  member_count: 2,
  can: {
    edit: role !== 'observer',
    manage: role === 'owner',
    invite: role === 'owner',
    create_board: role !== 'observer',
  },
})

let role = 'owner'
let calls: Call[] = []

beforeEach(() => {
  calls = []
  role = 'owner'
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: init.body ? JSON.parse(String(init.body)) : undefined })
      if (url === '/api/v1/me') return json(me)
      if (method !== 'GET') return method === 'POST' ? json({ id: 9, result: 'added' }, 201) : new Response(null, { status: 204 })
      if (url === '/api/v1/c/acme/my-projects')
        return json({
          projects: [
            {
              id: 4,
              name: 'Website relaunch',
              description: 'New site',
              color: 'c1',
              role: 'observer',
              boards: 3,
              members: [{ id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'observer' }],
            },
          ],
        })
      if (url === '/api/v1/c/acme/projects/4') return json(project(role))
      if (url === '/api/v1/c/acme/projects/4/members')
        return json({
          members: [
            { id: 1, name: 'Mai Tran', email: 'mai@example.com', role, last_active: null, you: true },
            { id: 2, name: 'Jo Lee', email: 'jo@gmail.com', role: 'owner', last_active: null, you: false },
          ],
          invites: [{ id: 5, email: 'new@example.com', role: 'member', invited_by: 'Jo Lee', created_at: 1_700_000_000, expires_at: Math.floor(Date.now() / 1000) + 5 * 86_400 + 60 }],
          can: project(role).can,
        })
      if (url === '/api/v1/c/acme/projects')
        return json({
          projects: [{ id: 4, name: 'Website relaunch', status: 'active', owners: [], members: 2, boards: 3 }],
          people: [{ id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'owner' }],
        })
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

describe('projects', () => {
  it('lists the projects a person belongs to and opens one', async () => {
    open('/c/acme')
    fireEvent.click(await screen.findByRole('button', { name: /Website relaunch/ }))
    expect(window.location.pathname).toBe('/c/acme/p/4')
    expect(await screen.findByRole('heading', { name: 'Website relaunch' })).toBeTruthy()
  })

  it('shows observers no settings tab and no invite form', async () => {
    role = 'observer'
    open('/c/acme/p/4/members')
    expect(await screen.findByText('jo@gmail.com')).toBeTruthy()
    expect(screen.queryByRole('tab', { name: 'Settings' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Invite' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Revoke' })).toBeNull()
    expect(screen.queryByLabelText('Role of Jo Lee')).toBeNull()
  })

  it('lets owners invite, change roles and save settings', async () => {
    open('/c/acme/p/4/members')
    expect(await screen.findByText(/expires in 5 days/)).toBeTruthy()
    fireEvent.change(await screen.findByLabelText('Email address'), { target: { value: 'x@example.com' } })
    fireEvent.click(screen.getByRole('button', { name: 'Invite' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'POST')).toBe(true))
    expect(calls.find((c) => c.method === 'POST')).toMatchObject({ url: '/api/v1/c/acme/projects/4/invites', body: { email: 'x@example.com', role: 'member' } })

    fireEvent.change(await screen.findByLabelText('Role of Jo Lee'), { target: { value: 'observer' } })
    await waitFor(() => expect(calls.some((c) => c.method === 'PATCH')).toBe(true))
    expect(calls.find((c) => c.method === 'PATCH')).toMatchObject({ url: '/api/v1/c/acme/projects/4/members/2', body: { role: 'observer' } })

    fireEvent.click(screen.getByRole('tab', { name: 'Settings' }))
    fireEvent.click(await screen.findByRole('switch', { name: 'Members can invite people' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'PATCH' && c.url === '/api/v1/c/acme/projects/4')).toBe(true))
    expect(calls.find((c) => c.url === '/api/v1/c/acme/projects/4' && c.method === 'PATCH')?.body).toEqual({ members_invite: true })
  })

  it('creates and deletes projects from Manage projects', async () => {
    open('/c/acme/projects')
    await screen.findByText('Website relaunch')
    fireEvent.click(screen.getByRole('button', { name: 'New project' }))
    fireEvent.change(screen.getByLabelText('Project name'), { target: { value: 'Mobile app' } })
    fireEvent.click(screen.getByRole('button', { name: 'Create project' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'POST')).toBe(true))
    expect(calls.find((c) => c.method === 'POST')).toMatchObject({ url: '/api/v1/c/acme/projects', body: { name: 'Mobile app', owner_id: 1 } })

    fireEvent.click(await screen.findByRole('button', { name: 'Delete' }))
    const go = screen.getByRole('button', { name: 'Delete project' }) as HTMLButtonElement
    expect(go.disabled).toBe(true)
    fireEvent.change(screen.getByLabelText('Type Website relaunch to confirm'), { target: { value: 'Website relaunch' } })
    fireEvent.click(go)
    await waitFor(() => expect(calls.some((c) => c.method === 'DELETE')).toBe(true))
    expect(calls.find((c) => c.method === 'DELETE')).toMatchObject({ url: '/api/v1/c/acme/projects/4', body: { confirm: 'Website relaunch' } })
  })
})
