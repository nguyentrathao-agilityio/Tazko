export type ImportCounts = {
  lists: number
  cards: number
  archived: number
  labels: number
  checklists: number
  items: number
  comments: number
  links: number
  files: number
}

export type ImportPerson = { key: string; name: string; email: string; match: number | null }

export type ImportCandidate = { id: number; name: string; email: string; editor: boolean }

export type ImportPreview = {
  format: 'tazko' | 'generic'
  name: string
  lists: string[]
  counts: ImportCounts
  colours: number
  people: ImportPerson[]
  candidates: ImportCandidate[]
  project: string
}

export type ImportResult = { id: number; name: string; counts: ImportCounts }
