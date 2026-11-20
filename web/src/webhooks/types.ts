export type Webhook = {
  id: number
  url: string
  events: string[]
  projects: number[] | null
  project_names: string[] | null
  active: boolean
  created_at: number
  by: string | null
  last: { status: string | null; code: number | null; at: number | null }
}

export type Delivery = {
  id: number
  event: string
  status: 'pending' | 'delivered' | 'failed'
  attempts: number
  code: number | null
  error: string | null
  ms: number | null
  created_at: number
  finished_at: number | null
  next_attempt_at: number
}

export const health = (h: Webhook) => (!h.active ? 'paused' : h.last.status === null || h.last.status === 'delivered' ? 'healthy' : 'failing')
