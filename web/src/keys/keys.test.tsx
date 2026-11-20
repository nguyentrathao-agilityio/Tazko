import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
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
  companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }],
  settings: { zen: false, zen_density: 'comfortable', zen_quiet: true },
}

const board = {
  id: 7,
  name: 'Sprint 15',
  description: '',
  background: 'sea',
  restricted: false,
  closed: false,
  starred: false,
  project: { id: 4, name: 'Website' },
  role: 'member',
  can: { edit: true, manage: false, invite: false, create_board: true },
  people: [{ id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'member', access: true }],
  lists: [
    { id: 11, title: 'To do', position: 'V', cards: [{ id: 21, title: 'First', position: 'V', labels: [], members: [] }, { id: 22, title: 'Second', position: 'k', labels: [], members: [] }] },
    { id: 12, title: 'Doing', position: 'k', cards: [{ id: 23, title: 'Third', position: 'V', labels: [], members: [] }] },
  ],
  labels: [{ id: 31, name: 'Bug', color: 3 }],
  archived_lists: 0,
}

let calls: Call[] = []
let stored: Record<string, unknown> = {}

beforeEach(() => {
  calls = []
  stored = { ...me.settings }
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: init.body ? JSON.parse(String(init.body)) : undefined })
      if (url === '/api/v1/me') return json(me)
      if (url === '/api/v1/me/settings') {
        stored = { ...stored, ...(init.body ? JSON.parse(String(init.body)) : {}) }
        return json(stored)
      }
      if (method !== 'GET') return method === 'POST' ? json({ id: 99 }, 201) : new Response(null, { status: 204 })
      if (url === '/api/v1/c/acme/boards/7') return json(board)
      return json({ error: 'not found' }, 404)
    }),
  )
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.history.pushState(null, '', '/')
})

async function openBoard(path = '/c/acme/b/7') {
  window.history.pushState(null, '', path)
  render(
    <Toaster>
      <App />
    </Toaster>,
  )
  await screen.findByRole('region', { name: 'To do' })
}

function press(key: string, target: Document | Element | Window = window) {
  act(() => {
    fireEvent.keyDown(target, { key })
  })
}

function selectedTitle() {
  return document.querySelector('.cd.kf')?.getAttribute('aria-label') ?? null
}

const writes = () => calls.filter((c) => c.method !== 'GET')

describe('keyboard shortcuts', () => {
  it('` and ? show and hide the shortcut list', async () => {
    await openBoard()
    press('`')
    expect(screen.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeTruthy()
    press('`')
    expect(screen.queryByRole('dialog', { name: 'Keyboard shortcuts' })).toBeNull()
    press('?')
    expect(screen.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeTruthy()
  })

  it('J K H L move the selection across cards and lists', async () => {
    await openBoard()
    press('j')
    await waitFor(() => expect(selectedTitle()).toBe('First'))
    press('j')
    await waitFor(() => expect(selectedTitle()).toBe('Second'))
    press('k')
    await waitFor(() => expect(selectedTitle()).toBe('First'))
    press('l')
    await waitFor(() => expect(selectedTitle()).toBe('Third'))
    press('h')
    await waitFor(() => expect(selectedTitle()).toBe('First'))
  })

  it('Enter opens the selected card and J moves to the next card while it is open', async () => {
    await openBoard()
    press('j')
    press('Enter')
    expect(window.location.pathname).toBe('/c/acme/b/7/c/21')
    await screen.findByRole('dialog', { name: 'Card' })
    press('j')
    expect(window.location.pathname).toBe('/c/acme/b/7/c/22')
  })

  it('E Space A D T M X and 1 act on the selected card', async () => {
    await openBoard()
    press('j')
    press('e')
    expect(await screen.findByRole('dialog', { name: 'Quick edit' })).toBeTruthy()
    press('Escape', document.activeElement ?? window)
    for (const [key, title] of [
      ['a', 'Members'],
      ['d', 'Dates'],
      ['t', 'Labels'],
      ['m', 'Move card'],
    ]) {
      press(key)
      expect(screen.getByRole('dialog', { name: title })).toBeTruthy()
      fireEvent.click(within(screen.getByRole('dialog', { name: title })).getByRole('button', { name: 'Close' }))
    }

    press(' ')
    await waitFor(() => expect(writes().some((c) => c.method === 'PATCH' && c.url.endsWith('/cards/21'))).toBe(true))
    expect(writes().find((c) => c.url.endsWith('/cards/21'))?.body).toEqual({ done: true, tz_offset: expect.any(Number) })
    press('1')
    await waitFor(() => expect(writes().some((c) => c.url.endsWith('/cards/21/labels/31'))).toBe(true))
    press('x')
    await waitFor(() => expect(writes().some((c) => c.url.endsWith('/cards/21/archive'))).toBe(true))
  })

  it('N opens the composer in the selected list and F opens the filter', async () => {
    await openBoard()
    press('l')
    press('l')
    press('n')
    const doing = screen.getByRole('region', { name: 'Doing' })
    expect(await within(doing).findByLabelText('Card title')).toBeTruthy()
    fireEvent.click(within(doing).getByRole('button', { name: 'Cancel' }))
    press('f')
    expect(await screen.findByRole('dialog', { name: 'Filter cards' })).toBeTruthy()
  })

  it('Z turns zen on and is remembered, and Esc clears the selection before leaving zen', async () => {
    await openBoard()
    press('z')
    await waitFor(() => expect(document.querySelector('.bd.zen')).toBeTruthy())
    expect(writes().find((c) => c.url === '/api/v1/me/settings')?.body).toEqual({ zen: true })
    expect(screen.getByRole('toolbar', { name: 'Zen controls' })).toBeTruthy()
    fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Card density' })).getByRole('radio', { name: 'Minimal' }))
    await waitFor(() => expect(document.querySelector('.bd.zd-minimal')).toBeTruthy())
    press('j')
    await waitFor(() => expect(selectedTitle()).toBe('First'))
    press('ArrowDown')
    await waitFor(() => expect(selectedTitle()).toBe('Second'))
    press('Escape')
    expect(selectedTitle()).toBeNull()
    expect(document.querySelector('.bd.zen')).toBeTruthy()
    press('Escape')
    await waitFor(() => expect(document.querySelector('.bd.zen')).toBeNull())
  })

  it('no shortcut fires while typing', async () => {
    await openBoard()
    const todo = screen.getByRole('region', { name: 'To do' })
    fireEvent.click(within(todo).getByRole('button', { name: 'Add a card' }))
    const box = within(todo).getByLabelText('Card title')
    const before = writes().length
    for (const key of ['`', '?', 'z', '/', 'f', 'q', 'n', 'j', 'k', 'h', 'l', 'e', ' ', 'a', 'd', 't', 'm', 'x', '1', '8']) {
      press(key, box)
      expect(screen.queryByRole('dialog')).toBeNull()
      expect(selectedTitle()).toBeNull()
    }
    expect(writes().length).toBe(before)
    expect(document.querySelector('.bd.zen')).toBeNull()
    expect(window.location.pathname).toBe('/c/acme/b/7')
    expect(screen.queryByText('cards match your filter', { exact: false })).toBeNull()
  })

  it('no board shortcut fires while a popover is open', async () => {
    await openBoard()
    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    const before = writes().length
    for (const key of ['j', 'z', 'n', 'x', '`']) press(key)
    expect(selectedTitle()).toBeNull()
    expect(writes().length).toBe(before)
    expect(screen.queryByRole('dialog', { name: 'Keyboard shortcuts' })).toBeNull()
  })
})
