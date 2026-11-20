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

let role = 'member'
let calls: Call[] = []

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
  people: [
    { id: 1, name: 'Mai Tran', email: 'mai@example.com', role, access: true },
    { id: 2, name: 'Jo Lee', email: 'jo@example.com', role: 'member', access: true },
    { id: 3, name: 'Ana Reyes', email: 'ana@example.com', role: 'observer', access: true },
  ],
  lists: [{ id: 11, title: 'To do', position: 'V', cards: [{ id: 21, title: 'Write copy', position: 'V', labels: [31], members: [2], due_at: 1, done: false, comments: 5, items: 8, items_done: 2, has_description: true, is_template: true, fields: { '81': 'High', '82': 3 } }] }],
  labels: [{ id: 31, name: 'Bug', color: 3 }],
  fields: [
    { id: 81, name: 'Priority', type: 'select', options: ['Low', 'High'], show_on_card: true },
    { id: 82, name: 'Points', type: 'number', options: [], show_on_card: false },
  ],
  archived_lists: 0,
})

const card = () => ({
  id: 21,
  title: 'Write copy',
  description: '<p><b>Goal</b> ship it</p>',
  start_at: null,
  due_at: null,
  done: false,
  cover: null,
  cover_image: null,
  fields: { '81': 'High', '82': 3 },
  attachments: [
    { id: 71, kind: 'file', name: 'old-draft.png', size: 2048, url: null, image: true, type: 'image/png', created_at: 1_700_000_000 },
    { id: 72, kind: 'file', name: 'spec.pdf', size: 3_200_000, url: null, image: false, type: 'application/pdf', created_at: 1_700_000_001 },
    { id: 73, kind: 'link', name: 'Brand guide', size: 0, url: 'https://example.com/brand', image: false, type: '', created_at: 1_700_000_002 },
    { id: 74, kind: 'file', name: 'hero.png', size: 482_000, url: null, image: true, type: 'image/png', created_at: 1_700_000_003 },
  ],
  list: { id: 11, title: 'To do' },
  archived: false,
  created_at: 1_700_000_000,
  created_by: null,
  labels: [31],
  members: [2],
  checklists: [{ id: 41, title: 'Launch', items: Array.from({ length: 8 }, (_, i) => ({ id: 50 + i, text: `Step ${i + 1}`, done: i < 2, assignee: i === 2 ? 2 : null, due_at: i === 2 ? 1_900_000_000 : null })) }],
  comments: Array.from({ length: 5 }, (_, i) => ({ id: 60 + i, author: { id: i === 0 ? 1 : 2, name: i === 0 ? 'Mai Tran' : 'Jo Lee', email: 'x@example.com' }, body: `Note ${i + 1}`, created_at: 1_700_000_000 + i, edited_at: null, reactions: i === 4 ? [{ emoji: '👍', count: 2, mine: true, people: ['Mai Tran', 'Jo Lee'] }] : [] })),
  activity: [{ id: 1, type: 'card_created', actor: { id: 2, name: 'Jo Lee', email: 'jo@example.com' }, payload: { list: 11 }, created_at: 1_699_999_000 }],
  can: { edit: role !== 'observer' },
})

beforeEach(() => {
  calls = []
  role = 'member'
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: typeof init.body === 'string' ? JSON.parse(init.body) : init.body })
      if (url === '/api/v1/me') return json(me)
      if (url === '/api/v1/c/acme/storage') return json({ max_file: 10 * 1024 * 1024, used: 1_932_735_283, limit: 5 * 1024 ** 3 })
      if (url === '/api/v1/c/acme/boards/7/card-templates') return json({ saved: [{ id: 21, title: 'Write copy' }], builtin: [{ key: 'bug', title: 'Bug report' }] })
      if (url === '/api/v1/c/acme/boards/7/card-templates/use') return json({ id: 99 }, 201)
      if (url.startsWith('/api/v1/c/acme/boards/7/background?name=')) return json({ error: 'choose a PNG, JPEG, GIF or WebP picture for the background' }, 415)
      if (url.startsWith('/api/v1/c/acme/boards/7/cards/21/attachments?name=')) return json({ error: 'this file is named .png but it is not an image Tazko can show' }, 415)
      if (method !== 'GET') return method === 'POST' ? json({ id: 99 }, 201) : new Response(null, { status: 204 })
      if (url === '/api/v1/c/acme/boards/7') return json(board())
      if (url === '/api/v1/c/acme/boards/7/cards/21') return json(card())
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

