import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Refused } from '@/auth/Refused'
import { SignIn } from '@/auth/SignIn'

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('SignIn', () => {
  it('shows the prototype wording and the Google button', () => {
    render(<SignIn search="" />)
    expect(screen.getByRole('heading', { name: 'Sign in to Tazko' })).toBeTruthy()
    expect(screen.getByRole('link', { name: 'Continue with Google' }).getAttribute('href')).toBe('/auth/google/start')
    expect(screen.getByText(/Access is by invitation only/)).toBeTruthy()
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it.each([
    ['expired', 'took too long'],
    ['failed', 'did not complete'],
    ['cancelled', 'was cancelled'],
    ['busy', 'Too many sign-in attempts'],
  ])('explains the %s error', (code, text) => {
    render(<SignIn search={`?error=${code}`} />)
    expect(screen.getByRole('alert').textContent).toContain(text)
  })
})

describe('Refused', () => {
  it('names the refused email and offers another account', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ email: 'jo.lee@gmail.com', reason: 'not_invited' }))))
    render(<Refused />)
    expect(await screen.findByRole('heading', { name: 'This account is not invited' })).toBeTruthy()
    expect(await screen.findByText('jo.lee@gmail.com')).toBeTruthy()
    expect(screen.getByText(/invite this exact email address/)).toBeTruthy()
    expect(screen.getByRole('link', { name: 'Try another Google account' }).getAttribute('href')).toBe('/auth/google/start')
  })

  it('explains an expired invitation', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ email: 'x@example.com', reason: 'invite_expired' }))))
    render(<Refused />)
    expect(await screen.findByRole('heading', { name: 'Your invitation has expired' })).toBeTruthy()
    expect(screen.getByText(/invitations last 7 days/)).toBeTruthy()
    expect(screen.getByText(/The invitation for/).textContent).toContain('x@example.com')
    expect(screen.queryByText(/not on the list/)).toBeNull()
  })

  it('explains a disabled account', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ email: 'x@example.com', reason: 'disabled' }))))
    render(<Refused />)
    expect(await screen.findByRole('heading', { name: 'This account is turned off' })).toBeTruthy()
    expect(screen.getByText(/Ask your administrator to turn it back on/)).toBeTruthy()
  })

  it('still renders when the note has expired', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response('', { status: 404 })))
    render(<Refused />)
    expect(await screen.findByText('This Google account is not on the list for Tazko.')).toBeTruthy()
  })
})
