import {
  closestCorners,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragOverEvent,
  type UniqueIdentifier,
} from '@dnd-kit/core'
import { arrayMove, horizontalListSortingStrategy, SortableContext, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { useState, type ReactNode } from 'react'
import { t } from '@/i18n'
import { CardTile } from '@/board/CardTile'
import { Composer } from '@/board/Composer'
import { ListColumn, type ListActions } from '@/board/ListColumn'
import type { BoardPerson, CardData, Label, ListData } from '@/board/types'

export type CardMove = { card: number; list: number; after: number | null }
export type ListMove = { list: number; after: number | null }

const keyboardCodes = { start: ['KeyG'], cancel: ['Escape'], end: ['KeyG', 'Enter', 'Space'] }

function kind(id: UniqueIdentifier): ['l' | 'c', number] {
  const [k, n] = String(id).split(':')
  return [k as 'l' | 'c', Number(n)]
}

function SortableList({ list, edit, children }: { list: ListData; edit: boolean; children: (s: ReturnType<typeof useSortable>) => ReactNode }) {
  const s = useSortable({ id: `l:${list.id}`, disabled: !edit, data: { type: 'list' } })
  return <>{children(s)}</>
}

export function BoardLists({
  lists: source,
  labels,
  people,
  edit,
  collapsed,
  onCollapse,
  actions,
  composing,
  setComposing,
  onAddCards,
  onMoveCard,
  onMoveList,
  onOpenCard,
  onQuickEdit,
  onToggleDone,
  after,
}: {
  lists: ListData[]
  labels: Label[]
  people: BoardPerson[]
  edit: boolean
  collapsed: number[]
  onCollapse: (id: number, on: boolean) => void
  actions: ListActions
  composing: number | null
  setComposing: (id: number | null) => void
  onAddCards: (list: ListData, titles: string[]) => Promise<boolean>
  onMoveCard: (m: CardMove) => void
  onMoveList: (m: ListMove) => void
  onOpenCard: (card: CardData) => void
  onQuickEdit: (card: CardData, el: HTMLElement) => void
  onToggleDone: (card: CardData) => void
  after?: ReactNode
}) {
  const [local, setLocal] = useState<{ from: ListData[]; lists: ListData[] } | null>(null)
  const [origin, setOrigin] = useState<number | null>(null)
  const lists = local && local.from === source ? local.lists : source
  const setLists = (next: ListData[] | ((cur: ListData[]) => ListData[])) =>
    setLocal((cur) => {
      const base = cur && cur.from === source ? cur.lists : source
      return { from: source, lists: typeof next === 'function' ? next(base) : next }
    })

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates, keyboardCodes }),
  )

  const listOf = (cardId: number, from = lists) => from.find((l) => l.cards.some((c) => c.id === cardId))

  const onDragOver = ({ active, over }: DragOverEvent) => {
    if (!over) return
    const [ak, aid] = kind(active.id)
    if (ak !== 'c') return
    const [ok, oid] = kind(over.id)
    setLists((cur) => {
      const from = listOf(aid, cur)
      const to = ok === 'l' ? cur.find((l) => l.id === oid) : listOf(oid, cur)
      if (!from || !to || from.id === to.id) return cur
      const card = from.cards.find((c) => c.id === aid)!
      const at = ok === 'l' ? to.cards.length : to.cards.findIndex((c) => c.id === oid)
      return cur.map((l) => {
        if (l.id === from.id) return { ...l, cards: l.cards.filter((c) => c.id !== aid) }
        if (l.id === to.id) return { ...l, cards: [...l.cards.slice(0, at), card, ...l.cards.slice(at)] }
        return l
      })
    })
  }

  const onDragEnd = ({ active, over }: DragEndEvent) => {
    const [ak, aid] = kind(active.id)
    setOrigin(null)
    if (!over) {
      setLists(source)
      return
    }

    const [ok, oid] = kind(over.id)
    if (ak === 'l') {
      if (ok !== 'l' || oid === aid) return
      const from = lists.findIndex((l) => l.id === aid)
      const to = lists.findIndex((l) => l.id === oid)
      const next = arrayMove(lists, from, to)
      setLists(next)
      onMoveList({ list: aid, after: to === 0 ? null : next[to - 1].id })
      return
    }

    const list = listOf(aid)
    if (!list) return
    let cards = list.cards
    if (ok === 'c' && oid !== aid && list.cards.some((c) => c.id === oid)) {
      cards = arrayMove(list.cards, list.cards.findIndex((c) => c.id === aid), list.cards.findIndex((c) => c.id === oid))
      setLists((cur) => cur.map((l) => (l.id === list.id ? { ...l, cards } : l)))
    }

    const at = cards.findIndex((c) => c.id === aid)
    const before = source.find((l) => l.cards.some((c) => c.id === aid))
    const prevIndex = before?.cards.findIndex((c) => c.id === aid) ?? -1
    if (before?.id === list.id && prevIndex === at) return
    onMoveCard({ card: aid, list: list.id, after: at === 0 ? null : cards[at - 1].id })
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCorners}
      onDragStart={({ active }) => setOrigin(kind(active.id)[1])}
      onDragOver={onDragOver}
      onDragEnd={onDragEnd}
      onDragCancel={() => {
        setOrigin(null)
        setLists(source)
      }}
      accessibility={{ screenReaderInstructions: { draggable: t('dnd.instructions') } }}
    >
      <div className="lists" id="lists" data-dragging={origin ?? undefined}>
        <SortableContext items={lists.map((l) => `l:${l.id}`)} strategy={horizontalListSortingStrategy}>
          {lists.map((l, i) => (
            <SortableList key={l.id} list={l} edit={edit}>
              {(s) => (
                <ListColumn
                  list={l}
                  lists={lists}
                  index={i}
                  count={l.cards.length}
                  edit={edit}
                  collapsed={collapsed.includes(l.id)}
                  onCollapse={(on) => onCollapse(l.id, on)}
                  actions={actions}
                  sortable={{
                    ref: s.setNodeRef,
                    style: { transform: CSS.Translate.toString(s.transform), transition: s.transition },
                    handle: { ...s.attributes, ...s.listeners, tabIndex: undefined, role: undefined },
                    dragging: s.isDragging,
                  }}
                  footer={
                    edit ? (
                      <Composer
                        open={composing === l.id}
                        onOpen={() => setComposing(l.id)}
                        onClose={() => setComposing(null)}
                        onAdd={(titles) => onAddCards(l, titles)}
                      />
                    ) : undefined
                  }
                >
                  <SortableContext items={l.cards.map((c) => `c:${c.id}`)} strategy={verticalListSortingStrategy}>
                    {l.cards.map((c) => (
                      <CardTile key={c.id} card={c} edit={edit} labels={labels} people={people} onOpen={onOpenCard} onQuickEdit={onQuickEdit} onToggleDone={onToggleDone} />
                    ))}
                  </SortableContext>
                </ListColumn>
              )}
            </SortableList>
          ))}
        </SortableContext>
        {after}
      </div>
    </DndContext>
  )
}
