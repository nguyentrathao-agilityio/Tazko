import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { Toaster } from '@/ui/Toaster'
import { Palette } from '@/palette/Palette'
import type { Command, Group } from '@/palette/commands'
import { saveSettings } from '@/settings/store'

type Call = { url: string; method: string; body?: unknown }
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
const me = { id: 1, email: 'mai@example.com', name: 'Mai Tran', is_super_admin: false, csrf_token: 'c'.repeat(64), companies: [{ slug: 'acme', name: 'Acme Studio', role: 'member' }] }
const card = (id: number, title: string, extra: Record<string, unknown> = {}) => ({ id, title, position: String(id), labels: [], members: [], ...extra })
const board = () => ({
  id: 7,
  name: 'Sprint 15',
  description: '',
  background: 'sea',
  restricted: false,
  closed: false,
  starred: false,
  project: { id: 4, name: 'Website' },
  role,
  can: { edit: role !== 'observer', manage: false, invite: false, create_board: true },
  people: [{ id: 1, name: 'Mai Tran', email: 'mai@example.com', role: 'member', access: true }],
  lists: [
    { id: 11, title: 'To do', position: 'V', cards: [card(21, 'Write copy'), card(22, 'Audit nav', { members: [1] })] },
    { id: 12, title: 'Done', position: 'W', cards: [card(23, 'Choose fonts', { done: true })] },
  ],
  labels: [],
  archived_lists: 0,
})

let calls: Call[] = []
let role = 'member'

const ctrlK = () => fireEvent.keyDown(window, { key: 'k', ctrlKey: true })
const input = () => screen.getByRole('searchbox', { name: 'Command palette' })
const press = (key: string, n = 1) => {
  for (let i = 0; i < n; i++) fireEvent.keyDown(input(), { key })
}
const options = () => screen.getAllByRole('option').map((o) => o.textContent ?? '')
const writes = () => calls.filter((c) => c.method !== 'GET')

async function choose(label: RegExp) {
  ctrlK()
  const at = options().findIndex((o) => label.test(o))
  expect(at, String(label)).toBeGreaterThanOrEqual(0)
  press('ArrowDown', at)
  press('Enter')
  await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Command palette' })).toBeNull())
  await act(() => new Promise((r) => setTimeout(r, 0)))
}

describe('palette keys', () => {
  afterEach(cleanup)

  it('runs every item with arrows and Enter only', () => {
    const groups: Group[] = ['actions', 'filter', 'density', 'zen', 'card', 'goto']
    const items: Command[] = groups.flatMap((g, i) => [0, 1].map((j) => ({ id: `${g}${j}`, group: g, label: `${g} item ${j}`, keys: j ? String(i) : undefined, run: vi.fn() })))
    vi.useFakeTimers()
    for (let i = 0; i < items.length; i++) {
      const view = render(<Palette items={items} onClose={() => undefined} />)
      press('ArrowDown', i)
      expect(screen.getAllByRole('option')[i].getAttribute('aria-selected')).toBe('true')
      press('Enter')
      vi.runAllTimers()
      expect(items[i].run).toHaveBeenCalledTimes(1)
      view.unmount()
    }
    render(<Palette items={items} onClose={() => undefined} />)
    press('ArrowUp')
    press('Enter')
    vi.runAllTimers()
    expect(items[items.length - 1].run).toHaveBeenCalledTimes(2)
    vi.useRealTimers()
  })

  it('narrows as you type and shows each shortcut', () => {
    const run = vi.fn()
    render(<Palette items={[{ id: 'a', group: 'actions', label: 'Enter zen mode', keys: 'Z', run }, { id: 'b', group: 'filter', label: 'Just my cards', keys: 'Q', run }, { id: 'c', group: 'cards', label: 'Hidden card', typed: true, run }]} onClose={() => undefined} />)
    expect(options()).toEqual(['Enter zen modeZ', 'Just my cardsQ'])
    fireEvent.change(input(), { target: { value: 'filter my' } })
    expect(options()).toEqual(['Just my cardsQ'])
    fireEvent.change(input(), { target: { value: 'hidden' } })
    expect(options()).toEqual(['Hidden card'])
  })
})

function openAt(path: string) {
  window.history.pushState(null, '', path)
  render(
    <Toaster>
      <App />
    </Toaster>,
  )
}

