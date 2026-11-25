import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from '@/App'
import { describe as describeActivity } from '@/card/activity'
import { Toaster } from '@/ui/Toaster'

type Call = { url: string; method: string; body?: unknown; headers: Record<string, string> }

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

const project = (role: string) => ({
  id: 4,
  name: 'Website relaunch',
  description: 'New site',
  color: 'c1',
  background: 'sea',
  members_create_boards: role !== 'member-no-boards',
  members_invite: false,
  role: role === 'member-no-boards' ? 'member' : role,
  member_count: 2,
  can: {
    edit: role !== 'observer',
    manage: role === 'owner',
    invite: role === 'owner',
    create_board: role === 'owner' || role === 'member',
  },
})

const preview = {
  format: 'generic',
  name: 'Marketing Q4',
  lists: ['Ideas', 'Doing', 'Done'],
  counts: { lists: 3, cards: 12, archived: 4, labels: 5, checklists: 0, items: 0, comments: 7, links: 2, files: 1 },
  colours: 11,
  people: [
    { key: 'p1', name: 'Mai Tran', email: 'mai@example.com', match: 1 },
    { key: 'p2', name: 'Former intern', email: '', match: null },
  ],
  candidates: [
    { id: 1, name: 'Mai Tran', email: 'mai@example.com', editor: true },
    { id: 2, name: '', email: 'jo@example.com', editor: true },
  ],
  project: 'Website relaunch',
}

let role = 'owner'
let calls: Call[] = []
let previewError: string | null = null

function board() {
  return {
    id: 7,
    name: 'Sprint 15',
    description: '',
    background: 'sea',
    restricted: false,
    closed: false,
    starred: false,
    project: { id: 4, name: 'Website relaunch' },
    role,
    can: { edit: role !== 'observer', manage: role === 'owner', invite: role === 'owner', create_board: role === 'owner' },
    people: [{ id: 1, name: 'Mai Tran', email: 'mai@example.com', role, access: true }],
    lists: [{ id: 11, title: 'To do', position: 'V', cards: [] }],
    labels: [],
    archived_lists: 0,
  }
}

