export type MyCard = {
  id: number
  title: string
  done: boolean
  due_at: number | null
  list: string
  board: { id: number; name: string }
  project: { id: number; name: string }
  items: number
  items_done: number
  can_edit: boolean
  labels: { id: number; name: string; color: number }[]
  company: { slug: string; name: string }
}

export type MyItem = {
  id: number
  text: string
  done: boolean
  due_at: number | null
  checklist: number
  card: { id: number; title: string }
  board: { id: number; name: string }
  project: { id: number; name: string }
  can_edit: boolean
  company: { slug: string; name: string }
}
