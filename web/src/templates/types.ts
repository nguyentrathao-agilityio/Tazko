export type BoardTemplate = {
  id: string
  name: string
  description: string
  background: string
  admin: boolean
  by: string
  lists: { title: string; cards: string[] }[]
  labels: number
  fields: number
}

export type TemplateList = { templates: BoardTemplate[]; can_offer: boolean; company: string }