beforeEach(() => {
  calls = []
  role = 'owner'
  previewError = null
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? 'GET'
      calls.push({ url, method, body: init.body, headers: (init.headers ?? {}) as Record<string, string> })
      if (url === '/api/v1/me') return json(me)
      if (url === '/api/v1/c/acme/projects/4') return json(project(role))
      if (url === '/api/v1/c/acme/projects/4/boards') return json({ boards: [] })
      if (url === '/api/v1/c/acme/boards/7') return json(board())
      if (url === '/api/v1/c/acme/boards/42') return json({ ...board(), id: 42, name: 'Marketing Q4' })
      if (url.startsWith('/api/v1/c/acme/projects/4/imports/preview')) return previewError ? json({ error: previewError }, 400) : json(preview)
      if (url === '/api/v1/c/acme/projects/4/imports') return json({ id: 42, name: 'Marketing Q4', counts: { ...preview.counts, cards: 12, lists: 3, archived: 0 } }, 201)
      if (method !== 'GET') return new Response(null, { status: 204 })
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

function exportFile(name = 'marketing-q4.json', size?: number) {
  const file = new File(['{"name":"Marketing Q4"}'], name, { type: 'application/json' })
  if (size !== undefined) Object.defineProperty(file, 'size', { value: size })
  return file
}

async function chooseFile(file: File) {
  const input = (await screen.findByText('Choose file')).querySelector('input[type=file]') as HTMLInputElement
  fireEvent.change(input, { target: { files: [file] } })
}

const imports = () => calls.filter((c) => c.method === 'POST' && c.url === '/api/v1/c/acme/projects/4/imports')
const sentBody = async (i: number) => JSON.parse(await (imports()[i].body as Blob).text()) as Record<string, unknown>

describe('import a board', () => {
  it('previews the chosen file, sends only changed people and opens the new board', async () => {
    open('/c/acme/p/4/import')
    expect(await screen.findByRole('heading', { name: 'Import a board' })).toBeTruthy()
    const file = exportFile()
    await chooseFile(file)

    expect(await screen.findByText('Marketing Q4')).toBeTruthy()
    const sent = calls.find((c) => c.url.includes('/imports/preview'))!
    expect(sent.url).toBe('/api/v1/c/acme/projects/4/imports/preview?name=marketing-q4.json')
    expect(sent.method).toBe('POST')
    expect(sent.body).toBe(file)
    expect(sent.headers['content-type']).toBe('application/json')
    expect(sent.headers['x-csrf-token']).toBe('c'.repeat(64))

    expect(screen.getByText('open cards').previousSibling?.textContent).toBe('12')
    expect(screen.getByText('archived cards').previousSibling?.textContent).toBe('4')
    expect(screen.getByText('attachments').previousSibling?.textContent).toBe('3')
    expect(screen.queryByText('checklists')).toBeNull()
    expect(screen.getByText('Lists: Ideas · Doing · Done')).toBeTruthy()
    expect(screen.getByText(/The file uses 11 label colours/)).toBeTruthy()

    const mai = screen.getByLabelText('Match Mai Tran') as HTMLSelectElement
    const intern = screen.getByLabelText('Match Former intern') as HTMLSelectElement
    expect(mai.value).toBe('1')
    expect(intern.value).toBe('')
    expect(within(intern).getByRole('option', { name: 'jo@example.com' })).toBeTruthy()
    fireEvent.change(intern, { target: { value: '2' } })
    fireEvent.click(screen.getByLabelText(/Bring archived cards/))
    fireEvent.click(screen.getByLabelText(/Bring comments/))

    const go = screen.getByRole('button', { name: 'Import into Website relaunch' })
    fireEvent.click(go)
    fireEvent.click(go)
    expect(await screen.findByRole('heading', { name: 'Imported' })).toBeTruthy()
    expect(imports()).toHaveLength(1)
    const done = imports()[0]
    expect(done.url).toBe('/api/v1/c/acme/projects/4/imports')
    expect(await sentBody(0)).toEqual({ name: 'marketing-q4.json', archived: false, comments: false, links: true, people: { p2: 2 }, file: { name: 'Marketing Q4' } })
    expect(screen.getByText('Marketing Q4 is ready: 12 cards in 3 lists. The import is in the board activity.')).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'Open the board' }))
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/b/42'))
  })

  it('leaves out a matched person by sending null and sends no people header when nothing changed', async () => {
    open('/c/acme/p/4/import')
    await chooseFile(exportFile())
    fireEvent.change(await screen.findByLabelText('Match Mai Tran'), { target: { value: '' } })
    fireEvent.click(screen.getByRole('button', { name: 'Import into Website relaunch' }))
    await screen.findByRole('heading', { name: 'Imported' })
    expect((await sentBody(0)).people).toEqual({ p1: null })

    fireEvent.click(screen.getByRole('button', { name: 'Import another board' }))
    await chooseFile(exportFile())
    fireEvent.click(await screen.findByRole('button', { name: 'Import into Website relaunch' }))
    await waitFor(() => expect(imports()).toHaveLength(2))
    expect(await sentBody(1)).toMatchObject({ archived: true, comments: true, links: true, people: {} })
  })

  it('shows the server error for a file that is not a board export', async () => {
    previewError = 'this file is not a board export'
    open('/c/acme/p/4/import')
    await chooseFile(exportFile('notes.json'))
    expect((await screen.findByRole('alert')).textContent).toBe('this file is not a board export')
    expect(screen.getByText('Drop a board export file here')).toBeTruthy()
  })

  it('refuses a file over 50 MB without calling the server', async () => {
    open('/c/acme/p/4/import')
    await chooseFile(exportFile('huge.json', 50 * 1024 * 1024 + 1))
    expect((await screen.findByRole('alert')).textContent).toMatch(/larger than 50 MB/)
    expect(calls.some((c) => c.url.includes('/imports'))).toBe(false)
  })

  it('offers Import a board only to people who can create boards', async () => {
    open('/c/acme/p/4')
    fireEvent.click(await screen.findByRole('button', { name: 'Import a board' }))
    await waitFor(() => expect(window.location.pathname).toBe('/c/acme/p/4/import'))
    cleanup()

    for (const r of ['observer', 'member-no-boards']) {
      role = r
      open('/c/acme/p/4')
      expect(await screen.findByRole('button', { name: 'Templates' })).toBeTruthy()
      expect(screen.queryByRole('button', { name: 'Import a board' })).toBeNull()
      cleanup()
      open('/c/acme/p/4/import')
      expect(await screen.findByText('You cannot create boards in this project')).toBeTruthy()
      expect(screen.queryByText('Choose file')).toBeNull()
      cleanup()
    }
  })
})

describe('export a board', () => {
  it('gives everyone who can open the board a download link to the export', async () => {
    role = 'observer'
    open('/c/acme/b/7')
    fireEvent.click(await screen.findByRole('button', { name: 'Board menu' }))
    fireEvent.click(screen.getByRole('button', { name: 'Export board' }))
    expect(screen.getByText(/Everything on Sprint 15 as one JSON file/)).toBeTruthy()
    const link = screen.getByRole('link', { name: 'Download' })
    expect(link.getAttribute('href')).toBe('/api/v1/c/acme/boards/7/export')
    expect(link.hasAttribute('download')).toBe(true)
  })

  it('describes an import in the board activity', () => {
    const ctx = { lists: [], labels: [], people: [] }
    const entry = (payload: Record<string, unknown>) => ({ id: 1, type: 'board_imported', actor: null, payload, created_at: 0 })
    expect(describeActivity(entry({ name: 'Marketing Q4', file: 'marketing-q4.json', cards: 12, lists: 3, archived: 0 }) as never, ctx)).toBe('imported this board from marketing-q4.json')
    expect(describeActivity(entry({ name: 'Marketing Q4' }) as never, ctx)).toBe('imported this board')
  })
})