describe('card dialog', () => {
  it('shows an item\'s assignee and due date only when set, and sets them from its menu', async () => {
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    await within(dialog).findByText('Step 3')
    const rows = [...dialog.querySelectorAll('.it')]
    expect(rows.filter((r) => r.querySelector('.itm')).map((r) => r.querySelector('.itx')?.textContent)).toEqual(['Step 3'])
    expect(within(rows[2] as HTMLElement).getByTitle('Assigned to Jo Lee')).toBeTruthy()

    fireEvent.click(within(dialog).getByRole('button', { name: 'Assign or date "Step 4"' }))
    const pop = await screen.findByRole('dialog', { name: 'Checklist item' })
    expect(within(pop).queryByRole('button', { name: /Ana Reyes/ })).toBeNull()
    fireEvent.click(within(pop).getByRole('button', { name: /Jo Lee/ }))
    await waitFor(() => expect(calls).toContainEqual({ url: '/api/v1/c/acme/boards/7/items/53', method: 'PATCH', body: { assignee: 2 } }))
    const dateBox = within(pop).getByLabelText('Due date')
    fireEvent.change(dateBox, { target: { value: '2026-12-24' } })
    expect(calls.some((c) => c.url === '/api/v1/c/acme/boards/7/items/53' && 'due_at' in ((c.body as object) ?? {}))).toBe(false)
    fireEvent.blur(dateBox)
    await waitFor(() => expect(calls.some((c) => c.url === '/api/v1/c/acme/boards/7/items/53' && typeof (c.body as { due_at?: number }).due_at === 'number')).toBe(true))

    fireEvent.click(within(rows[2] as HTMLElement).getByRole('button', { name: /^Due / }))
    const pop2 = await screen.findByRole('dialog', { name: 'Checklist item' })
    fireEvent.click(within(pop2).getByRole('button', { name: 'Remove due date' }))
    await waitFor(() => expect(calls).toContainEqual({ url: '/api/v1/c/acme/boards/7/items/52', method: 'PATCH', body: { due_at: null } }))
    fireEvent.click(within(pop2).getByRole('button', { name: 'Remove assignee' }))
    await waitFor(() => expect(calls).toContainEqual({ url: '/api/v1/c/acme/boards/7/items/52', method: 'PATCH', body: { assignee: null } }))
  })

  it('gives observers item dates and people without a menu', async () => {
    role = 'observer'
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    await within(dialog).findByText('Step 3')
    expect(within(dialog).queryByRole('button', { name: /^Assign or date/ })).toBeNull()
    expect((within(dialog).getByRole('button', { name: /^Due / }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('opens from the tile, shows badges and long-card limits', async () => {
    open('/c/acme/b/7')
    const tile = await screen.findByRole('button', { name: 'Write copy' })
    expect(within(tile).getByText('Bug')).toBeTruthy()
    expect(within(tile).getByText('2/8')).toBeTruthy()
    fireEvent.click(tile)
    expect(window.location.pathname).toBe('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    expect(within(dialog).getByText('Goal').tagName).toBe('B')
    expect(within(dialog).getByText('Step 6')).toBeTruthy()
    expect(within(dialog).queryByText('Step 7')).toBeNull()
    fireEvent.click(within(dialog).getByRole('button', { name: 'Show all 8 items' }))
    expect(within(dialog).getByText('Step 8')).toBeTruthy()
    expect(within(dialog).getAllByText(/^Note /)).toHaveLength(4)
    fireEvent.click(within(dialog).getByRole('button', { name: 'Show all 5 comments' }))
    expect(within(dialog).getAllByText(/^Note /)).toHaveLength(5)
    expect(within(dialog).getAllByRole('button', { name: 'Edit' }).length).toBeGreaterThan(0)
  })

  it('saves a comment, ticks an item and adds a label', async () => {
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    fireEvent.change(await within(dialog).findByLabelText('Comment'), { target: { value: 'Ship Friday' } })
    fireEvent.click(within(dialog).getAllByRole('button', { name: 'Save' })[0])
    await waitFor(() => expect(calls.some((c) => c.url.endsWith('/cards/21/comments'))).toBe(true))
    expect(calls.find((c) => c.url.endsWith('/comments'))?.body).toEqual({ body: 'Ship Friday' })

    fireEvent.click(within(dialog).getByRole('checkbox', { name: 'Step 3' }))
    await waitFor(() => expect(calls.some((c) => c.url.endsWith('/items/52'))).toBe(true))
    expect(calls.find((c) => c.url.endsWith('/items/52'))?.body).toEqual({ done: true })

    fireEvent.click(within(dialog).getByRole('button', { name: 'Add members' }))
    const pop = screen.getByRole('dialog', { name: 'Members' })
    expect((within(pop).getByRole('button', { name: /Ana Reyes/ }) as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(within(pop).getByRole('button', { name: /Mai Tran/ }))
    await waitFor(() => expect(calls.some((c) => c.url.endsWith('/cards/21/members/1'))).toBe(true))
  })

  it('gives observers no comment box and no editing', async () => {
    role = 'observer'
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    await within(dialog).findByText('Goal')
    expect(within(dialog).queryByLabelText('Comment')).toBeNull()
    expect(within(dialog).queryByRole('button', { name: 'Add checklist' })).toBeNull()
    expect(within(dialog).queryByRole('button', { name: 'Add members' })).toBeNull()
    expect((within(dialog).getAllByRole('checkbox', { name: 'Mark complete' })[0] as HTMLButtonElement).disabled).toBe(true)
    const files = within(dialog).getByRole('region', { name: 'Attachments' })
    expect(within(files).queryByRole('button', { name: 'Add' })).toBeNull()
    expect(within(files).queryByRole('button', { name: /Delete attachment/ })).toBeNull()
    expect(within(files).getByRole('link', { name: 'hero.png' }).getAttribute('href')).toBe('/api/v1/c/acme/boards/7/attachments/74')
  })

  it('lists attachments, makes a cover and explains a refused upload', async () => {
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    const files = await within(dialog).findByRole('region', { name: 'Attachments' })
    expect(within(files).queryByText('old-draft.png')).toBeNull()
    expect(within(files).getByRole('link', { name: 'spec.pdf' }).getAttribute('href')).toBe('/api/v1/c/acme/boards/7/attachments/72?download=1')
    expect(within(files).getByText(/^3\.1 MB/)).toBeTruthy()
    expect(within(files).getByRole('link', { name: 'Brand guide' }).getAttribute('href')).toBe('https://example.com/brand')
    fireEvent.click(within(files).getByRole('button', { name: 'Show all 4 attachments' }))
    expect(within(files).getByText('old-draft.png')).toBeTruthy()
    fireEvent.click(within(files).getAllByRole('button', { name: 'Make cover' })[1])
    await waitFor(() => expect(calls.some((c) => c.method === 'PUT' && c.url.endsWith('/attachments/74/cover'))).toBe(true))

    fireEvent.click(within(files).getByRole('button', { name: 'Add' }))
    const pop = screen.getByRole('dialog', { name: 'Attach' })
    expect(await within(pop).findByText('Files up to 10 MB. This company uses 1.8 of 5 GB.')).toBeTruthy()
    const big = new File(['x'], 'huge.png', { type: 'image/png' })
    Object.defineProperty(big, 'size', { value: 11 * 1024 * 1024 })
    fireEvent.change(within(pop).getByLabelText('Upload a file'), { target: { files: [big] } })
    expect(within(pop).getByRole('alert').textContent).toBe('Files can be up to 10 MB; this one is larger.')
    expect(calls.some((c) => c.url.includes('/attachments?name='))).toBe(false)
    const fake = new File(['MZ'], 'cat.png', { type: 'image/png' })
    fireEvent.change(within(pop).getByLabelText('Upload a file'), { target: { files: [fake] } })
    await waitFor(() => expect(within(pop).getByRole('alert').textContent).toContain('not an image'))
    expect(calls.find((c) => c.url.includes('/attachments?name='))?.url).toBe('/api/v1/c/acme/boards/7/cards/21/attachments?name=cat.png')

    fireEvent.change(within(pop).getByLabelText('Paste a link'), { target: { value: 'ftp://nope' } })
    fireEvent.click(within(pop).getByRole('button', { name: 'Attach link' }))
    expect(within(pop).getByRole('alert').textContent).toBe('Enter a link that starts with https://')
    fireEvent.change(within(pop).getByLabelText('Paste a link'), { target: { value: 'https://example.com/a' } })
    fireEvent.click(within(pop).getByRole('button', { name: 'Attach link' }))
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/attachments/link'))?.body).toEqual({ url: 'https://example.com/a' }))
  })

  it('shows custom fields on the tile and in the card, and sets values', async () => {
    open('/c/acme/b/7')
    const tile = await screen.findByRole('button', { name: 'Write copy' })
    expect(within(tile).getByText('Priority').parentElement?.textContent).toBe('Priority High')
    expect(within(tile).queryByText('Points')).toBeNull()
    fireEvent.click(tile)
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    const fields = within(dialog).getByRole('region', { name: 'Custom fields' })
    expect((within(fields).getByLabelText('Priority') as HTMLSelectElement).value).toBe('High')
    fireEvent.change(within(fields).getByLabelText('Priority'), { target: { value: 'Low' } })
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/cards/21/fields/81'))?.body).toEqual({ value: 'Low' }))
    const points = within(fields).getByLabelText('Points')
    fireEvent.change(points, { target: { value: '8' } })
    fireEvent.blur(points)
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/cards/21/fields/82'))?.body).toEqual({ value: 8 }))
  })

  it('toggles reactions on comments', async () => {
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    const chip = await within(dialog).findByRole('button', { name: '👍 2' })
    expect(chip.getAttribute('aria-pressed')).toBe('true')
    fireEvent.click(chip)
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/comments/64/reactions'))?.body).toEqual({ emoji: '👍', on: false }))
    fireEvent.click(within(dialog).getAllByRole('button', { name: 'Add reaction' })[0])
    fireEvent.click(screen.getByRole('button', { name: 'React with 🎉' }))
    await waitFor(() => expect(calls.some((c) => c.method === 'PUT' && c.body && (c.body as { emoji: string }).emoji === '🎉')).toBe(true))
  })

  it('gives observers read-only fields and no reaction button', async () => {
    role = 'observer'
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    const fields = await within(dialog).findByRole('region', { name: 'Custom fields' })
    expect((within(fields).getByLabelText('Priority') as HTMLSelectElement).disabled).toBe(true)
    expect(within(dialog).queryByRole('button', { name: 'Add reaction' })).toBeNull()
    expect((within(dialog).getByRole('button', { name: '👍 2' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('refuses a big or wrong background picture with a clear message and lists custom fields', async () => {
    open('/c/acme/b/7')
    await screen.findByRole('button', { name: 'Write copy' })
    fireEvent.click(screen.getByRole('button', { name: 'Board menu' }))
    const menu = screen.getByRole('complementary', { name: 'Board menu' })
    fireEvent.click(within(menu).getByRole('button', { name: 'Custom fields' }))
    expect(within(menu).getByText('Priority')).toBeTruthy()
    expect(within(menu).getByText(/Dropdown: Low, High · shown on cards/)).toBeTruthy()
    fireEvent.click(within(menu).getByRole('button', { name: 'Back' }))
    fireEvent.click(within(menu).getByRole('button', { name: 'Change background' }))
    const input = within(menu).getByLabelText('Upload a background image')
    const big = new File(['x'], 'sky.png', { type: 'image/png' })
    Object.defineProperty(big, 'size', { value: 6 * 1024 * 1024 })
    fireEvent.change(input, { target: { files: [big] } })
    expect(within(menu).getByRole('alert').textContent).toBe('Background pictures can be up to 5 MB; this one is larger.')
    fireEvent.change(input, { target: { files: [new File(['%PDF'], 'brief.png', { type: 'image/png' })] } })
    await waitFor(() => expect(within(menu).getByRole('alert').textContent).toBe('choose a PNG, JPEG, GIF or WebP picture for the background'))
  })

  it('marks cards as templates and makes cards from them under a list', async () => {
    open('/c/acme/b/7')
    const tile = await screen.findByRole('button', { name: 'Write copy' })
    expect(within(tile).getByText('Template')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Create from template' }))
    const pop = screen.getByRole('dialog', { name: 'Card template' })
    fireEvent.click(await within(pop).findByRole('button', { name: 'Bug report' }))
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/card-templates/use'))?.body).toEqual({ list_id: 11, builtin: 'bug' }))
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/b/7/c/99'))
  })

  it('offers Make template in the card menu', async () => {
    open('/c/acme/b/7/c/21')
    const dialog = await screen.findByRole('dialog', { name: 'Card' })
    fireEvent.click(await within(dialog).findByRole('button', { name: /Card menu/ }))
    fireEvent.click(screen.getByRole('button', { name: 'Make template' }))
    await waitFor(() => expect(calls.find((c) => c.url.endsWith('/cards/21/template'))?.body).toEqual({ on: true }))
  })
})

