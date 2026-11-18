import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { shortDate } from '@/card/look'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown }

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

let stored: Record<string, unknown> = {}
let calls: Call[] = []

const me = () => ({
  id: 1,
  email: 'mai@example.com',
  name: 'Mai Tran',
  is_super_admin: false,
  csrf_token: 'c'.repeat(64),
  companies: [
    { slug: 'acme', name: 'Acme Studio', role: 'owner' },
    { slug: 'north', name: 'Northwind Labs', role: 'member' },
  ],
  settings: stored,
})

beforeEach(() => {
  calls = []
  stored = { theme: 'system', zoom: 100, date_format: 'MDY', time_format: '12', time_zone: 'UTC' }
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      const body = init.body ? JSON.parse(String(init.body)) : undefined
      calls.push({ url, method, body })
      if (url === '/api/v1/me') return json(me())
      if (url === '/api/v1/me/settings' && method === 'PATCH') {
        stored = { ...stored, ...body }
        return json(stored)
      }
      if (url === '/api/v1/me/sessions')
        return json({
          sessions: [
            { id: 'a'.repeat(16), user_agent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Chrome/130', created_at: 1, last_used_at: 1_700_000_000, current: true },
            { id: 'b'.repeat(16), user_agent: 'Mozilla/5.0 (Windows NT 10.0) Firefox/130', created_at: 1, last_used_at: 1_700_000_000, current: false },
          ],
        })
      if (method !== 'GET') return method === 'POST' ? json({ ended: 1 }) : new Response(null, { status: 204 })
      return json({ projects: [] })
    }),
  )
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.history.pushState(null, '', '/')
  document.documentElement.removeAttribute('data-theme')
  document.documentElement.style.zoom = ''
})

function open(path: string) {
  window.history.pushState(null, '', path)
  render(
    <Toaster>
      <App />
    </Toaster>,
  )
}

describe('personal settings', () => {
  it('applies theme and zoom to the whole app at once and saves them', async () => {
    open('/c/acme/me/appearance')
    fireEvent.click(await screen.findByRole('radio', { name: /Dark/ }))
    await waitFor(() => expect(document.documentElement.getAttribute('data-theme')).toBe('dark'))
    fireEvent.change(screen.getByRole('slider'), { target: { value: '120' } })
    await waitFor(() => expect(document.documentElement.style.zoom).toBe('1.2'))
    expect(calls.filter((c) => c.method === 'PATCH').map((c) => c.body)).toEqual([{ theme: 'dark' }, { zoom: 120 }])
  })

  it('formats dates with the chosen time zone and format', async () => {
    open('/c/acme/me/appearance')
    fireEvent.change(await screen.findByLabelText('Date format'), { target: { value: 'ISO' } })
    fireEvent.change(screen.getByLabelText('Time zone'), { target: { value: 'Asia/Ho_Chi_Minh' } })
    await waitFor(() => expect(stored.time_zone).toBe('Asia/Ho_Chi_Minh'))
    expect(shortDate(Date.UTC(2026, 10, 7, 20, 0) / 1000)).toBe('2026-11-08')
  })

  it('shows the same settings in every company', async () => {
    stored = { ...stored, theme: 'dark', time_format: '24' }
    open('/c/acme/me/appearance')
    expect((await screen.findByRole('radio', { name: /Dark/ })).getAttribute('aria-checked')).toBe('true')
    cleanup()
    open('/c/north/me/appearance')
    expect((await screen.findByRole('radio', { name: /Dark/ })).getAttribute('aria-checked')).toBe('true')
    expect(screen.getByRole('radio', { name: '24-hour' }).getAttribute('aria-checked')).toBe('true')
  })

  it('saves the profile and signs out another session', async () => {
    open('/c/acme/me')
    fireEvent.change(await screen.findByLabelText('Display name'), { target: { value: 'Mai T. Tran' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save profile' }))
    await waitFor(() => expect(calls.some((c) => c.url === '/api/v1/me/profile')).toBe(true))
    expect(calls.find((c) => c.url === '/api/v1/me/profile')?.body).toEqual({ name: 'Mai T. Tran' })

    cleanup()
    open('/c/acme/me/sessions')
    expect(await screen.findByText('Chrome on macOS')).toBeTruthy()
    fireEvent.click(within(screen.getByRole('table')).getByRole('button', { name: 'Sign out' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'DELETE' && c.url === `/api/v1/me/sessions/${'b'.repeat(16)}`)).toBe(true))
  })
})
