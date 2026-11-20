import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown; csrf?: string }

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

const base = {
  id: 1,
  email: 'mai@example.com',
  name: 'Mai Tran',
  is_super_admin: false,
  csrf_token: 'c'.repeat(64),
}

let me: Record<string, unknown> = {}
let calls: Call[] = []

const settings = {
  members: [{ id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'owner', last_seen: null, disabled: false }],
  invites: [],
  storage_used: 1024 ** 2,
  storage_limit: 2 * 1024 ** 3,
}

beforeEach(() => {
  calls = []
  me = {
    ...base,
    companies: [
      { slug: 'acme', name: 'Acme Studio', role: 'owner' },
      { slug: 'north', name: 'Northwind Labs', role: 'member' },
    ],
  }
  try {
    window.localStorage.clear()
  } catch {
    return
  }
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      const headers = (init.headers ?? {}) as Record<string, string>
      calls.push({ url, method, body: init.body ? JSON.parse(String(init.body)) : undefined, csrf: headers['x-csrf-token'] })
      if (url === '/api/v1/me') return json(me)
      if (method !== 'GET') return json({ result: 'invited' }, 201)
      if (url === '/api/v1/c/acme/settings') return json(settings)
      if (url === '/api/v1/c/acme/projects') return json({ projects: [], people: [] })
      if (url.endsWith('/my-projects')) return json({ projects: [] })
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

describe('App', () => {
  it('sends a signed-out visitor to sign in, keeping where they were going', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => json({ error: 'sign in first' }, 401)))
    open('/c/acme/settings')
    const link = await screen.findByRole('link', { name: 'Continue with Google' })
    expect(window.location.pathname).toBe('/signin')
    expect(link.getAttribute('href')).toBe('/auth/google/start?next=%2Fc%2Facme%2Fsettings')
  })

  it('opens the first company on the company home', async () => {
    open('/')
    expect(await screen.findByText('Acme Studio · you see the projects you belong to')).toBeTruthy()
    expect(window.location.pathname).toBe('/c/acme')
    expect(await screen.findByRole('heading', { name: 'You are not in a project yet' })).toBeTruthy()
  })

  it('switches company from the switcher and remembers the choice', async () => {
    open('/c/acme')
    fireEvent.click(await screen.findByRole('button', { name: 'Switch company' }))
    fireEvent.click(within(screen.getByRole('menu')).getByRole('menuitem', { name: /Northwind Labs/ }))
    expect(await screen.findByText('Northwind Labs · you see the projects you belong to')).toBeTruthy()
    expect(screen.getByRole('status').textContent).toContain('Switched to Northwind Labs')
    expect(window.location.pathname).toBe('/c/north')
    expect(screen.queryByRole('link', { name: 'Company settings' })).toBeNull()
    cleanup()

    open('/')
    expect(await screen.findByText('Northwind Labs · you see the projects you belong to')).toBeTruthy()
  })

  it('shows owners their settings and keeps members out', async () => {
    open('/c/north/settings')
    expect(await screen.findByText('Only company owners can open this page')).toBeTruthy()
    expect(calls.some((c) => c.url.includes('/north/settings'))).toBe(false)
    cleanup()

    open('/c/acme/settings')
    expect(await screen.findByText('Acme Studio · you manage members and invites here as an owner')).toBeTruthy()
    fireEvent.change(await screen.findByLabelText('Email address'), { target: { value: 'new@example.com' } })
    fireEvent.click(screen.getByRole('button', { name: 'Invite' }))
    await waitFor(() => expect(calls.find((c) => c.method === 'POST')).toBeTruthy())
    const post = calls.find((c) => c.method === 'POST')!
    expect(post.url).toBe('/api/v1/c/acme/invites')
    expect(post.body).toEqual({ email: 'new@example.com', role: 'member', send_email: false })
    expect(post.csrf).toBe(base.csrf_token)
  })

  it('shows the manage projects page to owners', async () => {
    open('/c/acme/projects')
    expect(await screen.findByText(/No projects yet. Create the first one./)).toBeTruthy()
    expect((screen.getByRole('button', { name: 'New project' }) as HTMLButtonElement).disabled).toBe(false)
  })

  it('says when a company is not available', async () => {
    open('/c/elsewhere')
    expect(await screen.findByText('This company is not available')).toBeTruthy()
  })

  it('explains what to do with no company, and offers the console to super admins', async () => {
    me = { ...base, is_super_admin: true, companies: [] }
    open('/')
    expect(await screen.findByText('You are not in any company yet')).toBeTruthy()
    expect(screen.getByRole('link', { name: 'Admin console' })).toBeTruthy()
  })

  it('signs out with the CSRF token', async () => {
    open('/c/acme')
    fireEvent.click(await screen.findByRole('button', { name: 'Sign out' }))
    await waitFor(() => expect(window.location.pathname).toBe('/signin'))
    const out = calls.find((c) => c.url === '/auth/signout')!
    expect(out.method).toBe('POST')
    expect(out.csrf).toBe(base.csrf_token)
  })

  it('says when the server cannot be reached', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response('', { status: 502 })))
    open('/')
    expect(await screen.findByText('The server cannot be reached.')).toBeTruthy()
  })
})
