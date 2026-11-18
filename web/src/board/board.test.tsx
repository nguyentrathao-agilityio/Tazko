import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
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
}

let role = 'owner'
let calls: Call[] = []

function board() {
  return {
    id: 7,
    name: 'Sprint 15',
    description: '',
    background: 'sea',
    restricted: true,
    closed: false,
    starred: false,
    project: { id: 4, name: 'Website relaunch' },
    role,
    can: { edit: role !== 'observer', manage: role === 'owner', invite: role === 'owner', create_board: role !== 'observer' },
    people: [
      { id: 1, name: 'Mai Tran', email: 'mai@example.com', role, access: true },
      { id: 2, name: 'Jo Lee', email: 'jo@example.com', role: 'member', access: false },
    ],
    lists: [
      { id: 11, title: 'To do', position: 'V', cards: [{ id: 21, title: 'Write copy', position: 'V' }] },
      { id: 12, title: 'Doing', position: 'k', cards: [] },
    ],
    archived_lists: 0,
  }
}

beforeEach(() => {
  calls = []
  role = 'owner'
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: init.body ? JSON.parse(String(init.body)) : undefined })
      if (url === '/api/v1/me') return json(me)
      if (method !== 'GET') return method === 'POST' ? json({ id: 9 }, 201) : new Response(null, { status: 204 })
      if (url === '/api/v1/c/acme/boards/7') return json(board())
      if (url === '/api/v1/c/acme/boards/7/archive') return json({ lists: [{ id: 13, title: 'Old' }], cards: [] })
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

describe('board', () => {
  it('shows lists, the restricted chip and the people with access', async () => {
    open('/c/acme/b/7')
    expect(await screen.findByRole('button', { name: 'Sprint 15' })).toBeTruthy()
    expect(screen.getByRole('region', { name: 'To do' })).toBeTruthy()
    expect(screen.getByRole('button', { name: /Restricted/ })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Restricted/ }))
    const dialog = screen.getByRole('dialog', { name: 'Board access' })
    expect(within(dialog).getByRole('button', { name: /Jo Lee/ }).getAttribute('aria-pressed')).toBe('false')
    fireEvent.click(within(dialog).getByRole('button', { name: /Jo Lee/ }))
    await waitFor(() => expect(calls.some((c) => c.method === 'PUT')).toBe(true))
    expect(calls.find((c) => c.method === 'PUT')).toMatchObject({ url: '/api/v1/c/acme/boards/7/access', body: { restricted: true, people: [1, 2] } })
  })

  it('renames a list, adds a list and moves a list from its menu', async () => {
    open('/c/acme/b/7')
    fireEvent.click(await screen.findByRole('button', { name: 'Doing' }))
    const input = screen.getByLabelText('List title') as HTMLInputElement
    fireEvent.change(input, { target: { value: 'In progress' } })
    fireEvent.submit(input.form!)
    await waitFor(() => expect(calls.some((c) => c.method === 'PATCH')).toBe(true))
    expect(calls.find((c) => c.method === 'PATCH')).toMatchObject({ url: '/api/v1/c/acme/boards/7/lists/12', body: { title: 'In progress' } })

    fireEvent.click(screen.getByRole('button', { name: 'Add another list' }))
    fireEvent.change(screen.getByPlaceholderText('Enter list title'), { target: { value: 'Review' } })
    fireEvent.click(screen.getByRole('button', { name: 'Add list' }))
    await waitFor(() => expect(calls.some((c) => c.url.endsWith('/lists') && c.method === 'POST')).toBe(true))

    fireEvent.click(within(screen.getByRole('region', { name: 'Doing' })).getByRole('button', { name: 'List actions' }))
    fireEvent.click(screen.getByRole('button', { name: 'Move list' }))
    fireEvent.change(screen.getByLabelText('Position'), { target: { value: '0' } })
    fireEvent.click(screen.getByRole('button', { name: 'Move' }))
    await waitFor(() => expect(calls.some((c) => c.url.endsWith('/move'))).toBe(true))
    expect(calls.find((c) => c.url.endsWith('/move'))?.body).toEqual({ after: null })
  })

  it('gives observers a read-only board', async () => {
    role = 'observer'
    open('/c/acme/b/7')
    expect(await screen.findByText(/You are an observer on this project/)).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Add another list' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'List actions' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Share' })).toBeNull()
  })

  it('restores archived lists from the board menu', async () => {
    open('/c/acme/b/7')
    fireEvent.click(await screen.findByRole('button', { name: 'Board menu' }))
    fireEvent.click(screen.getByRole('button', { name: 'Archived items' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Restore' }))
    await waitFor(() => expect(calls.some((c) => c.url === '/api/v1/c/acme/boards/7/lists/13/restore')).toBe(true))
  })
  it('makes one card per pasted line', async () => {
    open('/c/acme/b/7')
    const doing = await screen.findByRole('region', { name: 'Doing' })
    fireEvent.click(within(doing).getByRole('button', { name: 'Add a card' }))
    fireEvent.change(within(doing).getByLabelText('Card title'), { target: { value: 'Buy milk\n\nCall Jo\nBook room' } })
    expect(within(doing).getByText('3 lines: one card per line.')).toBeTruthy()
    fireEvent.click(within(doing).getByRole('button', { name: 'Add 3 cards' }))
    await waitFor(() => expect(calls.some((c) => c.url === '/api/v1/c/acme/boards/7/cards')).toBe(true))
    expect(calls.find((c) => c.url === '/api/v1/c/acme/boards/7/cards')?.body).toEqual({ list_id: 12, titles: ['Buy milk', 'Call Jo', 'Book room'] })
  })

  it('renames a card from quick edit and sorts a list from its menu', async () => {
    open('/c/acme/b/7')
    const card = await screen.findByRole('button', { name: 'Write copy' })
    fireEvent.click(within(card).getByRole('button', { name: 'Quick edit' }))
    const box = screen.getByRole('dialog', { name: 'Quick edit' })
    fireEvent.change(within(box).getByLabelText('Card title'), { target: { value: 'Write the copy' } })
    fireEvent.click(within(box).getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'PATCH')).toBe(true))
    expect(calls.find((c) => c.method === 'PATCH')).toMatchObject({ url: '/api/v1/c/acme/boards/7/cards/21', body: { title: 'Write the copy' } })

    fireEvent.click(within(screen.getByRole('region', { name: 'To do' })).getByRole('button', { name: 'List actions' }))
    fireEvent.click(screen.getByRole('button', { name: 'Sort by…' }))
    fireEvent.click(screen.getByRole('button', { name: 'Card name (A to Z)' }))
    await waitFor(() => expect(calls.some((c) => c.url.endsWith('/sort'))).toBe(true))
    expect(calls.find((c) => c.url.endsWith('/sort'))?.body).toEqual({ by: 'name' })
  })
})
