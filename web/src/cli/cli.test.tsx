import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
let calls: Call[] = []
let device: { client: string; status: string; email: string } | null = null
const me = () => ({ id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }], settings: {} })

beforeEach(() => {
  calls = []
  device = { client: 'trung-laptop (linux)', status: 'waiting', email: 'mai@example.com' }
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method })
      if (url === '/api/v1/me') return json(me())
      if (url === '/api/v1/me/settings') return json({})
      if (url === '/api/v1/cli/device/WDJB-MJHT' && method === 'GET') return device ? json(device) : json({ error: 'not found' }, 404)
      if (url.startsWith('/api/v1/cli/device/WDJB-MJHT/')) return new Response(null, { status: 204 })
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

describe('command line sign-in', () => {
  it('shows the code and the device, and approves', async () => {
    open('/cli/authorize?code=wdjb-mjht')
    expect(await screen.findByText(/A program on trung-laptop \(linux\) is asking to sign in as you/)).toBeTruthy()
    expect(screen.getByText('WDJB-MJHT')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Authorize' }))
    expect(await screen.findByRole('heading', { name: 'You are signed in' })).toBeTruthy()
    expect(calls.some((c) => c.method === 'POST' && c.url === '/api/v1/cli/device/WDJB-MJHT/approve')).toBe(true)
  })

  it('cancels, and says when a code is unknown', async () => {
    open('/cli/authorize?code=WDJB-MJHT')
    fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }))
    expect(await screen.findByRole('heading', { name: 'Sign-in cancelled' })).toBeTruthy()
    cleanup()
    device = null
    open('/cli/authorize?code=WDJB-MJHT')
    await waitFor(() => expect(screen.getByText(/This code is not known/)).toBeTruthy())
    expect(screen.queryByRole('button', { name: 'Authorize' })).toBeNull()
  })

  it('explains how to install and sign in under Settings', async () => {
    open('/c/acme/me/cli')
    expect((await screen.findByRole('tab', { name: 'Command line' })).getAttribute('aria-selected')).toBe('true')
    expect(screen.getByText(`curl -fsSL ${window.location.origin}/cli/install.sh | sh`)).toBeTruthy()
    expect(screen.getByText(`tazko auth login --host ${window.location.origin}`)).toBeTruthy()
  })
})
