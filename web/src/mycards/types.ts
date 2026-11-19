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
  labels: { name: string; color: number }[]
  company: { slug: string; name: string }
}
