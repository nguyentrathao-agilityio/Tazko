import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
let calls: Call[] = []
let emailOn = true
const me = () => ({ id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), email_enabled: emailOn, companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }], settings: {} })
const previews = {
  from: 'Tazko <no-reply@example.com>',
  previews: [
    { kind: 'invite', subject: 'You are invited to Your company on Tazko', to: 'mai@example.com', text: '', html: '<p>Invite body</p>' },
    { kind: 'mention', subject: 'Mai Tran mentioned you on Redesign homepage hero', to: 'mai@example.com', text: '', html: '<p>Mention body</p>' },
  ],
}

beforeEach(() => {
  calls = []
  emailOn = true
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      const body = typeof init.body === 'string' ? JSON.parse(init.body) : undefined
      calls.push({ url, method, body })
      if (url === '/api/v1/me') return json(me())
      if (url === '/api/v1/me/settings') return json(body ?? {})
      if (url === '/api/v1/me/email/previews') return json(previews)
      if (url === '/api/v1/me/email/test') return json({ to: 'mai@example.com' }, 202)
      return json({ error: 'not found' }, 404)
    }),
  )
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.history.pushState(null, '', '/')
})

function open() {
  window.history.pushState(null, '', '/c/acme/me/notifications')
  render(
    <Toaster>
      <App />
    </Toaster>,
  )
}

const saves = () => calls.filter((c) => c.method === 'PATCH' && c.url === '/api/v1/me/settings').map((c) => c.body)

describe('notification settings', () => {
  it('saves app and email choices, keeps due reminders in the app, and sets the daily hour', async () => {
    open()
    expect((await screen.findByRole('tab', { name: 'Notifications' })).getAttribute('aria-selected')).toBe('true')
    fireEvent.click(screen.getByRole('switch', { name: 'Someone mentions you: By email' }))
    await waitFor(() => expect(saves()).toContainEqual({ notify_mention_email: false }))
    expect((screen.getByRole('switch', { name: 'Due-date reminders: In the app' }) as HTMLButtonElement).disabled).toBe(true)
    fireEvent.change(screen.getByLabelText('Send emails'), { target: { value: 'daily' } })
    await waitFor(() => expect(saves().some((b) => (b as { email_frequency?: string }).email_frequency === 'daily')).toBe(true))
    const daily = saves().find((b) => (b as { email_frequency?: string }).email_frequency === 'daily') as { daily_hour_utc: number }
    expect(daily.daily_hour_utc).toBeGreaterThanOrEqual(0)
    fireEvent.change(screen.getByLabelText('Due-date reminder'), { target: { value: '1h' } })
    await waitFor(() => expect(saves()).toContainEqual({ due_reminder: '1h' }))
  })

  it('shows email previews in a sandboxed frame and sends a test', async () => {
    open()
    fireEvent.click(await screen.findByRole('button', { name: 'Preview emails' }))
    const dialog = await screen.findByRole('dialog', { name: 'Email previews' })
    expect(within(dialog).getByText('You are invited to Your company on Tazko')).toBeTruthy()
    fireEvent.click(within(dialog).getByRole('tab', { name: 'Mention' }))
    const frame = within(dialog).getByTitle('Mai Tran mentioned you on Redesign homepage hero') as HTMLIFrameElement
    expect(frame.getAttribute('sandbox')).toBe('')
    expect(frame.getAttribute('srcdoc')).toContain('Mention body')
    fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }))
    fireEvent.click(screen.getByRole('button', { name: 'Send me a test email' }))
    expect(await screen.findByText('Test email on its way to mai@example.com')).toBeTruthy()
  })

  it('says when email is off and turns the email switches off', async () => {
    emailOn = false
    open()
    expect(await screen.findByText(/Email is not set up on this server/)).toBeTruthy()
    const mail = screen.getByRole('switch', { name: 'Someone mentions you: By email' }) as HTMLButtonElement
    expect(mail.disabled && mail.getAttribute('aria-checked') === 'false').toBe(true)
    expect(screen.queryByRole('button', { name: 'Send me a test email' })).toBeNull()
  })
})
