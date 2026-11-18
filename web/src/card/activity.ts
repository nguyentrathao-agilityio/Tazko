import type { Label, ListData } from '@/board/types'
import type { ActivityEntry } from '@/card/types'

type Ctx = { lists: ListData[]; labels: Label[]; people: { id: number; name: string; email: string }[] }

function str(v: unknown): string {
  return typeof v === 'string' ? v : ''
}

function num(v: unknown): number | null {
  return typeof v === 'number' ? v : null
}

export function describe(e: ActivityEntry, ctx: Ctx): string {
  const p = e.payload ?? {}
  const list = (id: unknown) => ctx.lists.find((l) => l.id === num(id))?.title ?? 'another list'
  const label = (id: unknown) => ctx.labels.find((l) => l.id === num(id))?.name || 'a label'
  const who = (id: unknown) => {
    const person = ctx.people.find((x) => x.id === num(id))
    return person ? person.name || person.email : 'someone'
  }

  switch (e.type) {
    case 'card_created':
      return `added this card to ${list(p.list)}`
    case 'card_copied':
      return 'copied this card'
    case 'card_renamed':
      return `renamed this card to “${str(p.to) || str(p.title)}”`
    case 'card_moved':
      return p.from_list === p.to_list ? `moved this card within ${list(p.to_list)}` : `moved this card from ${list(p.from_list)} to ${list(p.to_list)}`
    case 'card_archived':
      return 'archived this card'
    case 'card_restored':
      return 'restored this card'
    case 'card_done':
      return p.done ? 'marked this card complete' : 'marked this card incomplete'
    case 'card_updated':
      if ('description' in p) return 'updated the description'
      if ('due_at' in p || 'start_at' in p) return 'changed the dates'
      if ('cover' in p) return 'changed the cover'
      return 'updated this card'
    case 'label_added':
      return `added the ${label(p.label)} label`
    case 'label_removed':
      return `removed the ${label(p.label)} label`
    case 'member_assigned':
      return `added ${who(p.user)} to this card`
    case 'member_unassigned':
      return `removed ${who(p.user)} from this card`
    case 'checklist_added':
      return `added the checklist ${str(p.title)}`
    case 'checklist_renamed':
      return `renamed a checklist to ${str(p.title)}`
    case 'checklist_deleted':
      return 'deleted a checklist'
    case 'checklist_moved':
      return 'reordered the checklists'
    case 'item_added':
      return `added “${str(p.text)}” to a checklist`
    case 'item_checked':
      return 'completed a checklist item'
    case 'item_unchecked':
      return 'reopened a checklist item'
    case 'item_changed':
      return 'changed a checklist item'
    case 'item_deleted':
      return 'deleted a checklist item'
    case 'item_moved':
      return 'reordered a checklist'
    case 'comment_added':
      return 'commented on this card'
    case 'comment_edited':
      return 'edited a comment'
    case 'comment_deleted':
      return 'deleted a comment'
    case 'cards_created':
      return `added ${Array.isArray(p.cards) ? p.cards.length : 'several'} cards to ${list(p.list)}`
    case 'cards_moved':
      return `moved all cards from ${list(p.from_list)} to ${list(p.to_list)}`
    case 'cards_archived':
      return `archived all cards in ${list(p.list)}`
    case 'board_created':
      return p.copied_from ? 'created this board as a copy' : 'created this board'
    case 'board_updated':
      return 'changed the board'
    case 'board_closed':
      return 'closed this board'
    case 'board_reopened':
      return 'reopened this board'
    case 'board_access_changed':
      return p.restricted ? 'restricted this board to chosen people' : 'opened this board to the whole project'
    case 'list_created':
      return `added the list ${str(p.title)}`
    case 'list_renamed':
      return `renamed the list ${str(p.from)} to ${str(p.to)}`
    case 'list_moved':
      return `moved the list ${list(p.list)}`
    case 'list_copied':
      return `copied a list as ${str(p.title)}`
    case 'list_archived':
      return `archived the list ${str(p.title)}`
    case 'list_restored':
      return `restored the list ${str(p.title)}`
    case 'list_sorted':
      return `sorted the list ${list(p.list)}`
    case 'label_created':
      return `created the label ${str(p.name)}`
    case 'label_changed':
      return 'changed a label'
    case 'label_deleted':
      return 'deleted a label'
    default:
      return e.type.replace(/_/g, ' ')
  }
}

export type BoardEntry = ActivityEntry & { card: { id: number; title: string | null } | null }

export function describeBoard(e: BoardEntry, ctx: Ctx): string {
  const text = describe(e, ctx)
  const title = e.card?.title
  return title ? text.replace('this card', `“${title}”`) : text
}
