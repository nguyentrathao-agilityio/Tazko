import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

const me = {
  id: 1,
  email: 'mai@example.com',
  name: 'Mai',
  is_super_admin: false,
  csrf_token: 'c'.repeat(64),
  companies: [{ slug: 'acme', name: 'Acme', role: 'owner' }],
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.history.pushState(null, '', '/')
})

describe('App', () => {
  it('sends a signed-out visitor to sign in, keeping where they were going', async () => {
    window.history.pushState(null, '', '/c/acme')
    vi.stubGlobal('fetch', vi.fn(async () => json({ error: 'sign in first' }, 401)))
    render(<App />)
    const link = await screen.findByRole('link', { name: 'Continue with Google' })
    expect(window.location.pathname).toBe('/signin')
    expect(link.getAttribute('href')).toBe('/auth/google/start?next=%2Fc%2Facme')
  })

  it('shows the person and their companies, and signs out with the CSRF token', async () => {
    const fetch = vi.fn(async (url: string) => (url === '/api/v1/me' ? json(me) : new Response(null, { status: 204 })))
    vi.stubGlobal('fetch', fetch)
    render(<App />)
    expect(await screen.findByText('Signed in as mai@example.com')).toBeTruthy()
    expect(screen.getByText('Acme')).toBeTruthy()
    expect(screen.getByText('Owner')).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'Sign out' }))
    await waitFor(() => expect(window.location.pathname).toBe('/signin'))
    const [, init] = fetch.mock.calls.find(([u]) => u === '/auth/signout') as unknown as [string, RequestInit]
    expect(init.method).toBe('POST')
    expect((init.headers as Record<string, string>)['x-csrf-token']).toBe(me.csrf_token)
  })

  it('says when the server cannot be reached', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response('', { status: 502 })))
    render(<App />)
    expect(await screen.findByText('The server cannot be reached.')).toBeTruthy()
  })
})
