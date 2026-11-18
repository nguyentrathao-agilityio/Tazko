import { useEffect, useRef, useState } from 'react'
import type { CardData, ListData } from '@/board/types'
import { isTyping } from '@/ui/typing'

export type KeyActions = {
  open: (card: CardData) => void
  quick: (card: CardData) => void
  done: (card: CardData) => void
  pop: (kind: 'members' | 'dates' | 'labels' | 'move', card: CardData) => void
  archive: (card: CardData) => void
  label: (card: CardData, index: number) => void
  compose: (list: number) => void
  filter: () => void
  zen: () => void
  help: () => void
  leaveZen: () => boolean
  goToCard: (card: CardData) => void
}

type Opts = { lists: ListData[]; edit: boolean; cardOpen: number | null; zen: boolean; actions: KeyActions }

export function blockingLayer(): boolean {
  return Boolean(document.querySelector('.pop, .modal, .drawer, .srch, .ov:not(.cmov)'))
}

function find(lists: ListData[], id: number | null): [number, number] | null {
  if (id === null) return null
  for (let i = 0; i < lists.length; i++) {
    const j = lists[i].cards.findIndex((c) => c.id === id)
    if (j >= 0) return [i, j]
  }
  return null
}

export function step(lists: ListData[], from: number | null, dx: number, dy: number): CardData | null {
  const at = find(lists, from)
  if (!at) {
    const first = lists.find((l) => l.cards.length > 0)
    return first ? first.cards[0] : null
  }
  const [li, ci] = at
  if (dy) {
    const cards = lists[li].cards
    return cards[Math.max(0, Math.min(cards.length - 1, ci + dy))] ?? null
  }

  for (let i = li + dx; i >= 0 && i < lists.length; i += dx) {
    const cards = lists[i].cards
    if (cards.length) return cards[Math.min(ci, cards.length - 1)]
  }
  return lists[li].cards[ci]
}

export function useBoardKeys({ lists, edit, cardOpen, zen, actions }: Opts) {
  const [selected, setSelected] = useState<number | null>(null)
  const hover = useRef<number | null>(null)
  const state = useRef({ lists, edit, cardOpen, zen, actions, selected })

  useEffect(() => {
    state.current = { lists, edit, cardOpen, zen, actions, selected }
  })

  useEffect(() => {
    const onOver = (e: MouseEvent) => {
      const el = (e.target as HTMLElement | null)?.closest?.('.cd')
      hover.current = el ? Number(el.getAttribute('data-card')) : null
    }

    const onKey = (e: KeyboardEvent) => {
      const s = state.current
      if (e.ctrlKey || e.metaKey || e.altKey || e.defaultPrevented) return
      if (isTyping(e.target) || blockingLayer() || document.querySelector('.lists[data-dragging]')) return
      const key = e.key
      const lower = key.length === 1 ? key.toLowerCase() : key
      const target = (): CardData | null => {
        const focused = document.activeElement?.closest?.('.cd')
        const id = focused ? Number(focused.getAttribute('data-card')) : (s.selected ?? hover.current)
        const at = find(s.lists, id)
        return at ? s.lists[at[0]].cards[at[1]] : null
      }
      const handled = () => e.preventDefault()

      if (key === '`' || key === '?') {
        handled()
        s.actions.help()
        return
      }

      if (lower === 'z' && (!s.cardOpen || s.zen)) {
        handled()
        s.actions.zen()
        return
      }

      const arrowKeys: Record<string, number[]> = { ArrowDown: [0, 1], ArrowUp: [0, -1], ArrowLeft: [-1, 0], ArrowRight: [1, 0] }
      const arrows: Record<string, number[]> = s.zen ? arrowKeys : {}
      const moves: Record<string, number[]> = { j: [0, 1], k: [0, -1], h: [-1, 0], l: [1, 0], ...arrows }
      const move = moves[lower]

      if (s.cardOpen) {
        if (move) {
          handled()
          const next = step(s.lists, s.cardOpen, move[0], move[1])
          if (next && next.id !== s.cardOpen) s.actions.goToCard(next)
        }
        return
      }

      const boardKeys: Record<string, number[]> = s.selected !== null ? { ...moves, ...arrowKeys } : moves
      const nav = boardKeys[lower]
      if (nav) {
        handled()
        const next = step(s.lists, s.selected ?? hover.current, nav[0], nav[1])
        if (next) {
          setSelected(next.id)
          document.querySelector(`[data-card="${next.id}"]`)?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' })
        }
        return
      }

      if (key === 'Escape') {
        if (s.selected !== null) {
          handled()
          setSelected(null)
          return
        }

        if (s.actions.leaveZen()) handled()
        return
      }

      if (lower === 'f') {
        handled()
        s.actions.filter()
        return
      }

      if (lower === 'n' && s.edit) {
        handled()
        const at = find(s.lists, s.selected ?? hover.current)
        const list = at ? s.lists[at[0]] : s.lists[0]
        if (list) s.actions.compose(list.id)
        return
      }

      const card = target()
      if (!card) return
      if (key === 'Enter') {
        const active = document.activeElement
        if (active && active !== document.body && !active.classList.contains('cd')) return
        handled()
        s.actions.open(card)
        return
      }

      if (!s.edit) return
      if (lower === 'e') {
        handled()
        s.actions.quick(card)
      } else if (key === ' ') {
        handled()
        s.actions.done(card)
      } else if (lower === 'x') {
        handled()
        s.actions.archive(card)
        setSelected(null)
      } else if (lower === 'a' || lower === 'd' || lower === 't' || lower === 'm') {
        handled()
        s.actions.pop(lower === 'a' ? 'members' : lower === 'd' ? 'dates' : lower === 't' ? 'labels' : 'move', card)
      } else if (/^[1-8]$/.test(key)) {
        handled()
        s.actions.label(card, Number(key) - 1)
      }
    }

    document.addEventListener('mouseover', onOver)
    window.addEventListener('keydown', onKey)

    return () => {
      document.removeEventListener('mouseover', onOver)
      window.removeEventListener('keydown', onKey)
    }
  }, [])

  return { selected, setSelected }
}