describe('palette on a board', () => {
  beforeEach(() => {
    calls = []
    role = 'member'
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init: RequestInit = {}) => {
        const method = init.method ?? 'GET'
        const body = typeof init.body === 'string' ? JSON.parse(init.body) : undefined
        calls.push({ url, method, body })
        if (url === '/api/v1/me') return json(me)
        if (url === '/api/v1/c/acme/boards/7') return json(board())
        if (url === '/api/v1/me/settings') return json(body ?? {})
        if (url === '/api/v1/c/acme/projects') return json({ projects: [] })
        if (method !== 'GET') return new Response(null, { status: 204 })
        return json({ error: 'not found' }, 404)
      }),
    )
  })

  afterEach(async () => {
    cleanup()
    vi.unstubAllGlobals()
    await saveSettings({ zen: false, zen_density: 'comfortable', zen_quiet: true }).catch(() => undefined)
    window.history.pushState(null, '', '/')
  })

  it('opens with Ctrl K and lists the groups with shortcuts', async () => {
    openAt('/c/acme/b/7')
    await screen.findByText('Write copy')
    ctrlK()
    expect(screen.getByRole('dialog', { name: 'Command palette' })).toBeTruthy()
    const subs = [...document.querySelectorAll('.pal .sub')].map((s) => s.textContent)
    expect(subs).toEqual(['Actions', 'Filter', 'Density', 'Zen quiet mode', 'Card actions', 'Go to'])
    expect(options()).toContain('Enter zen modeZ')
    expect(options()).toContain('Just my cardsQ')
    press('Escape')
    expect(screen.queryByRole('dialog', { name: 'Command palette' })).toBeNull()
  })

  it('says so when a card action has no selected card', async () => {
    openAt('/c/acme/b/7')
    await screen.findByText('Write copy')
    await choose(/^Mark the selected card done/)
    expect(await screen.findByText(/Select a card first/)).toBeTruthy()
    expect(writes()).toEqual([])
  })

  it('runs card actions on the card selected with J', async () => {
    openAt('/c/acme/b/7')
    await screen.findByText('Write copy')
    fireEvent.keyDown(window, { key: 'j' })
    await choose(/^Mark "Write copy" done/)
    await waitFor(() => expect(writes()).toContainEqual({ url: '/api/v1/c/acme/boards/7/cards/21', method: 'PATCH', body: { done: true } }))
    await choose(/^Move "Write copy" to Done/)
    await waitFor(() => expect(writes()).toContainEqual({ url: '/api/v1/c/acme/boards/7/cards/21/move', method: 'POST', body: { list_id: 12, after: 23 } }))
    await choose(/^Archive "Write copy"/)
    await waitFor(() => expect(writes()).toContainEqual({ url: '/api/v1/c/acme/boards/7/cards/21/archive', method: 'POST', body: undefined }))
  })

  it('switches filter, density, quiet mode, zen and views', async () => {
    openAt('/c/acme/b/7')
    await screen.findByText('Write copy')
    await choose(/^Just my cards/)
    expect(screen.queryByText('Write copy')).toBeNull()
    await choose(/^Show everyone's cards/)
    expect(await screen.findByText('Write copy')).toBeTruthy()
    await choose(/^Compact cards/)
    await waitFor(() => expect(writes()).toContainEqual({ url: '/api/v1/me/settings', method: 'PATCH', body: { zen_density: 'compact' } }))
    await choose(/^Stay quiet in zen mode: turn off/)
    await waitFor(() => expect(writes()).toContainEqual({ url: '/api/v1/me/settings', method: 'PATCH', body: { zen_quiet: false } }))
    await choose(/^Enter zen mode/)
    await waitFor(() => expect(document.querySelector('.bd.zen')).toBeTruthy())
    await choose(/^Show the Dashboard/)
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/b/7/dashboard'))
    await choose(/^My cards/)
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/my-cards'))
  })

  it('gives observers no card changes and no new card', async () => {
    role = 'observer'
    openAt('/c/acme/b/7')
    await screen.findByText('Write copy')
    fireEvent.keyDown(window, { key: 'j' })
    ctrlK()
    const o = options()
    expect(o.some((x) => x.startsWith('Open "Write copy"'))).toBe(true)
    expect(o.some((x) => /^(Mark|Move|Archive|Assign|Dates|Labels|New card)/.test(x))).toBe(false)
  })

  it('opens over an open card and acts on that card', async () => {
    openAt('/c/acme/b/7/c/22')
    await screen.findByRole('dialog', { name: 'Card' })
    await choose(/^Mark "Audit nav" done/)
    await waitFor(() => expect(writes()).toContainEqual({ url: '/api/v1/c/acme/boards/7/cards/22', method: 'PATCH', body: { done: true } }))
  })

  it('offers no card actions for a card hidden by the filter', async () => {
    openAt('/c/acme/b/7')
    await screen.findByText('Write copy')
    fireEvent.keyDown(window, { key: 'j' })
    fireEvent.keyDown(window, { key: 'q' })
    ctrlK()
    expect(options().some((x) => x.includes('"Write copy"'))).toBe(false)
  })
})
