import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('App', () => {
  it('shows the server version from /health', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ status: 'ok', version: '0.1.0' }))))
    render(<App />)
    expect(await screen.findByText('Server is running, version 0.1.0.')).toBeTruthy()
  })

  it('says when the server cannot be reached', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response('', { status: 502 })))
    render(<App />)
    expect(await screen.findByText('The server cannot be reached.')).toBeTruthy()
  })
})
