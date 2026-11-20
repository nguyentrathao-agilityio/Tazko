import type { BoardView } from '@/board/BoardPage'
import type { CardData, ListData } from '@/board/types'
import { t, type MessageKey } from '@/i18n'
import type { Settings } from '@/settings/store'

export type Group = 'actions' | 'filter' | 'density' | 'zen' | 'card' | 'goto' | 'cards'

export type Command = { id: string; group: Group; label: string; keys?: string; hint?: string; typed?: boolean; run: () => void }

export const LIMIT = 50

export function narrow(items: Command[], q: string): Command[] {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean)
  return items
    .filter((c) => {
      if (c.typed && !words.length) return false
      const hay = `${t(`palette.group.${c.group}` as MessageKey)} ${c.label} ${c.hint ?? ''}`.toLowerCase()
      return words.every((w) => hay.includes(w))
    })
    .slice(0, LIMIT)
}

export type Personal = {
  density: Settings['zen_density']
  quiet: boolean
  setDensity: (d: Settings['zen_density']) => void
  toggleQuiet: () => void
  search: () => void
  go: (path: string) => void
  companyRoot: string
}

export function personalCommands(p: Personal): Command[] {
  const densities: [Settings['zen_density'], MessageKey][] = [
    ['comfortable', 'palette.comfortable'],
    ['compact', 'palette.compact'],
    ['minimal', 'palette.minimal'],
  ]
  return [
    { id: 'search', group: 'actions', label: t('palette.search'), keys: '/', run: p.search },
    ...densities.map(([d, key]): Command => ({ id: `density-${d}`, group: 'density', label: t(key), hint: p.density === d ? t('palette.current') : undefined, run: () => p.setDensity(d) })),
    { id: 'quiet', group: 'zen', label: p.quiet ? t('palette.quietOff') : t('palette.quietOn'), run: p.toggleQuiet },
    { id: 'go-projects', group: 'goto', label: t('nav.projects'), run: () => p.go(p.companyRoot) },
    { id: 'go-mycards', group: 'goto', label: t('nav.myCards'), run: () => p.go(`${p.companyRoot}/my-cards`) },
    { id: 'go-settings', group: 'goto', label: t('palette.settings'), run: () => p.go(`${p.companyRoot}/me/appearance`) },
  ]
}

export type BoardContext = {
  boardRoot: string
  view: BoardView
  zen: boolean
  edit: boolean
  mine: boolean
  hideDone: boolean
  filtered: boolean
  card: CardData | null
  lists: ListData[]
  zenToggle: () => void
  newCard: () => void
  help: () => void
  toggleMine: () => void
  toggleHideDone: () => void
  openFilter: () => void
  clearFilter: () => void
  noCard: () => void
  done: (c: CardData) => void
  pop: (kind: 'members' | 'dates' | 'labels', c: CardData) => void
  move: (c: CardData, list: ListData) => void
  archive: (c: CardData) => void
  open: (c: CardData) => void
}

const short = (title: string) => (title.length > 28 ? `${title.slice(0, 27)}…` : title)

export function boardCommands(b: BoardContext, p: Personal): Command[] {
  const views: [BoardView, MessageKey][] = [
    ['board', 'palette.showBoard'],
    ['table', 'palette.showTable'],
    ['dashboard', 'palette.showDashboard'],
  ]
  const c = b.card
  const name = c ? short(c.title) : ''
  const onCard = (fn: (card: CardData) => void) => () => (c ? fn(c) : b.noCard())
  const list = c ? b.lists.find((l) => l.cards.some((x) => x.id === c.id)) : undefined
  const cardActions: Command[] = [
    { id: 'card-open', group: 'card', label: c ? t('palette.openCard', { name }) : t('palette.openNone'), keys: 'Enter', run: onCard(b.open) },
  ]
  if (b.edit) {
    cardActions.push(
      { id: 'card-done', group: 'card', label: c ? t(c.done ? 'palette.undone' : 'palette.done', { name }) : t('palette.doneNone'), keys: 'Space', run: onCard(b.done) },
      { id: 'card-members', group: 'card', label: c ? t('palette.members', { name }) : t('palette.membersNone'), keys: 'A', run: onCard((x) => b.pop('members', x)) },
      { id: 'card-dates', group: 'card', label: c ? t('palette.dates', { name }) : t('palette.datesNone'), keys: 'D', run: onCard((x) => b.pop('dates', x)) },
      { id: 'card-labels', group: 'card', label: c ? t('palette.labels', { name }) : t('palette.labelsNone'), keys: 'T', run: onCard((x) => b.pop('labels', x)) },
      ...(c && list
        ? b.lists.filter((l) => l.id !== list.id).map((l): Command => ({ id: `card-move-${l.id}`, group: 'card', label: t('palette.moveTo', { name, list: l.title }), keys: 'M', run: () => b.move(c, l) }))
        : [{ id: 'card-move', group: 'card' as const, label: t('palette.moveNone'), keys: 'M', run: b.noCard }]),
      { id: 'card-archive', group: 'card', label: c ? t('palette.archive', { name }) : t('palette.archiveNone'), keys: 'X', run: onCard(b.archive) },
    )
  }

  const [search, ...personal] = personalCommands(p)
  return [
    { id: 'zen', group: 'actions', label: b.zen ? t('palette.zenOff') : t('palette.zenOn'), keys: 'Z', run: b.zenToggle },
    ...views.filter(([v]) => v !== b.view).map(([v, key]): Command => ({ id: `view-${v}`, group: 'actions', label: t(key), run: () => p.go(v === 'board' ? b.boardRoot : `${b.boardRoot}/${v}`) })),
    ...(b.edit ? [{ id: 'new-card', group: 'actions' as const, label: t('palette.newCard'), keys: 'N', run: b.newCard }] : []),
    search,
    { id: 'help', group: 'actions', label: t('palette.shortcuts'), keys: '`', run: b.help },
    { id: 'mine', group: 'filter', label: b.mine ? t('palette.mineOff') : t('palette.mineOn'), keys: 'Q', run: b.toggleMine },
    { id: 'hide-done', group: 'filter', label: b.hideDone ? t('palette.showDone') : t('palette.hideDone'), run: b.toggleHideDone },
    { id: 'filter', group: 'filter', label: t('palette.filter'), keys: 'F', run: b.openFilter },
    ...(b.filtered ? [{ id: 'clear-filter', group: 'filter' as const, label: t('palette.clearFilter'), run: b.clearFilter }] : []),
    ...personal.filter((x) => x.group !== 'goto'),
    ...cardActions,
    ...personal.filter((x) => x.group === 'goto'),
    ...b.lists.flatMap((l) => l.cards.map((x): Command => ({ id: `go-card-${x.id}`, group: 'cards', label: x.title, hint: l.title, typed: true, run: () => b.open(x) }))),
  ]
}
